use indexmap::{IndexMap, IndexSet};
use maat_errors::{Error, EvalError, Result};

use crate::{BuiltinFn, EnumVariantVal, Hashable, Integer, MapVal, NULL, Value};

/// Type registry index for `Option`.
const OPTION_TYPE_INDEX: u16 = 0;
/// Variant tag for `Some` within `Option`.
const SOME_TAG: u16 = 0;
/// Variant tag for `None` within `Option`.
const NONE_TAG: u16 = 1;

/// Type registry index for `Result`.
const RESULT_TYPE_INDEX: u16 = 1;
/// Variant tag for `Ok` within `Result`.
const OK_TAG: u16 = 0;
/// Variant tag for `Err` within `Result`.
const ERR_TAG: u16 = 1;

/// Type registry index for `ParseIntError`.
const PARSE_INT_ERROR_TYPE_INDEX: u16 = 2;

/// Describes why a string-to-integer parse operation failed.
///
/// This is a language-level type, registered as a builtin enum in the
/// type system so that user code can pattern-match on `Result<T, ParseIntError>`
/// values returned by the `str::parse_*` family of methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseIntError {
    /// The input string was empty (or contained only whitespace).
    Empty,
    /// The input contained a character that is not a valid digit.
    InvalidDigit,
    /// The parsed value exceeds the representable range of the target type.
    Overflow,
}

impl ParseIntError {
    /// Maps a [`std::num::IntErrorKind`] to the corresponding Maat variant.
    pub fn from_std(err: &std::num::ParseIntError) -> Self {
        match err.kind() {
            std::num::IntErrorKind::Empty => Self::Empty,
            std::num::IntErrorKind::InvalidDigit => Self::InvalidDigit,
            std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow => {
                Self::Overflow
            }
            _ => Self::InvalidDigit,
        }
    }

    /// Returns the variant tag used in the runtime type registry.
    ///
    /// Must match the order in `builtin_type_registry` in `maat_codegen`.
    pub const fn tag(self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::InvalidDigit => 1,
            Self::Overflow => 2,
        }
    }
}

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
    "__print_str" => __print_str,
    "__print_str_ln" => __print_str_ln,
    "__to_string" => __to_string,
    "__panic" => __panic,

    "Vector::len" => vector_len,
    "Vector::first" => vector_first,
    "Vector::last" => vector_last,
    "Vector::rest" => vector_rest,
    "Vector::push" => vector_push,
    "Vector::new" => vector_new,
    "Vector::join" => vector_join,

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
    "Set::to_vector" => set_to_vector,

    "str::len" => str_len,
    "str::trim" => str_trim,
    "str::contains" => str_contains,
    "str::starts_with" => str_starts_with,
    "str::ends_with" => str_ends_with,
    "str::split" => str_split,
    "str::parse_int" | "str::parse_i64" => str_parse_i64,
    "str::parse_i8" => str_parse_i8,
    "str::parse_i16" => str_parse_i16,
    "str::parse_i32" => str_parse_i32,
    "str::parse_i128" => str_parse_i128,
    "str::parse_u8" => str_parse_u8,
    "str::parse_u16" => str_parse_u16,
    "str::parse_u32" => str_parse_u32,
    "str::parse_u64" => str_parse_u64,
    "str::parse_u128" => str_parse_u128,
    "str::parse_usize" => str_parse_usize,

    "Option::unwrap" => option_unwrap,
    "Option::unwrap_or" => option_unwrap_or,
    "Option::is_some" => option_is_some,
    "Option::is_none" => option_is_none,

    "Result::unwrap" => result_unwrap,
    "Result::unwrap_or" => result_unwrap_or,
    "Result::is_ok" => result_is_ok,
    "Result::is_err" => result_is_err,

    "i16::from" => i16_from,
    "i32::from" => i32_from,
    "i64::from" => i64_from,
    "i128::from" => i128_from,
    "u16::from" => u16_from,
    "u32::from" => u32_from,
    "u64::from" => u64_from,
    "u128::from" => u128_from,

    "i8::default" => i8_default,
    "i16::default" => i16_default,
    "i32::default" => i32_default,
    "i64::default" => i64_default,
    "i128::default" => i128_default,
    "u8::default" => u8_default,
    "u16::default" => u16_default,
    "u32::default" => u32_default,
    "u64::default" => u64_default,
    "u128::default" => u128_default,
    "usize::default" => usize_default,
    "isize::default" => isize_default,
    "bool::default" => bool_default,
    "str::default" => str_default,

    "cmp::min" => cmp_min,
    "cmp::max" => cmp_max,
    "cmp::clamp" => cmp_clamp,
}

