use maat_errors::{EvalError, Result};

use crate::{BuiltinFn, NULL, Object};

/// The name of the `quote` special form for AST quoting.
///
/// Used to capture AST nodes without evaluation, enabling metaprogramming.
/// This is a special form handled directly in the evaluator, not a regular builtin.
pub const QUOTE: &str = "quote";

/// The name of the `unquote` special form for splicing evaluated expressions into quotes.
///
/// Used within `quote` to evaluate and splice expressions into the quoted AST.
/// This is a special form handled during quote evaluation, not a regular builtin.
pub const UNQUOTE: &str = "unquote";

/// Names of built-in types that expose inherent methods.
///
/// The compiler uses this list when resolving method calls: after checking
/// user-defined types in the type registry, it falls back to these names
/// and looks for `{type_name}::{method}` in the symbol table.
pub const BUILTIN_TYPE_NAMES: &[&str] = &["Array", "str"];

/// Declares the builtin function registry.
///
/// Generates three items:
/// - `BUILTINS`: ordered `&[(&str, BuiltinFn)]` array (one entry per name, preserving
///   index order for the compiler/VM pipeline)
/// - `BUILTIN_COUNT`: `usize` constant equal to `BUILTINS.len()`
/// - `get_builtin()`: name-to-function lookup
///
/// Each aliased name occupies its own index in `BUILTINS`, preserving the stable
/// index-based semantics that the compiler and VM depend on.
macro_rules! define_builtins {
    ( $( $( $name:literal )|+ => $func:ident ),* $(,)? ) => {
        /// Ordered registry of built-in functions for the compiler/VM pipeline.
        ///
        /// Each entry maps a fixed index to a `(name, function)` pair. The compiler
        /// resolves builtin identifiers by name and emits `OpGetBuiltin` with the
        /// corresponding index. The VM retrieves the function by index at runtime.
        ///
        /// The ordering must remain stable across compiler and VM sessions.
        pub const BUILTINS: &[(&str, BuiltinFn)] = &[
            $( $( ($name, $func), )+ )*
        ];

        /// The total number of registered builtin entries.
        pub const BUILTIN_COUNT: usize = BUILTINS.len();

        /// Attempts to retrieve a builtin by name. Returns `Some(fn)` or `None`.
        #[inline]
        pub fn get_builtin(name: &str) -> Option<BuiltinFn> {
            match name {
                $( $( $name )|+ => Some($func), )*
                _ => None,
            }
        }
    };
}

define_builtins! {
    "print" => print,
    "Array::len" | "str::len" => builtin_len,
    "Array::first" => array_first,
    "Array::last" => array_last,
    "Array::rest" => array_rest,
    "Array::push" => array_push,
}

/// Prints arguments to stdout, separated by spaces.
pub fn print(args: &[Object]) -> Result<Object> {
    if args.is_empty() {
        println!();
    } else {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                print!(" ");
            }
            print!("{arg}");
        }
        println!();
    }
    Ok(NULL)
}

/// Returns the length of an array or string. Receiver: `self` at `args[0]`.
fn builtin_len(args: &[Object]) -> Result<Object> {
    expect_arg_count("len", args, 1)?;
    match &args[0] {
        Object::Array(arr) => Ok(Object::Usize(arr.len())),
        Object::Str(s) => Ok(Object::Usize(s.len())),
        other => method_type_error(other, "len", "array or str"),
    }
}

/// Returns the first element of an array, or `null` if empty.
fn array_first(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::first", args, 1)?;
    match &args[0] {
        Object::Array(arr) => Ok(arr.first().cloned().unwrap_or(NULL)),
        other => method_type_error(other, "first", "array"),
    }
}

/// Returns the last element of an array, or `null` if empty.
fn array_last(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::last", args, 1)?;
    match &args[0] {
        Object::Array(arr) => Ok(arr.last().cloned().unwrap_or(NULL)),
        other => method_type_error(other, "last", "array"),
    }
}

/// Returns all elements after the first, or `null` if empty.
fn array_rest(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::rest", args, 1)?;
    match &args[0] {
        Object::Array(arr) => arr
            .split_first()
            .map_or(Ok(NULL), |(_, tail)| Ok(Object::Array(tail.to_vec()))),
        other => method_type_error(other, "rest", "array"),
    }
}

/// Returns a new array with `value` appended. Receiver at `args[0]`, value at `args[1]`.
fn array_push(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::push", args, 2)?;
    match &args[0] {
        Object::Array(arr) => {
            let mut new_arr = arr.to_vec();
            new_arr.push(args[1].clone());
            Ok(Object::Array(new_arr))
        }
        other => method_type_error(other, "push", "array"),
    }
}

fn expect_arg_count(method: &str, args: &[Object], count: usize) -> Result<()> {
    (args.len() == count).then_some(()).ok_or(
        EvalError::Builtin(format!(
            "{method}: wrong number of arguments. got={}, want={count}",
            args.len()
        ))
        .into(),
    )
}

fn method_type_error(obj: &Object, method: &str, expected_type: &str) -> Result<Object> {
    Err(EvalError::Builtin(format!(
        "cannot call `{method}` on {}, expected {expected_type}",
        obj.type_name()
    ))
    .into())
}
