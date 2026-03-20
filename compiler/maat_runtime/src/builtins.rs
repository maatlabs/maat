use indexmap::{IndexMap, IndexSet};
use maat_errors::{EvalError, Result};

use crate::{BuiltinFn, Hashable, MapObject, NULL, Object};

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
    "print" => print,
    "Array::len" => array_len,
    "Array::first" => array_first,
    "Array::last" => array_last,
    "Array::rest" => array_rest,
    "Array::push" => array_push,
    "Array::join" => array_join,
    "Map::new" => map_new,
    "Map::insert" => map_insert,
    "Map::get" => map_get,
    "Map::contains_key" => map_contains_key,
    "Map::remove" => map_remove,
    "Map::len" => map_len,
    "Map::keys" => map_keys,
    "Map::values" => map_values,
    "Set::new" => set_new,
    "Set::insert" => set_insert,
    "Set::contains" => set_contains,
    "Set::remove" => set_remove,
    "Set::len" => set_len,
    "Set::to_array" => set_to_array,
    "str::len" => str_len,
    "str::trim" => str_trim,
    "str::contains" => str_contains,
    "str::starts_with" => str_starts_with,
    "str::ends_with" => str_ends_with,
    "str::split" => str_split,
    "str::parse_int" => str_parse_int,
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

/// Returns the number of elements in an array. Receiver: `self` at `args[0]`.
fn array_len(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::len", args, 1)?;
    match &args[0] {
        Object::Array(arr) => Ok(Object::Usize(arr.len())),
        other => method_type_error(other, "len", "array"),
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

/// Joins array elements into a string with a separator. Receiver at `args[0]`, separator at `args[1]`.
fn array_join(args: &[Object]) -> Result<Object> {
    expect_arg_count("Array::join", args, 2)?;
    match (&args[0], &args[1]) {
        (Object::Array(arr), Object::Str(sep)) => {
            let joined = arr
                .iter()
                .map(|obj| format!("{obj}"))
                .collect::<Vec<_>>()
                .join(sep);
            Ok(Object::Str(joined))
        }
        (Object::Array(_), other) => Err(EvalError::Builtin(format!(
            "Array::join: separator must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "join", "array"),
    }
}

/// Creates a new empty map.
fn map_new(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::new", args, 0)?;
    Ok(Object::Map(MapObject {
        pairs: IndexMap::new(),
    }))
}

/// Returns a new map with the given key-value pair inserted.
/// Receiver at `args[0]`, key at `args[1]`, value at `args[2]`.
fn map_insert(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::insert", args, 3)?;
    match &args[0] {
        Object::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.insert(key, args[2].clone());
            Ok(Object::Map(MapObject { pairs: new_map }))
        }
        other => method_type_error(other, "insert", "Map"),
    }
}

/// Returns the value associated with the given key, or `null` if not present.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_get(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::get", args, 2)?;
    match &args[0] {
        Object::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(map.pairs.get(&key).cloned().unwrap_or(NULL))
        }
        other => method_type_error(other, "get", "Map"),
    }
}

/// Returns `true` if the map contains the given key.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_contains_key(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::contains_key", args, 2)?;
    match &args[0] {
        Object::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(Object::Bool(map.pairs.contains_key(&key)))
        }
        other => method_type_error(other, "contains_key", "Map"),
    }
}

/// Returns a new map with the given key removed.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_remove(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::remove", args, 2)?;
    match &args[0] {
        Object::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.swap_remove(&key);
            Ok(Object::Map(MapObject { pairs: new_map }))
        }
        other => method_type_error(other, "remove", "Map"),
    }
}

/// Returns the number of key-value pairs in the map.
fn map_len(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::len", args, 1)?;
    match &args[0] {
        Object::Map(map) => Ok(Object::Usize(map.pairs.len())),
        other => method_type_error(other, "len", "Map"),
    }
}

/// Returns an array of all keys in the map, in insertion order.
fn map_keys(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::keys", args, 1)?;
    match &args[0] {
        Object::Map(map) => {
            let keys = map.pairs.keys().map(hashable_to_object).collect();
            Ok(Object::Array(keys))
        }
        other => method_type_error(other, "keys", "Map"),
    }
}

/// Returns an array of all values in the map, in insertion order.
fn map_values(args: &[Object]) -> Result<Object> {
    expect_arg_count("Map::values", args, 1)?;
    match &args[0] {
        Object::Map(map) => {
            let values = map.pairs.values().cloned().collect();
            Ok(Object::Array(values))
        }
        other => method_type_error(other, "values", "Map"),
    }
}

/// Creates a new empty set.
fn set_new(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::new", args, 0)?;
    Ok(Object::Set(IndexSet::new()))
}

/// Returns a new set with the given value inserted. Receiver at `args[0]`, value at `args[1]`.
fn set_insert(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::insert", args, 2)?;
    match &args[0] {
        Object::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set = set.clone();
            new_set.insert(key);
            Ok(Object::Set(new_set))
        }
        other => method_type_error(other, "insert", "Set"),
    }
}

/// Returns `true` if the set contains the given value.
fn set_contains(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::contains", args, 2)?;
    match &args[0] {
        Object::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(Object::Bool(set.contains(&key)))
        }
        other => method_type_error(other, "contains", "Set"),
    }
}

