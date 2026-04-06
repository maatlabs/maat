use indexmap::{IndexMap, IndexSet};
use maat_errors::{Error, EvalError, Result};

use crate::{BuiltinFn, EnumVariantVal, Felt, Hashable, Integer, Map, Set, UNIT, Value};

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
    "__str_concat" => __str_concat,
    "__panic" => __panic,

    "Vector::len" => vector_len,
    "Vector::first" => vector_first,
    "Vector::last" => vector_last,
    "Vector::split_first" => vector_split_first,
    "Vector::push" => vector_push,
    "Vector::new" => vector_new,
    "Vector::join" => vector_join,
    "Vector::rev" => vector_rev,
    "Vector::count" => vector_count,
    "Vector::take" => vector_take,
    "Vector::skip" => vector_skip,
    "Vector::dedup" => vector_dedup,
    "Vector::chain" => vector_chain,
    "Vector::contains" => vector_contains,
    "Vector::enumerate" => vector_enumerate,
    "Vector::zip" => vector_zip,
    "Vector::windows" => vector_windows,
    "Vector::chunks" => vector_chunks,
    "Vector::sum" => vector_sum,
    "Vector::product" => vector_product,
    "Vector::min" => vector_min,
    "Vector::max" => vector_max,

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
    "Option::ok" => option_ok,
    "Option::flatten" => option_flatten,
    "Option::zip" => option_zip,

    "Result::unwrap" => result_unwrap,
    "Result::unwrap_or" => result_unwrap_or,
    "Result::is_ok" => result_is_ok,
    "Result::is_err" => result_is_err,
    "Result::unwrap_err" => result_unwrap_err,
    "Result::ok" => result_ok,
    "Result::err" => result_err,

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

    "char::is_alphabetic" => char_is_alphabetic,
    "char::is_numeric" => char_is_numeric,
    "char::to_string" => char_to_string,

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
    Ok(UNIT)
}

/// Prints a single value to stdout followed by a newline.
///
/// Internal builtin emitted by `println!()`.
fn __print_str_ln(args: &[Value]) -> Result<Value> {
    expect_arg_count("__print_str_ln", args, 1)?;
    println!("{}", args[0]);
    Ok(UNIT)
}

/// Converts any value to its string representation.
///
/// Internal builtin emitted by `print!` / `println!` format string expansion
/// for `{}` placeholder interpolation.
fn __to_string(args: &[Value]) -> Result<Value> {
    expect_arg_count("__to_string", args, 1)?;
    Ok(Value::Str(format!("{}", args[0])))
}

/// Concatenates two string values into a single string.
///
/// Internal builtin emitted by `panic!` format string expansion and
/// string concatenation via the `+` operator.
fn __str_concat(args: &[Value]) -> Result<Value> {
    expect_arg_count("__str_concat", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        _ => Err(EvalError::Builtin("__str_concat: expected two string arguments".into()).into()),
    }
}

/// Terminates execution with a runtime error.
///
/// Internal builtin emitted by `assert!`, `assert_eq!`, `panic!`, `todo!`,
/// and `unimplemented!` macro expansion.
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

/// Returns the first element of a vector, wrapped in `Some`, or `None` if empty.
fn vector_first(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::first", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(option_wrap(arr.first().cloned())),
        other => method_type_error(other, "first", "Vector"),
    }
}

/// Returns the last element of a vector, wrapped in `Some`, or `None` if empty.
fn vector_last(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::last", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(option_wrap(arr.last().cloned())),
        other => method_type_error(other, "last", "Vector"),
    }
}

/// Returns all elements of a vector after the first, or an empty vector if empty.
fn vector_split_first(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::split_first", args, 1)?;
    match &args[0] {
        Value::Vector(arr) => Ok(arr.split_first().map_or_else(
            || Value::Vector(vec![]),
            |(_, tail)| Value::Vector(tail.to_vec()),
        )),
        other => method_type_error(other, "split_first", "Vector"),
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

/// Returns a reversed copy of the vector.
fn vector_rev(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::rev", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            let mut reversed = v.clone();
            reversed.reverse();
            Ok(Value::Vector(reversed))
        }
        other => method_type_error(other, "rev", "Vector"),
    }
}

/// Returns the number of elements (alias for `len`).
fn vector_count(args: &[Value]) -> Result<Value> {
    vector_len(args)
}

/// Returns the first `n` elements.
fn vector_take(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::take", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(v), Value::Integer(Integer::Usize(n))) => {
            let taken = v.iter().take(*n).cloned().collect();
            Ok(Value::Vector(taken))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::take: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "take", "Vector"),
    }
}

