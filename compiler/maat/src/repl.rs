//! Interactive REPL for Maat.
//!
//! Provides two entry points:
//!
//! - [`start_interactive`] -- the production REPL powered by [`rustyline`], with
//!   persistent history, keyword completion, syntax highlighting, and multi-line
//!   input via brace/paren/bracket balancing.
//! - `start` -- a generic `BufRead`/`Write` interface used exclusively by the
//!   test suite.

use std::borrow::Cow;

use maat_ast::{Node, Stmt, fold_constants};
use maat_codegen::{Compiler, SymbolsTable};
use maat_errors::Error;
use maat_eval::{expand_macros, extract_macros};
use maat_lexer::{KEYWORDS, MaatLexer};
use maat_parser::MaatParser;
use maat_parser::reserved::{RESERVED_KEYWORDS, RESERVED_TYPE_NAMES};
use maat_runtime::{Env, Value};
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

/// Builtin macro names available for tab completion.
const MACRO_NAMES: &[&str] = &[
    "assert!",
    "assert_eq!",
    "panic!",
    "print!",
    "println!",
    "todo!",
    "unimplemented!",
];

/// Macro-system identifiers (`quote`/`unquote`).
const MACRO_IDENTS: &[&str] = &["quote", "unquote"];

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
            .chain(RESERVED_KEYWORDS)
            .chain(RESERVED_TYPE_NAMES)
            .chain(MACRO_NAMES)
            .chain(MACRO_IDENTS)
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
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                // Skip string contents so brackets inside strings don't
                // affect depth.
                '"' => {
                    let mut escaped = false;
                    for sc in chars.by_ref() {
                        if escaped {
                            escaped = false;
                        } else if sc == '\\' {
                            escaped = true;
                        } else if sc == '"' {
                            break;
                        }
                    }
                }
                // Skip char literal contents ('a', '\n', etc.).
                '\'' if matches!(chars.peek(), Some(c) if *c != '\'') => {
                    let mut escaped = false;
                    for sc in chars.by_ref() {
                        if escaped {
                            escaped = false;
                        } else if sc == '\\' {
                            escaped = true;
                        } else if sc == '\'' {
                            break;
                        }
                    }
                }
                // Skip line comments.
                '/' if matches!(chars.peek(), Some('/')) => break,
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

/// Returns `true` if `word` is a Maat keyword (lexer keyword, reserved
/// future keyword, or macro-system identifier).
fn is_keyword(word: &str) -> bool {
    KEYWORDS.binary_search(&word).is_ok()
        || RESERVED_KEYWORDS.binary_search(&word).is_ok()
        || MACRO_IDENTS.contains(&word)
}

