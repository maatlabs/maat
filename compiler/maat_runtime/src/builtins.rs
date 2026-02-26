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
    "len" => len,
    "print" => print,
    "first" => first,
    "last" => last,
    "rest" => rest,
    "push" => push,
}

pub fn len(args: &[Object]) -> Result<Object> {
    expect_arg_count(args, 1)?;
    match &args[0] {
        Object::Array(arr) => Ok(Object::Usize(arr.len())),
        Object::String(s) => Ok(Object::Usize(s.len())),
        _ => Err(EvalError::Builtin(format!(
            "argument to `len` not supported: {}",
            args[0].type_name()
        ))
        .into()),
    }
}

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

pub fn first(args: &[Object]) -> Result<Object> {
    expect_arg_count(args, 1)?;
    match &args[0] {
        Object::Array(arr) => match arr.first() {
            Some(obj) => Ok(obj.clone()),
            None => Ok(NULL),
        },
        obj => arr_builtin_error(obj, "first"),
    }
}

pub fn last(args: &[Object]) -> Result<Object> {
    expect_arg_count(args, 1)?;
    match &args[0] {
        Object::Array(arr) => match arr.last() {
            Some(obj) => Ok(obj.clone()),
            None => Ok(NULL),
        },
        obj => arr_builtin_error(obj, "last"),
    }
}

pub fn rest(args: &[Object]) -> Result<Object> {
    expect_arg_count(args, 1)?;
    match &args[0] {
        Object::Array(arr) => match arr.split_first() {
            Some((_, tail)) => Ok(Object::Array(tail.to_vec())),
            None => Ok(NULL),
        },
        obj => arr_builtin_error(obj, "rest"),
    }
}

pub fn push(args: &[Object]) -> Result<Object> {
    expect_arg_count(args, 2)?;
    match &args[0] {
        Object::Array(arr) => {
            let mut new_arr = arr.to_vec();
            new_arr.push(args[1].clone());
            Ok(Object::Array(new_arr))
        }
        obj => arr_builtin_error(obj, "push"),
    }
}

fn expect_arg_count(args: &[Object], count: usize) -> Result<()> {
    (args.len() == count).then_some(()).ok_or(
        EvalError::Builtin(format!(
            "wrong number of arguments. got={}, want={count}",
            args.len()
        ))
        .into(),
    )
}

fn arr_builtin_error(obj: &Object, func: &str) -> Result<Object> {
    Err(EvalError::Builtin(format!(
        "argument to `{func}` must be an array, got={}",
        obj.type_name()
    ))
    .into())
}