/// Skips the first `n` elements.
fn vector_skip(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::skip", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(v), Value::Integer(Integer::Usize(n))) => {
            let skipped = v.iter().skip(*n).cloned().collect();
            Ok(Value::Vector(skipped))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::skip: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "skip", "Vector"),
    }
}

/// Removes consecutive duplicate elements.
fn vector_dedup(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::dedup", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            let mut deduped = Vec::with_capacity(v.len());
            for item in v {
                if deduped.last() != Some(item) {
                    deduped.push(item.clone());
                }
            }
            Ok(Value::Vector(deduped))
        }
        other => method_type_error(other, "dedup", "Vector"),
    }
}

/// Concatenates two vectors.
fn vector_chain(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::chain", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(a), Value::Vector(b)) => {
            let mut chained = a.clone();
            chained.extend_from_slice(b);
            Ok(Value::Vector(chained))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::chain: expected Vector, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "chain", "Vector"),
    }
}

/// Returns `true` if the vector contains the given element.
fn vector_contains(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::contains", args, 2)?;
    match &args[0] {
        Value::Vector(v) => Ok(Value::Bool(v.contains(&args[1]))),
        other => method_type_error(other, "contains", "Vector"),
    }
}

/// Returns a vector of `(index, element)` tuples.
fn vector_enumerate(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::enumerate", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            let pairs = v
                .iter()
                .enumerate()
                .map(|(i, val)| Value::Tuple(vec![Value::Integer(Integer::Usize(i)), val.clone()]))
                .collect();
            Ok(Value::Vector(pairs))
        }
        other => method_type_error(other, "enumerate", "Vector"),
    }
}

/// Zips two vectors into a vector of tuples.
fn vector_zip(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::zip", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(a), Value::Vector(b)) => {
            let zipped = a
                .iter()
                .zip(b.iter())
                .map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()]))
                .collect();
            Ok(Value::Vector(zipped))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::zip: expected Vector, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "zip", "Vector"),
    }
}

/// Returns sliding windows of size `n`.
fn vector_windows(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::windows", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(v), Value::Integer(Integer::Usize(n))) => {
            if *n == 0 {
                return Err(EvalError::Builtin(
                    "Vector::windows: window size must be > 0".to_string(),
                )
                .into());
            }
            let windows = v.windows(*n).map(|w| Value::Vector(w.to_vec())).collect();
            Ok(Value::Vector(windows))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::windows: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "windows", "Vector"),
    }
}

/// Returns non-overlapping chunks of size `n`.
fn vector_chunks(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::chunks", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Vector(v), Value::Integer(Integer::Usize(n))) => {
            if *n == 0 {
                return Err(EvalError::Builtin(
                    "Vector::chunks: chunk size must be > 0".to_string(),
                )
                .into());
            }
            let chunks = v.chunks(*n).map(|c| Value::Vector(c.to_vec())).collect();
            Ok(Value::Vector(chunks))
        }
        (Value::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::chunks: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "chunks", "Vector"),
    }
}

/// Returns the sum of all integer elements. Empty vector returns 0.
fn vector_sum(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::sum", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            if v.is_empty() {
                return Ok(Value::Integer(Integer::I64(0)));
            }
            let mut acc = match &v[0] {
                Value::Integer(i) => i.zero(),
                other => {
                    return Err(EvalError::Builtin(format!(
                        "Vector::sum: expected integer elements, got {}",
                        other.type_name()
                    ))
                    .into());
                }
            };
            for item in v {
                match item {
                    Value::Integer(i) => {
                        acc = acc.checked_add(*i).ok_or_else(|| {
                            EvalError::Builtin("Vector::sum: overflow".to_string())
                        })?;
                    }
                    other => {
                        return Err(EvalError::Builtin(format!(
                            "Vector::sum: expected integer, got {}",
                            other.type_name()
                        ))
                        .into());
                    }
                }
            }
            Ok(Value::Integer(acc))
        }
        other => method_type_error(other, "sum", "Vector"),
    }
}

