use std::io::{self, BufRead, Write};

use crate::{Lexer, Parser};

const PROMPT: &str = ">> ";

/// Starts the REPL (Read-Eval-Print Loop) for interactive Maat sessions.
///
/// # Examples
///
/// ```no_run
/// use std::io;
/// use maat::interpreter::repl;
///
/// let stdin = io::stdin().lock();
/// let mut stdout = io::stdout().lock();
/// repl::start(stdin, &mut stdout).expect("REPL failed");
/// ```
///
/// # Behavior
///
/// The REPL operates in an infinite loop until EOF is encountered:
///
/// 1. Displays the prompt (`>> `)
/// 2. Reads a line of input
/// 3. Parses the input into an AST
/// 4. Reports syntax errors if any, otherwise prints the parsed AST
/// 5. Repeats
///
/// The session terminates when the reader returns EOF (e.g., Ctrl+D on Unix,
/// Ctrl+Z on Windows, or when reading from a closed pipe).
pub fn start<R: BufRead, W: Write>(mut reader: R, writer: &mut W) -> io::Result<()> {
    let mut source = String::new();

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
            writeln!(writer, "{program}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_let_statement() {
        let input = b"let x = 5;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains(PROMPT));
        assert!(result.contains("let x = 5;"));
    }

    #[test]
    fn parse_multiple_statements() {
        let input = b"let x = 5;\nlet y = 10;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert_eq!(result.matches(PROMPT).count(), 3);
        assert!(result.contains("let x = 5;"));
        assert!(result.contains("let y = 10;"));
    }

    #[test]
    fn parse_empty_line() {
        let input = b"\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains(PROMPT));
    }

    #[test]
    fn handle_eof() {
        let input = b"";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains(PROMPT));
    }

    #[test]
    fn parse_function_literal() {
        let input = b"let add = fn(x, y) { x + y; };\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("fn(x, y)"));
        assert!(result.contains("(x + y)"));
    }

    #[test]
    fn parse_expression() {
        let input = b"5 + 10 == 15;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("((5 + 10) == 15)"));
    }

    #[test]
    fn report_syntax_errors() {
        let input = b"let x = ;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("no prefix parse function"));
    }

    #[test]
    fn format_multiple_errors() {
        let input = b"let = fn(x + y);\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("expected next token"));
        assert!(result.contains("\n"));
    }
}
