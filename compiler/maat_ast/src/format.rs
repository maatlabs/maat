//! String formatting utilities

use core::fmt;

use crate::{GenericParam, TypeExpr, TypedParam};

/// A segment of a parsed format string.
pub enum FmtSegment {
    Literal(String),
    Arg,
    Capture(String),
}

/// Unescapes a string literal by processing escape sequences.
pub fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some('\'') => result.push('\''),
                Some('u') => {
                    if let Some(c) = parse_unicode_escape(&mut chars) {
                        result.push(c);
                    } else {
                        result.push_str("\\u");
                    }
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Converts a character literal's inner text (without surrounding quotes)
/// into the represented character.
pub fn unescape_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let result = match chars.next()? {
        '\\' => match chars.next()? {
            '\\' => '\\',
            '\'' => '\'',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '"' => '"',
            'u' => parse_unicode_escape(&mut chars)?,
            _ => return None,
        },
        c => c,
    };
    if chars.next().is_some() {
        return None;
    }
    Some(result)
}

/// Parses the `{XXXX}` portion of a `\u{XXXX}` Unicode escape sequence.
fn parse_unicode_escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    if chars.next() != Some('{') {
        return None;
    }
    let mut hex = String::new();
    for c in chars.by_ref() {
        if c == '}' {
            let code = u32::from_str_radix(&hex, 16).ok()?;
            return char::from_u32(code);
        }
        if hex.len() >= 6 {
            return None;
        }
        hex.push(c);
    }
    None
}

/// Writes a comma-separated list of displayable items to the formatter.
pub fn write_comma_separated<I, T>(f: &mut fmt::Formatter<'_>, iter: I) -> fmt::Result
where
    I: IntoIterator<Item = T>,
    T: fmt::Display,
{
    let mut iter = iter.into_iter();
    if let Some(first) = iter.next() {
        write!(f, "{first}")?;
        for item in iter {
            write!(f, ", {item}")?;
        }
    }
    Ok(())
}

/// Writes items separated by a custom delimiter `sep`.
pub fn write_separated_with<I, T>(f: &mut fmt::Formatter<'_>, iter: I, sep: &str) -> fmt::Result
where
    I: IntoIterator<Item = T>,
    T: fmt::Display,
{
    let mut iter = iter.into_iter();
    if let Some(first) = iter.next() {
        write!(f, "{first}")?;
        for item in iter {
            f.write_str(sep)?;
            write!(f, "{item}")?;
        }
    }
    Ok(())
}

/// Writes a generic parameter list as `<T, U: Bound>`, or nothing if empty.
pub fn write_generic_params(f: &mut fmt::Formatter<'_>, params: &[GenericParam]) -> fmt::Result {
    if !params.is_empty() {
        f.write_str("<")?;
        write_comma_separated(f, params)?;
        f.write_str(">")?;
    }
    Ok(())
}

/// Writes a typed parameter list to the formatter.
pub fn write_params(f: &mut fmt::Formatter<'_>, params: &[TypedParam]) -> fmt::Result {
    write_comma_separated(f, params)
}

/// Writes ` -> T` if a return type is present.
pub fn write_return_type(f: &mut fmt::Formatter<'_>, ret: &Option<TypeExpr>) -> fmt::Result {
    if let Some(ty) = ret {
        write!(f, " -> {ty}")?;
    }
    Ok(())
}

/// Returns `"pub "` if the item is public, empty string otherwise.
#[inline]
pub fn visibility_modifier(vis: bool) -> &'static str {
    if vis { "pub " } else { "" }
}

/// Renders documentation comment lines (`///`) above the item.
pub fn fmt_doc_comment(f: &mut fmt::Formatter<'_>, doc: &Option<String>) -> fmt::Result {
    if let Some(text) = doc {
        for line in text.lines() {
            writeln!(f, "///{line}")?;
        }
    }
    Ok(())
}

/// Parses a format string into a sequence of literal, positional, and capture segments.
pub fn parse_format_string(fmt: &str) -> Vec<FmtSegment> {
    let mut segments = Vec::new();
    let mut buf = String::new();
    let mut chars = fmt.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    buf.push('{');
                } else if chars.peek() == Some(&'}') {
                    chars.next();
                    if !buf.is_empty() {
                        segments.push(FmtSegment::Literal(std::mem::take(&mut buf)));
                    }
                    segments.push(FmtSegment::Arg);
                } else {
                    // Try to parse `{identifier}`.
                    let mut name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '}' {
                            break;
                        }
                        name.push(c);
                        chars.next();
                    }
                    if chars.peek() == Some(&'}') && is_identifier(&name) {
                        chars.next();
                        if !buf.is_empty() {
                            segments.push(FmtSegment::Literal(std::mem::take(&mut buf)));
                        }
                        segments.push(FmtSegment::Capture(name));
                    } else {
                        // Not a valid capture; emit as literal text.
                        buf.push('{');
                        buf.push_str(&name);
                    }
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    buf.push('}');
                } else {
                    buf.push('}');
                }
            }
            _ => buf.push(ch),
        }
    }

    if !buf.is_empty() {
        segments.push(FmtSegment::Literal(buf));
    }
    segments
}

/// Returns `true` if `s` is a valid Maat identifier (`[a-zA-Z_][a-zA-Z0-9_]*`).
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}