/// Returns the product of all integer elements. Empty vector returns 1.
fn vector_product(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::product", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            if v.is_empty() {
                return Ok(Value::Integer(Integer::I64(1)));
            }
            let mut acc = match &v[0] {
                Value::Integer(i) => i.one(),
                other => {
                    return Err(EvalError::Builtin(format!(
                        "Vector::product: expected integer elements, got {}",
                        other.type_name()
                    ))
                    .into());
                }
            };
            for item in v {
                match item {
                    Value::Integer(i) => {
                        acc = acc.checked_mul(*i).ok_or_else(|| {
                            EvalError::Builtin("Vector::product: overflow".to_string())
                        })?;
                    }
                    other => {
                        return Err(EvalError::Builtin(format!(
                            "Vector::product: expected integer, got {}",
                            other.type_name()
                        ))
                        .into());
                    }
                }
            }
            Ok(Value::Integer(acc))
        }
        other => method_type_error(other, "product", "Vector"),
    }
}

/// Returns the minimum element, or `None` if empty.
fn vector_min(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::min", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            if v.is_empty() {
                return Ok(option_wrap(None));
            }
            let mut min = &v[0];
            for item in &v[1..] {
                match (item, min) {
                    (Value::Integer(a), Value::Integer(b))
                        if a.partial_cmp(b) == Some(std::cmp::Ordering::Less) =>
                    {
                        min = item;
                    }
                    _ => {}
                }
            }
            Ok(option_wrap(Some(min.clone())))
        }
        other => method_type_error(other, "min", "Vector"),
    }
}

/// Returns the maximum element, or `None` if empty.
fn vector_max(args: &[Value]) -> Result<Value> {
    expect_arg_count("Vector::max", args, 1)?;
    match &args[0] {
        Value::Vector(v) => {
            if v.is_empty() {
                return Ok(option_wrap(None));
            }
            let mut max = &v[0];
            for item in &v[1..] {
                match (item, max) {
                    (Value::Integer(a), Value::Integer(b))
                        if a.partial_cmp(b) == Some(std::cmp::Ordering::Greater) =>
                    {
                        max = item;
                    }
                    _ => {}
                }
            }
            Ok(option_wrap(Some(max.clone())))
        }
        other => method_type_error(other, "max", "Vector"),
    }
}

/// Creates a new empty map.
fn map_new(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::new", args, 0)?;
    Ok(Value::Map(Map {
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
            Ok(Value::Map(Map { pairs: new_map }))
        }
        other => method_type_error(other, "insert", "Map"),
    }
}

/// Returns the value associated with the given key wrapped in `Some`, or
/// `None` if not present. Receiver at `args[0]`, key at `args[1]`.
fn map_get(args: &[Value]) -> Result<Value> {
    expect_arg_count("Map::get", args, 2)?;
    match &args[0] {
        Value::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            Ok(option_wrap(map.pairs.get(&key).cloned()))
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
            Ok(Value::Map(Map { pairs: new_map }))
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
    Ok(Value::Set(Set(IndexSet::new())))
}

/// Returns a new set with the given value inserted. Receiver at `args[0]`, value at `args[1]`.
fn set_insert(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::insert", args, 2)?;
    match &args[0] {
        Value::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set = set.clone();
            new_set.0.insert(key);
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
            Ok(Value::Bool(set.0.contains(&key)))
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
            new_set.0.swap_remove(&key);
            Ok(Value::Set(new_set))
        }
        other => method_type_error(other, "remove", "Set"),
    }
}

/// Returns the number of elements in the set.
fn set_len(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::len", args, 1)?;
    match &args[0] {
        Value::Set(set) => Ok(Value::Integer(Integer::Usize(set.0.len()))),
        other => method_type_error(other, "len", "Set"),
    }
}

/// Converts a set to a vec of its elements.
fn set_to_vector(args: &[Value]) -> Result<Value> {
    expect_arg_count("Set::to_vector", args, 1)?;
    match &args[0] {
        Value::Set(set) => {
            let arr = set
                .0
                .iter()
                .map(|h| match h {
                    Hashable::Integer(v) => Value::Integer(*v),
                    Hashable::Felt(v) => Value::Felt(Felt::new(*v)),
                    Hashable::Bool(v) => Value::Bool(*v),
                    Hashable::Char(v) => Value::Char(*v),
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

/// Returns `true` if the character is alphabetic (Unicode `Alphabetic` property).
fn char_is_alphabetic(args: &[Value]) -> Result<Value> {
    expect_arg_count("char::is_alphabetic", args, 1)?;
    match &args[0] {
        Value::Char(c) => Ok(Value::Bool(c.is_alphabetic())),
        other => method_type_error(other, "is_alphabetic", "char"),
    }
}

/// Returns `true` if the character is numeric (Unicode `Numeric_Type` property).
fn char_is_numeric(args: &[Value]) -> Result<Value> {
    expect_arg_count("char::is_numeric", args, 1)?;
    match &args[0] {
        Value::Char(c) => Ok(Value::Bool(c.is_numeric())),
        other => method_type_error(other, "is_numeric", "char"),
    }
}

/// Converts a character to a single-character string.
fn char_to_string(args: &[Value]) -> Result<Value> {
    expect_arg_count("char::to_string", args, 1)?;
    match &args[0] {
        Value::Char(c) => Ok(Value::Str(c.to_string())),
        other => method_type_error(other, "to_string", "char"),
    }
}

/// Wraps a Rust `std::option::Option<Value>` into a Maat `Option<T>` enum variant.
fn option_wrap(opt: Option<Value>) -> Value {
    match opt {
        Some(val) => Value::EnumVariant(EnumVariantVal {
            type_index: OPTION_TYPE_INDEX,
            tag: SOME_TAG,
            fields: vec![val],
        }),
        None => Value::EnumVariant(EnumVariantVal {
            type_index: OPTION_TYPE_INDEX,
            tag: NONE_TAG,
            fields: vec![],
        }),
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

/// Converts `Some(x)` to `Ok(x)`, or `None` to `Err(())`.
fn option_ok(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::ok", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: RESULT_TYPE_INDEX,
                tag: OK_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: RESULT_TYPE_INDEX,
                tag: ERR_TAG,
                fields: vec![Value::Unit],
            }))
        }
        other => method_type_error(other, "ok", "Option"),
    }
}

/// Collapses `Some(Some(x))` to `Some(x)`, or `Some(None)` / `None` to `None`.
fn option_flatten(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::flatten", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            match &v.fields[0] {
                inner @ Value::EnumVariant(iv) if iv.type_index == OPTION_TYPE_INDEX => {
                    Ok(inner.clone())
                }
                _ => Err(EvalError::Builtin(
                    "called `Option::flatten()` on a non-nested Option".to_string(),
                )
                .into()),
            }
        }
        Value::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            Ok(args[0].clone())
        }
        other => method_type_error(other, "flatten", "Option"),
    }
}