/// Prints a single value to stdout without a trailing newline.
///
/// Internal builtin emitted by `print!` and `println!` macro expansion.
fn __print_str(args: &[Value]) -> Result<Value> {
    expect_arg_count("__print_str", args, 1)?;
    print!("{}", args[0]);
    Ok(NULL)
}

/// Prints a single value to stdout followed by a newline.
///
/// Internal builtin emitted by `println!()`.
fn __print_str_ln(args: &[Value]) -> Result<Value> {
    expect_arg_count("__print_str_ln", args, 1)?;
    println!("{}", args[0]);
    Ok(NULL)
}

/// Converts any value to its string representation.
///
/// Internal builtin emitted by `print!` / `println!` format string expansion
/// for `{}` placeholder interpolation.
fn __to_string(args: &[Value]) -> Result<Value> {
    expect_arg_count("__to_string", args, 1)?;
    Ok(Value::Str(format!("{}", args[0])))
}

/// Terminates execution with a runtime error.
///
/// Internal builtin emitted by `assert!` and `assert_eq!` macro expansion.
fn __panic(args: &[Value]) -> Result<Value> {
    expect_arg_count("__panic", args, 1)?;
    Err(EvalError::Builtin(format!("{}", args[0])).into())
}

/// Returns the number of elements in a vec. Receiver: `self` at `args[0]`.
fn vector_len(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::len", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(Value::Integer(Integer::Usize(arr.len()))),
        other => method_type_error(other, "len", "Vector"),
    }
}

/// Returns the first element of a vec, or `null` if empty.
fn vector_first(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::first", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(arr.first().cloned().unwrap_or(NULL)),
        other => method_type_error(other, "first", "Vector"),
    }
}

/// Returns the last element of a vec, or `null` if empty.
fn vector_last(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::last", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(arr.last().cloned().unwrap_or(NULL)),
        other => method_type_error(other, "last", "Vector"),
    }
}

/// Returns all elements after the first, or `null` if empty.
fn vector_rest(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::rest", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => arr
            .split_first()
            .map_or(Ok(NULL), |(_, tail)| Ok(Value::Vector(tail.to_vec()))),
        other => method_type_error(other, "rest", "Vector"),
    }
}

/// Returns a new vec with `value` appended. Receiver at `args[0]`, value at `args[1]`.
fn vector_push(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::push", args, 2)?;
    match &args[0] {
        Value::Vector(arr) => {
            let mut new_arr = arr.to_vec();
            new_arr.push(args[1].clone());
            Ok(Value::Vector(new_arr))
        }
        other => method_type_error(other, "push", "Vector"),
    }
}

/// Joins vec elements into a string with a separator. Receiver at `args[0]`, separator at `args[1]`.
fn vector_join(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::join", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(arr), Value::Str(sep)) => {
            let joined = arr
                .iter()
                .map(|val| format!("{val}"))
                .collect::<Vec<_>>()
                .join(sep);
            Ok(Value::Str(joined))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::join: separator must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "join", "Vector"),
    }
}

/// Creates a new empty vector.
fn vector_new(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::new", args, 0)?;
    Ok(Value::Vector(Vec::new()))
}

/// Creates a new empty map.
fn map_new(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::new", args, 0)?;
    Ok(Value::Map(MapVal {
        pairs: IndexMap::new(),
    }))
}

/// Returns a new map with the given key-value pair inserted.
/// Receiver at `args[0]`, key at `args[1]`, value at `args[2]`.
fn map_insert(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::insert", args, 3)?;
    match &args[0] {
        Value::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.insert(key, args[2].clone());
            Ok(Value::Map(MapVal { pairs: new_map }))
        }
        other => method_type_error(other, "insert", "Map"),
    }
}

