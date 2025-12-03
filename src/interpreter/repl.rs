use std::io::{self, BufRead, Write};

use crate::lexer::{Lexer, TokenKind};

const PROMPT: &str = ">> ";

/// Starts the REPL (Read-Eval-Print Loop) for interactive Maat sessions.
///
/// The REPL reads lines of source code from the input reader, tokenizes them using
/// the lexer, and prints each token to the output writer. This provides an interactive
/// environment for exploring the language's lexical structure.
///
/// # Type Parameters
///
/// * `R` - A buffered reader implementing [`BufRead`], typically [`std::io::Stdin`].
/// * `W` - A writer implementing [`Write`], typically [`std::io::Stdout`].
///
/// # Parameters
///
/// * `reader` - The input source for reading user commands.
/// * `writer` - The output destination for printing tokens and prompts.
///
/// # Returns
///
/// Returns `Ok(())` when the session ends normally (EOF), or an [`io::Error`] if
/// I/O operations fail.
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
/// 3. Tokenizes the input using the lexer
/// 4. Prints each token in debug format
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
        let mut lexer = Lexer::new(line);

        loop {
            let token = lexer.next_token();
            if token.kind == TokenKind::Eof {
                break;
            }
            writeln!(writer, "{token:?}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tokenization() {
        let input = b"let x = 5;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");

        assert!(result.contains("Token"));
        assert!(result.contains("Let"));
        assert!(result.contains("Ident"));
        assert!(result.contains("Assign"));
        assert!(result.contains("Int"));
        assert!(result.contains("Semicolon"));
    }

    #[test]
    fn multiple_lines() {
        let input = b"let x = 5;\nlet y = 10;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        let prompt_count = result.matches(PROMPT).count();
        assert_eq!(prompt_count, 3);
    }

    #[test]
    fn empty_line() {
        let input = b"\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains(PROMPT));
    }

    #[test]
    fn eof_handling() {
        let input = b"";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains(PROMPT));
    }

    #[test]
    fn complex_expression() {
        let input = b"let add = fn(x, y) { x + y; };\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("Function"));
        assert!(result.contains("LParen"));
        assert!(result.contains("RParen"));
        assert!(result.contains("LBrace"));
        assert!(result.contains("RBrace"));
    }

    #[test]
    fn operators() {
        let input = b"5 + 10 == 15;\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("Plus"));
        assert!(result.contains("Equal"));
    }

    #[test]
    fn whitespace() {
        let input = b"   let   x   =   5   ;   \n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("Let"));
        assert!(result.contains("Ident"));
        assert!(result.contains("Assign"));
        assert!(result.contains("Int"));
        assert!(result.contains("Semicolon"));
    }

    #[test]
    fn invalid_tokens() {
        let input = b"@#$\n";
        let mut output = Vec::new();

        start(&input[..], &mut output).expect("REPL failed");

        let result = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(result.contains("Invalid"));
    }
}