/// Combines two `Option`s into `Some((a, b))` if both are `Some`, otherwise `None`.
fn option_zip(args: &[Value]) -> Result<Value> {
    expect_arg_count("Option::zip", args, 2)?;
    match (&args[0], &args[1]) {
        (Value::EnumVariant(a), Value::EnumVariant(b))
            if a.type_index == OPTION_TYPE_INDEX
                && a.tag == SOME_TAG
                && b.type_index == OPTION_TYPE_INDEX
                && b.tag == SOME_TAG =>
        {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![Value::Tuple(vec![a.fields[0].clone(), b.fields[0].clone()])],
            }))
        }
        (Value::EnumVariant(a), Value::EnumVariant(b))
            if a.type_index == OPTION_TYPE_INDEX && b.type_index == OPTION_TYPE_INDEX =>
        {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        _ => Err(EvalError::Builtin("Option::zip requires two Option values".to_string()).into()),
    }
}

/// Extracts the `Err` value, or produces a runtime error on `Ok`.
fn result_unwrap_err(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::unwrap_err", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            Ok(v.fields[0].clone())
        }
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            let ok_val = v
                .fields
                .first()
                .map_or("unknown".to_string(), |e| format!("{e}"));
            Err(EvalError::Builtin(format!(
                "called `Result::unwrap_err()` on an `Ok` value: {ok_val}"
            ))
            .into())
        }
        other => method_type_error(other, "unwrap_err", "Result"),
    }
}

/// Converts `Ok(x)` to `Some(x)`, or `Err(_)` to `None`.
fn result_ok(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::ok", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        other => method_type_error(other, "ok", "Result"),
    }
}

/// Converts `Err(e)` to `Some(e)`, or `Ok(_)` to `None`.
fn result_err(args: &[Value]) -> Result<Value> {
    expect_arg_count("Result::err", args, 1)?;
    match &args[0] {
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        Value::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            Ok(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        other => method_type_error(other, "err", "Result"),
    }
}

/// Converts a `Hashable` back to an `Value`.
fn hashable_to_object(h: &Hashable) -> Value {
    match h {
        Hashable::Integer(v) => Value::Integer(*v),
        Hashable::Felt(v) => Value::Felt(Felt::new(*v)),
        Hashable::Bool(v) => Value::Bool(*v),
        Hashable::Char(v) => Value::Char(*v),
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
