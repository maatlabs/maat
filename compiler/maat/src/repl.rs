//! Interactive REPL for Maat.
//!
//! Provides two entry points:
//!
//! - [`start_interactive`] -- the production REPL powered by [`rustyline`], with
//!   persistent history, keyword completion, syntax highlighting, and multi-line
//!   input via brace/paren/bracket balancing.
//! - [`start`] -- a generic `BufRead`/`Write` interface used exclusively by the
//!   test suite.

use std::borrow::Cow;

use maat_ast::fold::fold_constants;
use maat_ast::{Node, Stmt};
use maat_codegen::{Compiler, SymbolsTable};
use maat_errors::Error;
use maat_eval::{define_macros, expand_macros};
use maat_lexer::Lexer;
use maat_parser::Parser;
use maat_runtime::{Env, Object};
use maat_types::TypeChecker;
use maat_vm::VM;
use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Cmd, Editor, EventHandler, KeyCode, KeyEvent, Modifiers};

use crate::diagnostic;

const PROMPT: &str = ">> ";
const REPL_SOURCE: &str = "<repl>";

/// History file name, stored in the current working directory.
const HISTORY_FILENAME: &str = ".maat_history";

/// Maat keywords, used for tab completion and syntax highlighting.
const KEYWORDS: &[&str] = &[
    "let", "if", "else", "true", "false", "fn", "return", "macro", "as", "loop", "while", "for",
    "in", "break", "continue", "where", "struct", "enum", "match", "impl", "trait", "self", "Self",
    "mod", "use", "pub", "mut",
];

/// Builtin function names available at the top level.
const BUILTIN_NAMES: &[&str] = &["print"];

/// Type names available for completion.
const TYPE_NAMES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "bool",
    "str", "Vector", "Set", "Map", "Option", "Result",
];

// ANSI escape sequences for syntax highlighting.
const BOLD_CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const ANSI_RESET: &str = "\x1b[0m";

/// REPL helper providing tab completion, input validation, and syntax highlighting.
#[derive(rustyline::Helper)]
struct MaatHelper;

impl Completer for MaatHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let prefix = &line[..pos];
        let word_start = prefix
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map_or(0, |i| i + 1);
        let word = &prefix[word_start..];

        if word.is_empty() {
            return Ok((pos, Vec::new()));
        }

        let candidates = KEYWORDS
            .iter()
            .chain(BUILTIN_NAMES)
            .chain(TYPE_NAMES)
            .filter(|kw| kw.starts_with(word))
            .map(|kw| (*kw).to_owned())
            .collect();

        Ok((word_start, candidates))
    }
}

