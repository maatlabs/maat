//! Terminal diagnostics for compile-time and runtime errors.
//!
//! Uses [`ariadne`] to render source snippets with underlined spans and
//! contextual messages. Falls back to plain `eprintln!` output when no
//! source span is available.

use ariadne::{Color, Label, Report, ReportKind, Source};
use maat_errors::{CompileError, Error, ParseError, TypeError, VmError};
use maat_span::Span;

/// Routes an [`Error`] to the appropriate diagnostic reporter.
pub fn report_error(path: &str, source: &str, error: &Error) {
    match error {
        Error::Parse(e) => report_parse_error(path, source, e),
        Error::Compile(e) => report_compile_error(path, source, e),
        Error::Type(e) => report_type_error(path, source, e),
        Error::Vm(e) => report_vm_error(path, source, e),
        _ => eprintln!("{path}: {error}"),
    }
}

/// Renders a parse error with a source snippet to stderr.
pub fn report_parse_error(path: &str, source: &str, error: &ParseError) {
    let range = byte_range_to_char_range(source, error.span);

    Report::build(ReportKind::Error, (path, range.clone()))
        .with_message("parse error")
        .with_label(
            Label::new((path, range))
                .with_message(&error.message)
                .with_color(Color::Red),
        )
        .finish()
        .eprint((path, Source::from(source)))
        .ok();
}

/// Renders a compile error with a source snippet to stderr.
///
/// If the error carries no span, falls back to a plain message.
pub fn report_compile_error(path: &str, source: &str, error: &CompileError) {
    match error.span {
        Some(span) => {
            let range = byte_range_to_char_range(source, span);

            Report::build(ReportKind::Error, (path, range.clone()))
                .with_message("compile error")
                .with_label(
                    Label::new((path, range))
                        .with_message(error.kind.to_string())
                        .with_color(Color::Red),
                )
                .finish()
                .eprint((path, Source::from(source)))
                .ok();
        }
        None => eprintln!("{path}: compile error: {}", error.kind),
    }
}

/// Renders a VM runtime error with a source snippet to stderr.
///
/// If the error carries no span, falls back to a plain message.
pub fn report_vm_error(path: &str, source: &str, error: &VmError) {
    match error.span {
        Some(span) => {
            let range = byte_range_to_char_range(source, span);

            Report::build(ReportKind::Error, (path, range.clone()))
                .with_message("runtime error")
                .with_label(
                    Label::new((path, range))
                        .with_message(&error.message)
                        .with_color(Color::Red),
                )
                .finish()
                .eprint((path, Source::from(source)))
                .ok();
        }
        None => eprintln!("{path}: vm error: {}", error.message),
    }
}

/// Renders a type error with a source snippet to stderr.
pub fn report_type_error(path: &str, source: &str, error: &TypeError) {
    let range = byte_range_to_char_range(source, error.span);

    Report::build(ReportKind::Error, (path, range.clone()))
        .with_message("type error")
        .with_label(
            Label::new((path, range))
                .with_message(error.kind.to_string())
                .with_color(Color::Red),
        )
        .finish()
        .eprint((path, Source::from(source)))
        .ok();
}

/// Converts a byte-offset [`Span`] to a character-offset range suitable for
/// `ariadne`.
///
/// Ariadne operates on character indices rather than byte offsets. This
/// function walks the source string to translate byte positions into their
/// corresponding character positions.
fn byte_range_to_char_range(source: &str, span: Span) -> std::ops::Range<usize> {
    let start = source[..span.start.min(source.len())].chars().count();
    let end = source[..span.end.min(source.len())].chars().count();
    start..end.max(start + 1)
}