/// Returns the value associated with the given key, or `null` if not present.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_get(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::get", args, 2)?;
    match &args[0] {
        Value::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(map.pairs.get(&key).cloned().unwrap_or(NULL))
        }
        other => method_type_error(other, "get", "Map"),
    }
}

/// Returns `true` if the map contains the given key.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_contains_key(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::contains_key", args, 2)?;
    match &args[0] {
        Value::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(Value::Bool(map.pairs.contains_key(&key)))
        }
        other => method_type_error(other, "contains_key", "Map"),
    }
}

/// Returns a new map with the given key removed.
/// Receiver at `args[0]`, key at `args[1]`.
fn map_remove(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::remove", args, 2)?;
    match &args[0] {
        Value::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.swap_remove(&key);
            Ok(Value::Map(MapVal { pairs: new_map }))
        }
        other => method_type_error(other, "remove", "Map"),
    }
}

/// Returns the number of key-value pairs in the map.
fn map_len(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::len", args, 1)?;
    match &args[0] {
        Value::Map(map) => Ok(Value::Integer(Integer::Usize(map.pairs.len()))),
        other => method_type_error(other, "len", "Map"),
    }
}

/// Returns a vector of all keys in the map, in insertion order.
fn map_keys(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::keys", args, 1)?;
    match &args[0] {
        Value::Map(map) => {
            let keys = map.pairs.keys().map(hashable_to_object).collect();
            Ok(Value::Vector(keys))
        }
        other => method_type_error(other, "keys", "Map"),
    }
}

/// Returns a vector of all values in the map, in insertion order.
fn map_values(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::values", args, 1)?;
    match &args[0] {
        Value::Map(map) => {
            let values = map.pairs.values().cloned().collect();
            Ok(Value::Vector(values))
        }
        other => method_type_error(other, "values", "Map"),
    }
}

/// Creates a new empty set.
fn set_new(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::new", args, 0)?;
    Ok(Value::Set(IndexSet::new()))
}

/// Returns a new set with the given value inserted. Receiver at `args[0]`, value at `args[1]`.
fn set_insert(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::insert", args, 2)?;
    match &args[0] {
        Value::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set = set.clone();
            new_set.insert(key);
            Ok(Value::Set(new_set))
        }
        other => method_type_error(other, "insert", "Set"),
    }
}

/// Returns `true` if the set contains the given value.
fn set_contains(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::contains", args, 2)?;
    match &args[0] {
        Value::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(Value::Bool(set.contains(&key)))
        }
        other => method_type_error(other, "contains", "Set"),
    }
}

/// Returns a new set with the given value removed.
fn set_remove(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::remove", args, 2)?;
    match &args[0] {
        Value::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set = set.clone();
            new_set.swap_remove(&key);
            Ok(Value::Set(new_set))
        }
        other => method_type_error(other, "remove", "Set"),
    }
}

/// Returns the number of elements in the set.
fn set_len(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::len", args, 1)?;
    match &args[0] {
        Value::Set(set) => Ok(Value::Integer(Integer::Usize(set.len()))),
        other => method_type_error(other, "len", "Set"),
    }
}

/// Converts a set to a vec of its elements.
fn set_to_vector(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::to_vector", args, 1)?;
    match &args[0] {
        Value::Set(set) => {
            let arr = set
                .iter()
                .map(|h| match h {
                    Hashable::Integer(v) => Value::Integer(*v),
                    Hashable::Bool(v) => Value::Bool(*v),
                    Hashable::Str(v) => Value::Str(v.clone()),
                })
                .collect();
            Ok(Value::Vector(arr))
        }
        other => method_type_error(other, "to_vector", "Set"),
    }
}

/// Returns the byte length of a string. Receiver: `self` at `args[0]`.
fn str_len(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::len", args, 1)?;
    match &args[0] {
        Value::Str(s) => Ok(Value::Integer(Integer::Usize(s.len()))),
        other => method_type_error(other, "len", "str"),
    }
}

/// Returns a new string with leading and trailing whitespace removed.
fn str_trim(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::trim", args, 1)?;
    match &args[0] {
        Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
        other => method_type_error(other, "trim", "str"),
    }
}

/// Returns `true` if the string contains the given substring.
fn str_contains(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::contains", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(haystack), Value::Str(needle)) => {
            Ok(Value::Bool(haystack.contains(needle.as_str())))
        }
        (Value::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::contains: pattern must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "contains", "str"),
    }
}

