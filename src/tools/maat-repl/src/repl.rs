use std::io::{self, BufRead, Write};

use maat_ast::{Node, Statement};
use maat_codegen::{Compiler, SymbolsTable};
use maat_eval::{define_macros, expand_macros};
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_runtime::{Env, Object};
use maat_vm::VM;

const PROMPT: &str = ">> ";

/// Starts the REPL (Read-Eval-Print Loop) for interactive Maat sessions.
///
/// The REPL compiles each line of input to bytecode and executes it on the
/// bytecode VM. Global variable bindings, the constants pool, the global
/// store, and the macro environment all persist across REPL iterations so
/// that definitions from earlier lines remain visible in subsequent ones.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// // This function is called from the maat-repl binary
/// # fn start<R, W>(_: R, _: &mut W) -> std::io::Result<()> { Ok(()) }
/// # fn main() {
/// let stdin = io::stdin().lock();
/// let mut stdout = io::stdout().lock();
/// start(stdin, &mut stdout).expect("REPL failed");
/// # }
/// ```
///
/// # Behavior
///
/// The REPL operates in an infinite loop until EOF is encountered:
///
/// 1. Displays the prompt (`>> `)
/// 2. Reads a line of input
/// 3. Parses the input into an AST
/// 4. Expands macros (extracting definitions, then replacing macro calls)
/// 5. Compiles the expanded AST to bytecode, reusing accumulated session state
/// 6. Runs the bytecode on the VM, reusing the global store
/// 7. Prints the last popped stack value (suppressing `null` and let-only outputs)
/// 8. Reports errors from any phase without resetting the session
///
/// The session terminates when the reader returns EOF (e.g., Ctrl+D on Unix,
/// Ctrl+Z on Windows, or when reading from a closed pipe).
pub fn start<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
    let mut source = String::new();
    let mut symbols_table = SymbolsTable::new();
    let mut constants: Vec<Object> = Vec::new();
    let mut globals: Vec<Object> = Vec::new();
    let macro_env = Env::default();

    loop {
        write!(writer, "{PROMPT}")?;
        writer.flush()?;

        source.clear();
        if reader.read_line(&mut source)? == 0 {
            break;
        }

        let line = source.trim_end();
        let mut parser = Parser::new(Lexer::new(line));
        let program = parser.parse_program();

        if !parser.errors().is_empty() {
            for err in parser.errors() {
                writeln!(writer, "  {err}")?;
            }
            continue;
        }

        let program = define_macros(program, &macro_env);
        let expanded = expand_macros(Node::Program(program), &macro_env);
        let program = match expanded {
            Node::Program(p) => p,
            _ => unreachable!("expand_macros preserves Program variant"),
        };

        let only_let_stmts = !program.statements.is_empty()
            && program
                .statements
                .iter()
                .all(|s| matches!(s, Statement::Let(_)));

        let prev_symbols = symbols_table.clone();
        let prev_constants = constants.clone();

        let mut compiler = Compiler::with_state(symbols_table, constants);
        if let Err(e) = compiler.compile(&Node::Program(program)) {
            writeln!(writer, "  {e}")?;
            symbols_table = prev_symbols;
            constants = prev_constants;
            continue;
        }

        let next_symbols = compiler.symbols_table().clone();
        let bytecode = match compiler.bytecode() {
            Ok(bc) => bc,
            Err(e) => {
                writeln!(writer, "  {e}")?;
                symbols_table = next_symbols;
                constants = prev_constants;
                continue;
            }
        };
        let next_constants = bytecode.constants.clone();

        let mut vm = VM::with_globals(bytecode, globals);
        if let Err(e) = vm.run() {
            writeln!(writer, "  {e}")?;
        }

        globals = vm.globals().to_vec();
        symbols_table = next_symbols;
        constants = next_constants;

        match vm.last_popped_stack_elem() {
            Some(val) if !only_let_stmts && !matches!(val, Object::Null) => {
                writeln!(writer, "{val}")?;
            }
            _ => writeln!(writer)?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_output(raw: &str) -> Vec<String> {
        raw.lines()
            .filter_map(|line| {
                line.strip_prefix(PROMPT)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
            .collect()
    }

    #[test]
    fn eval_integer_expression() {
        let input = b"5 + 10;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], "15");
    }

    #[test]
    fn eval_let_statement() {
        let input = b"let x = 5;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 0);
    }

    #[test]
    fn eval_multiple_statements() {
        let input = b"let x = 5;\nlet y = 10;\nx + y;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs, vec!["15"]);
    }

    #[test]
    fn eval_boolean_expression() {
        let input = b"1 < 2;\ntrue == false;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs, vec!["true", "false"]);
    }

    #[test]
    fn eval_function_application() {
        let input = b"let add = fn(x, y) { x + y; }; add(2, 3);\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], "5");
    }

    #[test]
    fn eval_closure() {
        let input =
            b"let newAdder = fn(x) { fn(y) { x + y } }; let addTwo = newAdder(2); addTwo(3);\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], "5");
    }

    #[test]
    fn eval_empty_input() {
        let input = b"\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 0);
    }

    #[test]
    fn handle_eof() {
        let input = b"";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.starts_with(PROMPT));
        assert_eq!(extract_output(&result).len(), 0);
    }

    #[test]
    fn report_parse_errors() {
        let input = b"let x = ;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert!(
            outputs
                .iter()
                .any(|line| line.contains("no prefix parse function"))
        );
    }

    #[test]
    fn report_vm_errors() {
        let input = b"5 + true;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("vm error"), "got: {:?}", outputs[0]);
        assert!(
            outputs[0].contains("unsupported types for binary operation"),
            "got: {:?}",
            outputs[0]
        );
    }

    #[test]
    fn eval_macro_expansion() {
        let input = b"let double = macro(x) { quote(unquote(x) * 2) };\ndouble(21);\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs, vec!["42"]);
    }

    #[test]
    fn globals_persist_across_iterations() {
        let input = b"let x = 42;\nx * 2;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs, vec!["84"]);
    }
}
