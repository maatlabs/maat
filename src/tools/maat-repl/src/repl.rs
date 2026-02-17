use std::io::{self, BufRead, Write};

use maat_ast as ast;
use maat_eval::eval;
use maat_lexer::Lexer;
use maat_parse::Parser;
use maat_runtime::{Env, Object};

const PROMPT: &str = ">> ";

/// Starts the REPL (Read-Eval-Print Loop) for interactive Maat sessions.
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
/// 4. Evaluates the AST and prints the result
/// 5. Reports errors if any occur during parsing or evaluation
/// 6. Repeats
///
/// The session terminates when the reader returns EOF (e.g., Ctrl+D on Unix,
/// Ctrl+Z on Windows, or when reading from a closed pipe).
pub fn start<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
    let mut source = String::new();
    let env = Env::default();

    loop {
        write!(writer, "{PROMPT}")?;
        writer.flush()?;

        source.clear();
        let bytes_read = reader.read_line(&mut source)?;
        if bytes_read == 0 {
            break;
        }

        let line = source.trim_end();
        let lexer = Lexer::new(line);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        if !parser.errors().is_empty() {
            for err in parser.errors() {
                writeln!(writer, "  {err}")?;
            }
        } else {
            // Check if the program contains only Let statements
            let only_let_stmts = !program.statements.is_empty()
                && program
                    .statements
                    .iter()
                    .all(|stmt| matches!(stmt, ast::Statement::Let(_)));

            match eval(ast::Node::Program(program), &env) {
                Ok(result) => {
                    // Suppress output for let-only statements and null values
                    if only_let_stmts || matches!(result, Object::Null) {
                        writeln!(writer)?
                    } else {
                        writeln!(writer, "{result}")?
                    }
                }
                Err(e) => writeln!(writer, "  {e}")?,
            }
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
        // Empty input evaluates to null, which is suppressed in REPL output
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
    fn report_eval_errors() {
        let input = b"5 + true;\n";
        let mut output = Vec::new();
        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let outputs = extract_output(&result);
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("eval error"));
        assert!(outputs[0].contains("invalid infix expression"));
    }
}