/// Returns `true` if the string starts with the given prefix.
fn str_starts_with(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::starts_with", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(s), Value::Str(prefix)) => Ok(Value::Bool(s.starts_with(prefix.as_str()))),
        (Value::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::starts_with: prefix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "starts_with", "str"),
    }
}

/// Returns `true` if the string ends with the given suffix.
fn str_ends_with(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::ends_with", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(s), Value::Str(suffix)) => Ok(Value::Bool(s.ends_with(suffix.as_str()))),
        (Value::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::ends_with: suffix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "ends_with", "str"),
    }
}

/// Splits a string by a delimiter, returning a vector of substrings.
fn str_split(args: &[Value]) -> Result<Value> {
    expect_arg_count("str::split", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(s), Value::Str(delim)) => {
            let parts = s
                .split(delim.as_str())
                .map(|part| Value::Str(part.to_string()))
                .collect();
            Ok(Value::Vector(parts))
        }
        (Value::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::split: delimiter must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "split", "str"),
    }
}

/// Wraps a successfully parsed integer in `Ok(value)`.
fn parse_ok(value: Value) -> Value {
    Value::EnumVariant(EnumVariantVal {
        type_index: RESULT_TYPE_INDEX,
        tag: 0,
        fields: vec![value],
    })
}

/// Wraps a parse failure in `Err(ParseIntError::variant)`.
fn parse_err(error: ParseIntError) -> Value {
    Value::EnumVariant(EnumVariantVal {
        type_index: RESULT_TYPE_INDEX,
        tag: 1,
        fields: vec![Value::EnumVariant(EnumVariantVal {
            type_index: PARSE_INT_ERROR_TYPE_INDEX,
            tag: error.tag(),
            fields: vec![],
        })],
    })
}

/// Generates a typed string-to-integer parse function.
/// Each generated function trims the input string, attempts to parse it as the
/// target integer type.
///
/// Returns `Ok(value)` on success or `Err(ParseIntError)` on failure.
macro_rules! define_str_parse {
    ($fn_name:ident, $method:literal, $rust_ty:ty, $variant:ident) => {
        fn $fn_name(args: &[Value]) -> Result<Value> {
            expect_arg_count(concat!("str::", $method), args, 1)?;
            match &args[0] {
                Value::Str(s) => Ok(s.trim().parse::<$rust_ty>().map_or_else(
                    |e| parse_err(ParseIntError::from_std(&e)),
                    |v| parse_ok(Value::Integer(Integer::$variant(v))),
                )),
                other => method_type_error(other, $method, "str"),
            }
        }
    };
}

define_str_parse!(str_parse_i8, "parse_i8", i8, I8);
define_str_parse!(str_parse_i16, "parse_i16", i16, I16);
define_str_parse!(str_parse_i32, "parse_i32", i32, I32);
define_str_parse!(str_parse_i64, "parse_i64", i64, I64);
define_str_parse!(str_parse_i128, "parse_i128", i128, I128);
define_str_parse!(str_parse_u8, "parse_u8", u8, U8);
define_str_parse!(str_parse_u16, "parse_u16", u16, U16);
define_str_parse!(str_parse_u32, "parse_u32", u32, U32);
define_str_parse!(str_parse_u64, "parse_u64", u64, U64);
define_str_parse!(str_parse_u128, "parse_u128", u128, U128);
define_str_parse!(str_parse_usize, "parse_usize", usize, Usize);

/// Extracts the inner value from `Some(x)`, or produces a runtime error on `None`.
fn option_unwrap(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::unwrap", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            Ok(v.fields[0].clone())
        }
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => Err(
            EvalError::Builtin("called `Option::unwrap()` on a `None` value".to_string()).into(),
        ),
        other => method_type_error(other, "unwrap", "Option"),
    }
}

/// Extracts the inner value from `Some(x)`, or returns the provided default on `None`.
fn option_unwrap_or(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::unwrap_or", args, 2)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            Ok(v.fields[0].clone())
        }
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            Ok(args[1].clone())
        }
        other => method_type_error(other, "unwrap_or", "Option"),
    }
}