/// Returns `true` if `word` is a builtin type name.
fn is_type_name(word: &str) -> bool {
    RESERVED_TYPE_NAMES.binary_search(&word).is_ok()
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
                    if next.is_alphanumeric() || next == '_' || next == '!' {
                        end += next.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let word = &line[start..end];
                if is_keyword(word) || MACRO_NAMES.contains(&word) {
                    result.push_str(BOLD_CYAN);
                    result.push_str(word);
                    result.push_str(ANSI_RESET);
                } else if is_type_name(word) {
                    result.push_str(YELLOW);
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
            } else if ch == '\'' {
                let start = i;
                let rest = &line[i + 1..];
                let is_label = rest.starts_with(|c: char| c.is_alphabetic() || c == '_')
                    && !rest.contains('\'');
                if is_label {
                    let mut end = i + 1;
                    while let Some(&(j, next)) = chars.peek() {
                        if next.is_alphanumeric() || next == '_' {
                            end = j + next.len_utf8();
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    result.push_str(BOLD_CYAN);
                    result.push_str(&line[start..end]);
                    result.push_str(ANSI_RESET);
                } else {
                    let mut end = i + 1;
                    let mut escaped = false;
                    for (j, sc) in &mut chars {
                        end = j + sc.len_utf8();
                        if escaped {
                            escaped = false;
                        } else if sc == '\\' {
                            escaped = true;
                        } else if sc == '\'' {
                            break;
                        }
                    }
                    result.push_str(GREEN);
                    result.push_str(&line[start..end]);
                    result.push_str(ANSI_RESET);
                }
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
    let mut constants: Vec<Value> = Vec::new();
    let mut globals: Vec<Value> = Vec::new();
    let macro_env = Env::default();
    let mut type_checker = TypeChecker::new();

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

        let mut parser = MaatParser::new(MaatLexer::new(line));
        let program = parser.parse();
        if !parser.errors().is_empty() {
            for err in parser.errors() {
                diagnostic::report_parse_error(REPL_SOURCE, line, err);
            }
            println!();
            continue;
        }

        let program = extract_macros(program, &macro_env);
        let expanded = expand_macros(Node::Program(program), &macro_env);
        let mut program = match expanded {
            Node::Program(p) => p,
            _ => unreachable!("expand_macros preserves Program variant"),
        };

        type_checker.check_program_mut(&mut program);
        let type_errors = type_checker.drain_errors();
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
            Some(val) if !only_let_stmts && !matches!(val, Value::Unit) => {
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
        let mut constants: Vec<Value> = Vec::new();
        let mut globals: Vec<Value> = Vec::new();
        let macro_env = Env::default();
        let mut type_checker = TypeChecker::new();

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
            let mut parser = MaatParser::new(MaatLexer::new(line));
            let program = parser.parse();
            if !parser.errors().is_empty() {
                for err in parser.errors() {
                    diagnostic::report_parse_error(REPL_SOURCE, line, err);
                }
                writeln!(writer)?;
                continue;
            }
            let program = extract_macros(program, &macro_env);
            let expanded = expand_macros(Node::Program(program), &macro_env);
            let mut program = match expanded {
                Node::Program(p) => p,
                _ => unreachable!("expand_macros preserves Program variant"),
            };
            type_checker.check_program_mut(&mut program);
            let type_errors = type_checker.drain_errors();
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
                Some(val) if !only_let_stmts && !matches!(val, Value::Unit) => {
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

    /// Runs a REPL session and returns the non-empty lines printed after
    /// each `>> ` prompt.
    fn repl(input: &[u8]) -> Vec<String> {
        let mut output = Vec::new();
        start(input, &mut output).expect("REPL failed");
        let raw = String::from_utf8(output).expect("Invalid UTF-8");
        extract_output(&raw)
    }

    #[test]
    fn eval_integer_expression() {
        assert_eq!(repl(b"5 + 10;\n"), vec!["15"]);
    }

    #[test]
    fn eval_boolean_expression() {
        assert_eq!(repl(b"1 < 2;\ntrue == false;\n"), vec!["true", "false"]);
    }

    #[test]
    fn eval_let_statement() {
        assert!(repl(b"let x = 5;\n").is_empty());
    }

    #[test]
    fn eval_multiple_statements() {
        assert_eq!(repl(b"let x = 5;\nlet y = 10;\nx + y;\n"), vec!["15"]);
    }

    #[test]
    fn eval_empty_input() {
        assert!(repl(b"\n").is_empty());
    }

    #[test]
    fn handle_eof() {
        let mut output = Vec::new();
        start(&b""[..], &mut output).expect("REPL failed");
        let raw = String::from_utf8(output).expect("Invalid UTF-8");
        assert!(raw.starts_with(PROMPT));
        assert!(extract_output(&raw).is_empty());
    }

    #[test]
    fn eval_function_application() {
        assert_eq!(
            repl(b"let add = fn(x, y) { x + y; }; add(2, 3);\n"),
            vec!["5"],
        );
    }

    #[test]
    fn eval_closure() {
        assert_eq!(
            repl(
                b"let newAdder = fn(x) { fn(y) { x + y } }; let addTwo = newAdder(2); addTwo(3);\n"
            ),
            vec!["5"],
        );
    }

    #[test]
    fn eval_recursive_fibonacci() {
        let input = b"let fibonacci = fn(x) { if x == 0 { 0 } else if x == 1 { return 1; } else { fibonacci(x - 1) + fibonacci(x - 2) } };\nfibonacci(15);\n";
        assert_eq!(repl(input), vec!["610"]);
    }

    #[test]
    fn globals_persist_across_iterations() {
        assert_eq!(repl(b"let x = 42;\nx * 2;\n"), vec!["84"]);
    }

    #[test]
    fn report_parse_errors() {
        // Parse errors go to stderr via ariadne, so stdout output is empty.
        assert!(repl(b"let x = ;\n").is_empty());
    }

    #[test]
    fn report_type_errors() {
        // `5 + true` is caught by the type checker.
        assert!(repl(b"5 + true;\n").is_empty());
    }

    #[test]
    fn eval_macro_double() {
        assert_eq!(
            repl(b"let double = macro(x) { quote(unquote(x) * 2) };\ndouble(21);\n"),
            vec!["42"],
        );
    }

    #[test]
    fn eval_macro_unless() {
        let input = b"let unless = macro(cond, cons, alt) { quote(if !(unquote(cond)) { unquote(cons); } else { unquote(alt); }) };\nunless(10 > 5, \"not greater\", \"greater\");\n";
        assert_eq!(repl(input), vec!["greater"]);
    }

    #[test]
    fn eval_char_literal() {
        assert_eq!(repl(b"'a';\n"), vec!["a"]);
    }

    #[test]
    fn eval_char_method() {
        assert_eq!(repl(b"'z'.is_alphabetic();\n"), vec!["true"]);
    }

    #[test]
    fn eval_tuple() {
        assert_eq!(repl(b"(1, true, \"hello\");\n"), vec!["(1, true, hello)"]);
    }

    #[test]
    fn eval_tuple_field_access() {
        assert_eq!(repl(b"let t = (10, 20, 30);\nt.1;\n"), vec!["20"]);
    }

    #[test]
    fn eval_vector_higher_order() {
        assert_eq!(
            repl(b"[1, 2, 3, 4].filter(fn(x: i64) -> bool { x % 2 == 0 }).map(fn(x: i64) -> i64 { x * 10 });\n"),
            vec!["[20, 40]"],
        );
    }

    #[test]
    fn eval_map_literal() {
        assert_eq!(
            repl(b"{\"a\": 1, \"b\": 2}.contains_key(\"a\");\n"),
            vec!["true"],
        );
    }

    #[test]
    fn eval_map_method_across_iterations() {
        assert_eq!(
            repl(b"let m = {\"x\": 10, \"y\": 20};\nm.len();\n"),
            vec!["2"],
        );
    }

    #[test]
    fn eval_if_without_parens() {
        assert_eq!(
            repl(b"if 1 < 2 { \"yes\" } else { \"no\" };\n"),
            vec!["yes"],
        );
    }
}