/// Returns a new set with the given value removed.
fn set_remove(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::remove", args, 2)?;
    match &args[0] {
        Object::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set = set.clone();
            new_set.swap_remove(&key);
            Ok(Object::Set(new_set))
        }
        other => method_type_error(other, "remove", "Set"),
    }
}

/// Returns the number of elements in the set.
fn set_len(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::len", args, 1)?;
    match &args[0] {
        Object::Set(set) => Ok(Object::Usize(set.len())),
        other => method_type_error(other, "len", "Set"),
    }
}

/// Converts a set to an array of its elements.
fn set_to_array(args: &[Object]) -> Result<Object> {
    expect_arg_count("Set::to_array", args, 1)?;
    match &args[0] {
        Object::Set(set) => {
            let arr = set
                .iter()
                .map(|h| match h {
                    Hashable::I8(v) => Object::I8(*v),
                    Hashable::I16(v) => Object::I16(*v),
                    Hashable::I32(v) => Object::I32(*v),
                    Hashable::I64(v) => Object::I64(*v),
                    Hashable::I128(v) => Object::I128(*v),
                    Hashable::Isize(v) => Object::Isize(*v),
                    Hashable::U8(v) => Object::U8(*v),
                    Hashable::U16(v) => Object::U16(*v),
                    Hashable::U32(v) => Object::U32(*v),
                    Hashable::U64(v) => Object::U64(*v),
                    Hashable::U128(v) => Object::U128(*v),
                    Hashable::Usize(v) => Object::Usize(*v),
                    Hashable::Bool(v) => Object::Bool(*v),
                    Hashable::Str(v) => Object::Str(v.clone()),
                })
                .collect();
            Ok(Object::Array(arr))
        }
        other => method_type_error(other, "to_array", "Set"),
    }
}

/// Returns the byte length of a string. Receiver: `self` at `args[0]`.
fn str_len(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::len", args, 1)?;
    match &args[0] {
        Object::Str(s) => Ok(Object::Usize(s.len())),
        other => method_type_error(other, "len", "str"),
    }
}

/// Returns a new string with leading and trailing whitespace removed.
fn str_trim(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::trim", args, 1)?;
    match &args[0] {
        Object::Str(s) => Ok(Object::Str(s.trim().to_string())),
        other => method_type_error(other, "trim", "str"),
    }
}

/// Returns `true` if the string contains the given substring.
fn str_contains(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::contains", args, 2)?;
    match (&args[0], &args[1]) {
        (Object::Str(haystack), Object::Str(needle)) => {
            Ok(Object::Bool(haystack.contains(needle.as_str())))
        }
        (Object::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::contains: pattern must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "contains", "str"),
    }
}

/// Returns `true` if the string starts with the given prefix.
fn str_starts_with(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::starts_with", args, 2)?;
    match (&args[0], &args[1]) {
        (Object::Str(s), Object::Str(prefix)) => Ok(Object::Bool(s.starts_with(prefix.as_str()))),
        (Object::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::starts_with: prefix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "starts_with", "str"),
    }
}

/// Returns `true` if the string ends with the given suffix.
fn str_ends_with(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::ends_with", args, 2)?;
    match (&args[0], &args[1]) {
        (Object::Str(s), Object::Str(suffix)) => Ok(Object::Bool(s.ends_with(suffix.as_str()))),
        (Object::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::ends_with: suffix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "ends_with", "str"),
    }
}

/// Splits a string by a delimiter, returning an array of substrings.
fn str_split(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::split", args, 2)?;
    match (&args[0], &args[1]) {
        (Object::Str(s), Object::Str(delim)) => {
            let parts = s
                .split(delim.as_str())
                .map(|part| Object::Str(part.to_string()))
                .collect();
            Ok(Object::Array(parts))
        }
        (Object::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::split: delimiter must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "split", "str"),
    }
}

/// Parses a string as a base-10 integer. Returns `null` on failure.
fn str_parse_int(args: &[Object]) -> Result<Object> {
    expect_arg_count("str::parse_int", args, 1)?;
    match &args[0] {
        Object::Str(s) => Ok(s.trim().parse::<i64>().map_or(NULL, Object::I64)),
        other => method_type_error(other, "parse_int", "str"),
    }
}

/// Converts a `Hashable` back to an `Object`.
fn hashable_to_object(h: &Hashable) -> Object {
    match h {
        Hashable::I8(v) => Object::I8(*v),
        Hashable::I16(v) => Object::I16(*v),
        Hashable::I32(v) => Object::I32(*v),
        Hashable::I64(v) => Object::I64(*v),
        Hashable::I128(v) => Object::I128(*v),
        Hashable::Isize(v) => Object::Isize(*v),
        Hashable::U8(v) => Object::U8(*v),
        Hashable::U16(v) => Object::U16(*v),
        Hashable::U32(v) => Object::U32(*v),
        Hashable::U64(v) => Object::U64(*v),
        Hashable::U128(v) => Object::U128(*v),
        Hashable::Usize(v) => Object::Usize(*v),
        Hashable::Bool(v) => Object::Bool(*v),
        Hashable::Str(v) => Object::Str(v.clone()),
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