/// Returns `true` if the `Option` is `Some`.
fn option_is_some(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::is_some", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX => {
            Ok(Value::Bool(v.tag == SOME_TAG))
        }
        other => method_type_error(other, "is_some", "Option"),
    }
}

/// Returns `true` if the `Option` is `None`.
fn option_is_none(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::is_none", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX => {
            Ok(Value::Bool(v.tag == NONE_TAG))
        }
        other => method_type_error(other, "is_none", "Option"),
    }
}

/// Extracts the `Ok` value, or produces a runtime error on `Err`.
fn result_unwrap(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::unwrap", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            Ok(v.fields[0].clone())
        }
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            let err_val = v
                .fields
                .first()
                .map_or("unknown".to_string(), |e| format!("{e}"));
            Err(EvalError::Builtin(format!(
                "called `Result::unwrap()` on an `Err` value: {err_val}"
            ))
            .into())
        }
        other => method_type_error(other, "unwrap", "Result"),
    }
}

/// Extracts the `Ok` value, or returns the provided default on `Err`.
fn result_unwrap_or(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::unwrap_or", args, 2)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            Ok(v.fields[0].clone())
        }
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            Ok(args[1].clone())
        }
        other => method_type_error(other, "unwrap_or", "Result"),
    }
}

/// Returns `true` if the `Result` is `Ok`.
fn result_is_ok(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::is_ok", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX => {
            Ok(Value::Bool(v.tag == OK_TAG))
        }
        other => method_type_error(other, "is_ok", "Result"),
    }
}

/// Returns `true` if the `Result` is `Err`.
fn result_is_err(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::is_err", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX => {
            Ok(Value::Bool(v.tag == ERR_TAG))
        }
        other => method_type_error(other, "is_err", "Result"),
    }
}

/// Converts a `Hashable` back to an `Value`.
fn hashable_to_object(h: &Hashable) -> Value {
    match h {
        Hashable::Integer(v) => Value::Integer(*v),
        Hashable::Bool(v) => Value::Bool(*v),
        Hashable::Str(v) => Value::Str(v.clone()),
    }
}

fn expect_arg_count(method: &str, args: &[Value], count: usize) -> Result<()> {
    (args.len() == count).then_some(()).ok_or(
        EvalError::Builtin(format!(
            "{method}: wrong number of arguments. got={}, want={count}",
            args.len()
        ))
        .into(),
    )
}

fn method_type_error(val: &Value, method: &str, expected_type: &str) -> Result<Value> {
    Err(EvalError::Builtin(format!(
        "cannot call `{method}` on {}, expected {expected_type}",
        val.type_name()
    ))
    .into())
}

/// Generates a lossless numeric widening conversion builtin.
///
/// Each generated function extracts the source integer, widens it via
/// `to_i128()`, range-checks against the target, and wraps the result.
macro_rules! define_from_signed {
    ($fn_name:ident, $target_name:expr, $target_ty:ty, $variant:ident) => {
        fn $fn_name(args: &[Value]) -> Result<Value> {
            expect_arg_count($target_name, args, 1)?;
            match &args[0] {
                Value::Integer(n) => {
                    let wide = n
                        .to_i128()
                        .ok_or_else(|| conversion_error(n, $target_name))?;
                    let val = <$target_ty>::try_from(wide)
                        .map_err(|_| conversion_error(n, $target_name))?;
                    Ok(Value::Integer(Integer::$variant(val)))
                }
                other => Err(EvalError::Builtin(format!(
                    "{}: expected integer, got {}",
                    $target_name,
                    other.type_name()
                ))
                .into()),
            }
        }
    };
}

macro_rules! define_from_unsigned {
    ($fn_name:ident, $target_name:expr, $target_ty:ty, $variant:ident) => {
        fn $fn_name(args: &[Value]) -> Result<Value> {
            expect_arg_count($target_name, args, 1)?;
            match &args[0] {
                Value::Integer(n) => {
                    let wide = n
                        .to_i128()
                        .ok_or_else(|| conversion_error(n, $target_name))?;
                    let val = <$target_ty>::try_from(wide)
                        .map_err(|_| conversion_error(n, $target_name))?;
                    Ok(Value::Integer(Integer::$variant(val)))
                }
                other => Err(EvalError::Builtin(format!(
                    "{}: expected integer, got {}",
                    $target_name,
                    other.type_name()
                ))
                .into()),
            }
        }
    };
}