impl Validator for MaatHelper {
    fn validate(&self, ctx: &mut ValidationContext<'_>) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();
        let mut depth: i32 = 0;

        for ch in input.chars() {
            match ch {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }

        if depth > 0 {
            Ok(ValidationResult::Incomplete)
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

impl Hinter for MaatHelper {
    type Hint = String;
}

impl Highlighter for MaatHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let mut result = String::new();
        let mut chars = line.char_indices().peekable();

        while let Some((i, ch)) = chars.next() {
            if ch.is_alphabetic() || ch == '_' {
                let start = i;
                let mut end = i + ch.len_utf8();
                while let Some(&(_, next)) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        end += next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let word = &line[start..end];
                if KEYWORDS.contains(&word) {
                    result.push_str(BOLD_CYAN);
                    result.push_str(word);
                    result.push_str(ANSI_RESET);
                } else {
                    result.push_str(word);
                }
            } else if ch == '"' {
                let start = i;
                let mut end = i + 1;
                let mut escaped = false;
                for (j, sc) in &mut chars {
                    end = j + sc.len_utf8();
                    if escaped {
                        escaped = false;
                    } else if sc == '\\' {
                        escaped = true;
                    } else if sc == '"' {
                        break;
                    }
                }
                result.push_str(GREEN);
                result.push_str(&line[start..end]);
                result.push_str(ANSI_RESET);
            } else if ch.is_ascii_digit() {
                let start = i;
                let mut end = i + 1;
                while let Some(&(j, next)) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        end = j + next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                result.push_str(YELLOW);
                result.push_str(&line[start..end]);
                result.push_str(ANSI_RESET);
            } else {
                result.push(ch);
            }
        }

        Cow::Owned(result)
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _forced: rustyline::highlight::CmdKind,
    ) -> bool {
        true
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Owned(format!("{BOLD_GREEN}{prompt}{ANSI_RESET}"))
    }
}

/// Starts the interactive REPL.
///
/// Provides persistent command history, keyword tab-completion, syntax
/// highlighting, and multi-line input (unclosed braces/parens/brackets
/// trigger a continuation prompt). Session state--global variable bindings,
/// the constants pool, and the macro environment--persists across iterations.
///
/// The session terminates on `exit`, `quit`, or EOF (Ctrl-D).
pub fn start_interactive() {
    use rustyline::Config;
    use rustyline::error::ReadlineError;

    let config = Config::builder().auto_add_history(true).build();

    let mut editor = Editor::with_config(config).expect("failed to initialize REPL editor");
    editor.set_helper(Some(MaatHelper));

    // Bind Tab to complete.
    editor.bind_sequence(
        KeyEvent(KeyCode::Tab, Modifiers::NONE),
        EventHandler::Simple(Cmd::Complete),
    );

    let history_file = std::path::Path::new(HISTORY_FILENAME);
    let _ = editor.load_history(history_file);

    let mut symbols_table = SymbolsTable::new();
    let mut constants: Vec<Object> = Vec::new();
    let mut globals: Vec<Object> = Vec::new();
    let macro_env = Env::default();

    loop {
        let input = match editor.readline(PROMPT) {
            Ok(line) => line,
            Err(ReadlineError::Eof | ReadlineError::Interrupted) => break,
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        };

        let line = input.trim();
        if line.is_empty() {
            println!();
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }

        let mut parser = Parser::new(Lexer::new(line));
        let program = parser.parse();
        if !parser.errors().is_empty() {
            for err in parser.errors() {
                diagnostic::report_parse_error(REPL_SOURCE, line, err);
            }
            println!();
            continue;
        }

        let program = define_macros(program, &macro_env);
        let expanded = expand_macros(Node::Program(program), &macro_env);
        let mut program = match expanded {
            Node::Program(p) => p,
            _ => unreachable!("expand_macros preserves Program variant"),
        };

        let type_errors = TypeChecker::new().check_program(&mut program);
        if !type_errors.is_empty() {
            for err in &type_errors {
                diagnostic::report_type_error(REPL_SOURCE, line, err);
            }
            println!();
            continue;
        }

        let fold_errors = fold_constants(&mut program);
        if !fold_errors.is_empty() {
            for err in &fold_errors {
                diagnostic::report_type_error(REPL_SOURCE, line, err);
            }
            println!();
            continue;
        }

        let only_let_stmts = !program.statements.is_empty()
            && program.statements.iter().all(|s| matches!(s, Stmt::Let(_)));

        let prev_symbols = symbols_table.clone();
        let prev_constants = constants.clone();

        let mut compiler = Compiler::with_state(symbols_table, constants);
        if let Err(e) = compiler.compile(&Node::Program(program)) {
            report_error(line, &e);
            println!();
            symbols_table = prev_symbols;
            constants = prev_constants;
            continue;
        }

        let next_symbols = compiler.symbols_table().clone();
        let bytecode = match compiler.bytecode() {
            Ok(bc) => bc,
            Err(e) => {
                report_error(line, &e);
                println!();
                symbols_table = next_symbols;
                constants = prev_constants;
                continue;
            }
        };
        let next_constants = bytecode.constants.clone();

        let mut vm = VM::with_globals(bytecode, globals);
        if let Err(e) = vm.run() {
            report_error(line, &e);
            println!();
        }

        globals = vm.globals().to_vec();
        symbols_table = next_symbols;
        constants = next_constants;

        match vm.last_popped_stack_elem() {
            Some(val) if !only_let_stmts && !matches!(val, Object::Null) => {
                println!("{val}");
            }
            _ => println!(),
        }
    }

    let _ = editor.save_history(history_file);
}

/// Routes a REPL error to the diagnostic module.
fn report_error(source: &str, error: &Error) {
    match error {
        Error::Parse(e) => diagnostic::report_parse_error(REPL_SOURCE, source, e),
        Error::Compile(e) => diagnostic::report_compile_error(REPL_SOURCE, source, e),
        Error::Type(e) => diagnostic::report_type_error(REPL_SOURCE, source, e),
        Error::Vm(e) => diagnostic::report_vm_error(REPL_SOURCE, source, e),
        _ => eprintln!("{REPL_SOURCE}: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, Write};

    use super::*;

    fn start<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
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
            if line == "exit" || line == "quit" {
                break;
            }
            let mut parser = Parser::new(Lexer::new(line));
            let program = parser.parse();
            if !parser.errors().is_empty() {
                for err in parser.errors() {
                    diagnostic::report_parse_error(REPL_SOURCE, line, err);
                }
                writeln!(writer)?;
                continue;
            }
            let program = define_macros(program, &macro_env);
            let expanded = expand_macros(Node::Program(program), &macro_env);
            let mut program = match expanded {
                Node::Program(p) => p,
                _ => unreachable!("expand_macros preserves Program variant"),
            };
            let type_errors = TypeChecker::new().check_program(&mut program);
            if !type_errors.is_empty() {
                for err in &type_errors {
                    diagnostic::report_type_error(REPL_SOURCE, line, err);
                }
                writeln!(writer)?;
                continue;
            }
            let fold_errors = fold_constants(&mut program);
            if !fold_errors.is_empty() {
                for err in &fold_errors {
                    diagnostic::report_type_error(REPL_SOURCE, line, err);
                }
                writeln!(writer)?;
                continue;
            }
            let only_let_stmts = !program.statements.is_empty()
                && program.statements.iter().all(|s| matches!(s, Stmt::Let(_)));

            let prev_symbols = symbols_table.clone();
            let prev_constants = constants.clone();

            let mut compiler = Compiler::with_state(symbols_table, constants);
            if let Err(e) = compiler.compile(&Node::Program(program)) {
                report_error(line, &e);
                writeln!(writer)?;
                symbols_table = prev_symbols;
                constants = prev_constants;
                continue;
            }
            let next_symbols = compiler.symbols_table().clone();
            let bytecode = match compiler.bytecode() {
                Ok(bc) => bc,
                Err(e) => {
                    report_error(line, &e);
                    writeln!(writer)?;
                    symbols_table = next_symbols;
                    constants = prev_constants;
                    continue;
                }
            };
            let next_constants = bytecode.constants.clone();

            let mut vm = VM::with_globals(bytecode, globals);
            if let Err(e) = vm.run() {
                report_error(line, &e);
                writeln!(writer)?;
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

        // Parse errors now go to stderr via ariadne, so stdout output is empty
        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 0);
    }

    #[test]
    fn report_vm_errors() {
        let input = b"5 + true;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        // VM errors now go to stderr via ariadne, so stdout output is empty
        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 0);
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