define_from_signed!(i16_from, "i16::from", i16, I16);
define_from_signed!(i32_from, "i32::from", i32, I32);
define_from_signed!(i64_from, "i64::from", i64, I64);
define_from_signed!(i128_from, "i128::from", i128, I128);
define_from_unsigned!(u16_from, "u16::from", u16, U16);
define_from_unsigned!(u32_from, "u32::from", u32, U32);
define_from_unsigned!(u64_from, "u64::from", u64, U64);
define_from_unsigned!(u128_from, "u128::from", u128, U128);

fn conversion_error(n: &Integer, target: &str) -> Error {
    EvalError::Builtin(format!("{target}: value {n} out of range")).into()
}

/// Generates a zero-arg `Type::default()` builtin returning the zero value.
macro_rules! define_default {
    ($fn_name:ident, $name:expr, $value:expr) => {
        fn $fn_name(args: &[Value]) -> Result<Value> {
            expect_arg_count($name, args, 0)?;
            Ok($value)
        }
    };
}

define_default!(i8_default, "i8::default", Value::Integer(Integer::I8(0)));
define_default!(i16_default, "i16::default", Value::Integer(Integer::I16(0)));
define_default!(i32_default, "i32::default", Value::Integer(Integer::I32(0)));
define_default!(i64_default, "i64::default", Value::Integer(Integer::I64(0)));
define_default!(
    i128_default,
    "i128::default",
    Value::Integer(Integer::I128(0))
);
define_default!(u8_default, "u8::default", Value::Integer(Integer::U8(0)));
define_default!(u16_default, "u16::default", Value::Integer(Integer::U16(0)));
define_default!(u32_default, "u32::default", Value::Integer(Integer::U32(0)));
define_default!(u64_default, "u64::default", Value::Integer(Integer::U64(0)));
define_default!(
    u128_default,
    "u128::default",
    Value::Integer(Integer::U128(0))
);
define_default!(
    usize_default,
    "usize::default",
    Value::Integer(Integer::Usize(0))
);
define_default!(
    isize_default,
    "isize::default",
    Value::Integer(Integer::Isize(0))
);
define_default!(bool_default, "bool::default", Value::Bool(false));
define_default!(str_default, "str::default", Value::Str(String::new()));

/// Compares two integers of the same type, returning the smaller.
fn cmp_min(args: &[Value]) -> Result<Value> {
    expect_arg_count("cmp::min", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Integer(a), Value::Integer(b)) => {
            let ord = a
                .to_i128()
                .zip(b.to_i128())
                .ok_or_else(|| cmp_error("min"))?;
            Ok(if ord.0 <= ord.1 {
                args[0].clone()
            } else {
                args[1].clone()
            })
        }
        _ => Err(cmp_error("min")),
    }
}

/// Compares two integers of the same type, returning the larger.
fn cmp_max(args: &[Value]) -> Result<Value> {
    expect_arg_count("cmp::max", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Integer(a), Value::Integer(b)) => {
            let ord = a
                .to_i128()
                .zip(b.to_i128())
                .ok_or_else(|| cmp_error("max"))?;
            Ok(if ord.0 >= ord.1 {
                args[0].clone()
            } else {
                args[1].clone()
            })
        }
        _ => Err(cmp_error("max")),
    }
}

/// Restricts a value to the range `[min, max]`.
fn cmp_clamp(args: &[Value]) -> Result<Value> {
    expect_arg_count("cmp::clamp", args, 3)?;
    match (&args[0], &args[1], &args[2]) {
        (Value::Integer(val), Value::Integer(lo), Value::Integer(hi)) => {
            let (v, l, h) = val
                .to_i128()
                .zip(lo.to_i128())
                .zip(hi.to_i128())
                .map(|((v, l), h)| (v, l, h))
                .ok_or_else(|| cmp_error("clamp"))?;
            Ok(if v < l {
                args[1].clone()
            } else if v > h {
                args[2].clone()
            } else {
                args[0].clone()
            })
        }
        _ => Err(cmp_error("clamp")),
    }
}

fn cmp_error(name: &str) -> Error {
    EvalError::Builtin(format!("cmp::{name}: expected integer arguments")).into()
}
