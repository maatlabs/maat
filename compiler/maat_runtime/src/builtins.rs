use indexmap::{IndexMap, IndexSet};
use maat_errors::{Error, EvalError, Result};

use crate::{
    BuiltinArg, BuiltinFn, BuiltinReturn, EnumVariantVal, Felt, Hashable, Integer, Map, Set, UNIT,
    Value,
};

const OPTION_TYPE_INDEX: u16 = 0;
const SOME_TAG: u16 = 0;
const NONE_TAG: u16 = 1;

const RESULT_TYPE_INDEX: u16 = 1;
const OK_TAG: u16 = 0;
const ERR_TAG: u16 = 1;

const PARSE_INT_ERROR_TYPE_INDEX: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseIntError {
    Empty,
    InvalidDigit,
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

    pub const fn tag(self) -> u16 {
        match self {
            Self::Empty => 0,
            Self::InvalidDigit => 1,
            Self::Overflow => 2,
        }
    }
}

macro_rules! define_builtins {
    ( $( $( $name:literal )|+ => $func:ident ),* $(,)? ) => {
        pub const BUILTINS: &[(&str, BuiltinFn)] = &[
            $( $( ($name, $func), )+ )*
        ];

        pub const BUILTIN_COUNT: usize = BUILTINS.len();

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

fn arg_to_value(arg: &BuiltinArg<'_>) -> Value {
    match arg {
        BuiltinArg::Unit => Value::Unit,
        BuiltinArg::Integer(i) => Value::Integer(*i),
        BuiltinArg::Felt(f) => Value::Felt(*f),
        BuiltinArg::Bool(b) => Value::Bool(*b),
        BuiltinArg::Char(c) => Value::Char(*c),
        BuiltinArg::Str(s) => Value::Str((*s).to_owned()),
        BuiltinArg::Tuple(t) => Value::Tuple((*t).to_vec()),
        BuiltinArg::Vector(_) => {
            unreachable!(
                "BuiltinArg::Vector cannot be embedded into a Value directly; \
                 return it via BuiltinReturn::Vector so the VM segment-allocates."
            )
        }
        BuiltinArg::Array(a) => Value::Array((*a).to_vec()),
        BuiltinArg::Map(m) => Value::Map((*m).clone()),
        BuiltinArg::Set(s) => Value::Set((*s).clone()),
        BuiltinArg::Builtin(f) => Value::Builtin(*f),
        BuiltinArg::CompiledFn(f) => Value::CompiledFn((*f).clone()),
        BuiltinArg::Closure(c) => Value::Closure((*c).clone()),
        BuiltinArg::Struct(s) => Value::Struct((*s).clone()),
        BuiltinArg::EnumVariant(ev) => Value::EnumVariant((*ev).clone()),
        BuiltinArg::Range(s, e) => Value::Range(*s, *e),
        BuiltinArg::RangeInclusive(s, e) => Value::RangeInclusive(*s, *e),
        BuiltinArg::Relocatable(r) => Value::Relocatable(*r),
    }
}

/// Convenience for builtins that return a primitive [`Value`].
#[inline]
fn val(v: Value) -> Result<BuiltinReturn> {
    Ok(BuiltinReturn::Value(v))
}

fn __print_str(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("__print_str", args, 1)?;
    print!("{}", args[0]);
    val(UNIT)
}

fn __print_str_ln(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("__print_str_ln", args, 1)?;
    println!("{}", args[0]);
    val(UNIT)
}

fn __to_string(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("__to_string", args, 1)?;
    Ok(BuiltinReturn::Str(format!("{}", args[0])))
}

fn __str_concat(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("__str_concat", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Str(a), BuiltinArg::Str(b)) => Ok(BuiltinReturn::Str(format!("{a}{b}"))),
        _ => Err(EvalError::Builtin("__str_concat: expected two string arguments".into()).into()),
    }
}

fn __panic(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("__panic", args, 1)?;
    Err(EvalError::Builtin(format!("{}", args[0])).into())
}

fn vector_len(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::len", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(arr) | BuiltinArg::Array(arr) => {
            val(Value::Integer(Integer::Usize(arr.len())))
        }
        other => method_type_error(other, "len", "Vector"),
    }
}

fn vector_first(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::first", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(arr) => val(option_wrap(arr.first().cloned())),
        other => method_type_error(other, "first", "Vector"),
    }
}

fn vector_last(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::last", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(arr) => val(option_wrap(arr.last().cloned())),
        other => method_type_error(other, "last", "Vector"),
    }
}

fn vector_split_first(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::split_first", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(arr) => {
            let tail = arr.split_first().map(|(_, t)| t).unwrap_or(&[]);
            Ok(BuiltinReturn::vector_from_values(tail.to_vec()))
        }
        other => method_type_error(other, "split_first", "Vector"),
    }
}

fn vector_push(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::push", args, 2)?;
    match &args[0] {
        BuiltinArg::Vector(arr) => {
            let mut new_arr = arr.to_vec();
            new_arr.push(arg_to_value(&args[1]));
            Ok(BuiltinReturn::vector_from_values(new_arr))
        }
        other => method_type_error(other, "push", "Vector"),
    }
}

fn vector_join(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::join", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(arr), BuiltinArg::Str(sep)) => {
            let joined = arr
                .iter()
                .map(|v| format!("{v}"))
                .collect::<Vec<_>>()
                .join(sep);
            Ok(BuiltinReturn::Str(joined))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::join: separator must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "join", "Vector"),
    }
}

fn vector_new(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::new", args, 0)?;
    Ok(BuiltinReturn::vector_from_values(Vec::new()))
}

fn vector_rev(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::rev", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            let mut reversed = v.to_vec();
            reversed.reverse();
            Ok(BuiltinReturn::vector_from_values(reversed))
        }
        other => method_type_error(other, "rev", "Vector"),
    }
}

fn vector_count(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    vector_len(args)
}

fn vector_take(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::take", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(v), BuiltinArg::Integer(Integer::Usize(n))) => {
            let taken = v.iter().take(*n).cloned().collect();
            Ok(BuiltinReturn::vector_from_values(taken))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::take: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "take", "Vector"),
    }
}

fn vector_skip(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::skip", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(v), BuiltinArg::Integer(Integer::Usize(n))) => {
            let skipped = v.iter().skip(*n).cloned().collect();
            Ok(BuiltinReturn::vector_from_values(skipped))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::skip: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "skip", "Vector"),
    }
}

fn vector_dedup(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::dedup", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            let mut deduped = Vec::with_capacity(v.len());
            for item in *v {
                if deduped.last() != Some(item) {
                    deduped.push(item.clone());
                }
            }
            Ok(BuiltinReturn::vector_from_values(deduped))
        }
        other => method_type_error(other, "dedup", "Vector"),
    }
}

fn vector_chain(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::chain", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(a), BuiltinArg::Vector(b)) => {
            let mut chained = a.to_vec();
            chained.extend_from_slice(b);
            Ok(BuiltinReturn::vector_from_values(chained))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::chain: expected Vector, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "chain", "Vector"),
    }
}

fn vector_contains(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::contains", args, 2)?;
    match &args[0] {
        BuiltinArg::Vector(v) | BuiltinArg::Array(v) => {
            let target = arg_to_value(&args[1]);
            val(Value::Bool(v.contains(&target)))
        }
        other => method_type_error(other, "contains", "Vector"),
    }
}

fn vector_enumerate(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::enumerate", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            let pairs = v
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    Value::Tuple(vec![Value::Integer(Integer::Usize(i)), item.clone()])
                })
                .collect();
            Ok(BuiltinReturn::vector_from_values(pairs))
        }
        other => method_type_error(other, "enumerate", "Vector"),
    }
}

fn vector_zip(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::zip", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(a), BuiltinArg::Vector(b)) => {
            let zipped = a
                .iter()
                .zip(b.iter())
                .map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()]))
                .collect();
            Ok(BuiltinReturn::vector_from_values(zipped))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::zip: expected Vector, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "zip", "Vector"),
    }
}

fn vector_windows(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::windows", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(v), BuiltinArg::Integer(Integer::Usize(n))) => {
            if *n == 0 {
                return Err(EvalError::Builtin(
                    "Vector::windows: window size must be > 0".to_string(),
                )
                .into());
            }
            let windows = v
                .windows(*n)
                .map(|w| BuiltinReturn::vector_from_values(w.to_vec()))
                .collect();
            Ok(BuiltinReturn::Vector(windows))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::windows: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "windows", "Vector"),
    }
}

fn vector_chunks(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::chunks", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Vector(v), BuiltinArg::Integer(Integer::Usize(n))) => {
            if *n == 0 {
                return Err(EvalError::Builtin(
                    "Vector::chunks: chunk size must be > 0".to_string(),
                )
                .into());
            }
            let chunks = v
                .chunks(*n)
                .map(|c| BuiltinReturn::vector_from_values(c.to_vec()))
                .collect();
            Ok(BuiltinReturn::Vector(chunks))
        }
        (BuiltinArg::Vector(_), other) => Err(EvalError::Builtin(format!(
            "Vector::chunks: expected usize, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "chunks", "Vector"),
    }
}

fn vector_sum(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::sum", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            if v.is_empty() {
                return val(Value::Integer(Integer::I64(0)));
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
            for item in *v {
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
            val(Value::Integer(acc))
        }
        other => method_type_error(other, "sum", "Vector"),
    }
}

fn vector_product(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::product", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            if v.is_empty() {
                return val(Value::Integer(Integer::I64(1)));
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
            for item in *v {
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
            val(Value::Integer(acc))
        }
        other => method_type_error(other, "product", "Vector"),
    }
}

fn vector_min(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::min", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            if v.is_empty() {
                return val(option_wrap(None));
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
            val(option_wrap(Some(min.clone())))
        }
        other => method_type_error(other, "min", "Vector"),
    }
}

fn vector_max(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Vector::max", args, 1)?;
    match &args[0] {
        BuiltinArg::Vector(v) => {
            if v.is_empty() {
                return val(option_wrap(None));
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
            val(option_wrap(Some(max.clone())))
        }
        other => method_type_error(other, "max", "Vector"),
    }
}

fn map_new(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::new", args, 0)?;
    val(Value::Map(Map {
        pairs: IndexMap::new(),
    }))
}

fn map_insert(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::insert", args, 3)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.insert(key, arg_to_value(&args[2]));
            val(Value::Map(Map { pairs: new_map }))
        }
        other => method_type_error(other, "insert", "Map"),
    }
}

fn map_get(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::get", args, 2)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            val(option_wrap(map.pairs.get(&key).cloned()))
        }
        other => method_type_error(other, "get", "Map"),
    }
}

fn map_contains_key(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::contains_key", args, 2)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            val(Value::Bool(map.pairs.contains_key(&key)))
        }
        other => method_type_error(other, "contains_key", "Map"),
    }
}

fn map_remove(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::remove", args, 2)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_map = map.pairs.clone();
            new_map.swap_remove(&key);
            val(Value::Map(Map { pairs: new_map }))
        }
        other => method_type_error(other, "remove", "Map"),
    }
}

fn map_len(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::len", args, 1)?;
    match &args[0] {
        BuiltinArg::Map(map) => val(Value::Integer(Integer::Usize(map.pairs.len()))),
        other => method_type_error(other, "len", "Map"),
    }
}

fn map_keys(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::keys", args, 1)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let keys = map.pairs.keys().map(hashable_to_object).collect();
            Ok(BuiltinReturn::vector_from_values(keys))
        }
        other => method_type_error(other, "keys", "Map"),
    }
}

fn map_values(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Map::values", args, 1)?;
    match &args[0] {
        BuiltinArg::Map(map) => {
            let values = map.pairs.values().cloned().collect();
            Ok(BuiltinReturn::vector_from_values(values))
        }
        other => method_type_error(other, "values", "Map"),
    }
}

fn set_new(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::new", args, 0)?;
    val(Value::Set(Set(IndexSet::new())))
}

fn set_insert(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::insert", args, 2)?;
    match &args[0] {
        BuiltinArg::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set: Set = (*set).clone();
            new_set.0.insert(key);
            val(Value::Set(new_set))
        }
        other => method_type_error(other, "insert", "Set"),
    }
}

fn set_contains(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::contains", args, 2)?;
    match &args[0] {
        BuiltinArg::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            val(Value::Bool(set.0.contains(&key)))
        }
        other => method_type_error(other, "contains", "Set"),
    }
}

fn set_remove(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::remove", args, 2)?;
    match &args[0] {
        BuiltinArg::Set(set) => {
            let key = Hashable::try_from(args[1].clone())?;
            let mut new_set: Set = (*set).clone();
            new_set.0.swap_remove(&key);
            val(Value::Set(new_set))
        }
        other => method_type_error(other, "remove", "Set"),
    }
}

fn set_len(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::len", args, 1)?;
    match &args[0] {
        BuiltinArg::Set(set) => val(Value::Integer(Integer::Usize(set.0.len()))),
        other => method_type_error(other, "len", "Set"),
    }
}

fn set_to_vector(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Set::to_vector", args, 1)?;
    match &args[0] {
        BuiltinArg::Set(set) => {
            let arr = set.0.iter().map(hashable_to_object).collect();
            Ok(BuiltinReturn::vector_from_values(arr))
        }
        other => method_type_error(other, "to_vector", "Set"),
    }
}

fn str_len(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::len", args, 1)?;
    match &args[0] {
        BuiltinArg::Str(s) => val(Value::Integer(Integer::Usize(s.len()))),
        other => method_type_error(other, "len", "str"),
    }
}

fn str_trim(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::trim", args, 1)?;
    match &args[0] {
        BuiltinArg::Str(s) => Ok(BuiltinReturn::Str(s.trim().to_string())),
        other => method_type_error(other, "trim", "str"),
    }
}

fn str_contains(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::contains", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Str(haystack), BuiltinArg::Str(needle)) => {
            val(Value::Bool(haystack.contains(*needle)))
        }
        (BuiltinArg::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::contains: pattern must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "contains", "str"),
    }
}

fn str_starts_with(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::starts_with", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Str(s), BuiltinArg::Str(prefix)) => val(Value::Bool(s.starts_with(*prefix))),
        (BuiltinArg::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::starts_with: prefix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "starts_with", "str"),
    }
}

fn str_ends_with(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::ends_with", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Str(s), BuiltinArg::Str(suffix)) => val(Value::Bool(s.ends_with(*suffix))),
        (BuiltinArg::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::ends_with: suffix must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "ends_with", "str"),
    }
}

fn str_split(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::split", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Str(s), BuiltinArg::Str(delim)) => {
            let parts = s
                .split(*delim)
                .map(|part| Value::Str(part.to_string()))
                .collect();
            Ok(BuiltinReturn::vector_from_values(parts))
        }
        (BuiltinArg::Str(_), other) => Err(EvalError::Builtin(format!(
            "str::split: delimiter must be a string, got {}",
            other.type_name()
        ))
        .into()),
        (other, _) => method_type_error(other, "split", "str"),
    }
}

fn char_is_alphabetic(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("char::is_alphabetic", args, 1)?;
    match &args[0] {
        BuiltinArg::Char(c) => val(Value::Bool(c.is_alphabetic())),
        other => method_type_error(other, "is_alphabetic", "char"),
    }
}

fn char_is_numeric(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("char::is_numeric", args, 1)?;
    match &args[0] {
        BuiltinArg::Char(c) => val(Value::Bool(c.is_numeric())),
        other => method_type_error(other, "is_numeric", "char"),
    }
}

fn char_to_string(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("char::to_string", args, 1)?;
    match &args[0] {
        BuiltinArg::Char(c) => Ok(BuiltinReturn::Str(c.to_string())),
        other => method_type_error(other, "to_string", "char"),
    }
}

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

fn parse_ok(value: Value) -> Value {
    Value::EnumVariant(EnumVariantVal {
        type_index: RESULT_TYPE_INDEX,
        tag: 0,
        fields: vec![value],
    })
}

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

macro_rules! define_str_parse {
    ($fn_name:ident, $method:literal, $rust_ty:ty, $variant:ident) => {
        fn $fn_name(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
            expect_arg_count(concat!("str::", $method), args, 1)?;
            match &args[0] {
                BuiltinArg::Str(s) => val(s.trim().parse::<$rust_ty>().map_or_else(
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

fn option_unwrap(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::unwrap", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            val(v.fields[0].clone())
        }
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            Err(
                EvalError::Builtin("called `Option::unwrap()` on a `None` value".to_string())
                    .into(),
            )
        }
        other => method_type_error(other, "unwrap", "Option"),
    }
}

fn option_unwrap_or(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::unwrap_or", args, 2)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            val(v.fields[0].clone())
        }
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            val(arg_to_value(&args[1]))
        }
        other => method_type_error(other, "unwrap_or", "Option"),
    }
}

fn option_is_some(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::is_some", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX => {
            val(Value::Bool(v.tag == SOME_TAG))
        }
        other => method_type_error(other, "is_some", "Option"),
    }
}

fn option_is_none(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::is_none", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX => {
            val(Value::Bool(v.tag == NONE_TAG))
        }
        other => method_type_error(other, "is_none", "Option"),
    }
}

fn result_unwrap(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::unwrap", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            val(v.fields[0].clone())
        }
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
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

fn result_unwrap_or(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::unwrap_or", args, 2)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            val(v.fields[0].clone())
        }
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            val(arg_to_value(&args[1]))
        }
        other => method_type_error(other, "unwrap_or", "Result"),
    }
}

fn result_is_ok(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::is_ok", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX => {
            val(Value::Bool(v.tag == OK_TAG))
        }
        other => method_type_error(other, "is_ok", "Result"),
    }
}

fn result_is_err(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::is_err", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX => {
            val(Value::Bool(v.tag == ERR_TAG))
        }
        other => method_type_error(other, "is_err", "Result"),
    }
}

fn option_ok(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::ok", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: RESULT_TYPE_INDEX,
                tag: OK_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: RESULT_TYPE_INDEX,
                tag: ERR_TAG,
                fields: vec![Value::Unit],
            }))
        }
        other => method_type_error(other, "ok", "Option"),
    }
}

fn option_flatten(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::flatten", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == SOME_TAG => {
            match &v.fields[0] {
                inner @ Value::EnumVariant(iv) if iv.type_index == OPTION_TYPE_INDEX => {
                    val(inner.clone())
                }
                _ => Err(EvalError::Builtin(
                    "called `Option::flatten()` on a non-nested Option".to_string(),
                )
                .into()),
            }
        }
        BuiltinArg::EnumVariant(v) if v.type_index == OPTION_TYPE_INDEX && v.tag == NONE_TAG => {
            val(arg_to_value(&args[0]))
        }
        other => method_type_error(other, "flatten", "Option"),
    }
}

fn option_zip(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Option::zip", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::EnumVariant(a), BuiltinArg::EnumVariant(b))
            if a.type_index == OPTION_TYPE_INDEX
                && a.tag == SOME_TAG
                && b.type_index == OPTION_TYPE_INDEX
                && b.tag == SOME_TAG =>
        {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![Value::Tuple(vec![a.fields[0].clone(), b.fields[0].clone()])],
            }))
        }
        (BuiltinArg::EnumVariant(a), BuiltinArg::EnumVariant(b))
            if a.type_index == OPTION_TYPE_INDEX && b.type_index == OPTION_TYPE_INDEX =>
        {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        _ => Err(EvalError::Builtin("Option::zip requires two Option values".to_string()).into()),
    }
}

fn result_unwrap_err(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::unwrap_err", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            val(v.fields[0].clone())
        }
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
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

fn result_ok(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::ok", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        other => method_type_error(other, "ok", "Result"),
    }
}

fn result_err(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("Result::err", args, 1)?;
    match &args[0] {
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == ERR_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: SOME_TAG,
                fields: vec![v.fields[0].clone()],
            }))
        }
        BuiltinArg::EnumVariant(v) if v.type_index == RESULT_TYPE_INDEX && v.tag == OK_TAG => {
            val(Value::EnumVariant(EnumVariantVal {
                type_index: OPTION_TYPE_INDEX,
                tag: NONE_TAG,
                fields: vec![],
            }))
        }
        other => method_type_error(other, "err", "Result"),
    }
}

fn hashable_to_object(h: &Hashable) -> Value {
    match h {
        Hashable::Integer(v) => Value::Integer(*v),
        Hashable::Felt(v) => Value::Felt(Felt::new(*v)),
        Hashable::Bool(v) => Value::Bool(*v),
        Hashable::Char(v) => Value::Char(*v),
        Hashable::Str(v) => Value::Str(v.clone()),
    }
}

fn expect_arg_count(method: &str, args: &[BuiltinArg<'_>], count: usize) -> Result<()> {
    (args.len() == count).then_some(()).ok_or(
        EvalError::Builtin(format!(
            "{method}: wrong number of arguments. got={}, want={count}",
            args.len()
        ))
        .into(),
    )
}

fn method_type_error(
    arg: &BuiltinArg<'_>,
    method: &str,
    expected_type: &str,
) -> Result<BuiltinReturn> {
    Err(EvalError::Builtin(format!(
        "cannot call `{method}` on {}, expected {expected_type}",
        arg.type_name()
    ))
    .into())
}

macro_rules! define_from_signed {
    ($fn_name:ident, $target_name:expr, $target_ty:ty, $variant:ident) => {
        fn $fn_name(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
            expect_arg_count($target_name, args, 1)?;
            match &args[0] {
                BuiltinArg::Integer(n) => {
                    let wide = n
                        .to_i128()
                        .ok_or_else(|| conversion_error(n, $target_name))?;
                    let v = <$target_ty>::try_from(wide)
                        .map_err(|_| conversion_error(n, $target_name))?;
                    val(Value::Integer(Integer::$variant(v)))
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
        fn $fn_name(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
            expect_arg_count($target_name, args, 1)?;
            match &args[0] {
                BuiltinArg::Integer(n) => {
                    let wide = n
                        .to_i128()
                        .ok_or_else(|| conversion_error(n, $target_name))?;
                    let v = <$target_ty>::try_from(wide)
                        .map_err(|_| conversion_error(n, $target_name))?;
                    val(Value::Integer(Integer::$variant(v)))
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

macro_rules! define_default {
    ($fn_name:ident, $name:expr, $value:expr) => {
        fn $fn_name(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
            expect_arg_count($name, args, 0)?;
            val($value)
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

fn str_default(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("str::default", args, 0)?;
    Ok(BuiltinReturn::Str(String::new()))
}

fn cmp_min(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("cmp::min", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Integer(a), BuiltinArg::Integer(b)) => {
            let ord = a
                .to_i128()
                .zip(b.to_i128())
                .ok_or_else(|| cmp_error("min"))?;
            val(if ord.0 <= ord.1 {
                Value::Integer(*a)
            } else {
                Value::Integer(*b)
            })
        }
        _ => Err(cmp_error("min")),
    }
}

fn cmp_max(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("cmp::max", args, 2)?;
    match (&args[0], &args[1]) {
        (BuiltinArg::Integer(a), BuiltinArg::Integer(b)) => {
            let ord = a
                .to_i128()
                .zip(b.to_i128())
                .ok_or_else(|| cmp_error("max"))?;
            val(if ord.0 >= ord.1 {
                Value::Integer(*a)
            } else {
                Value::Integer(*b)
            })
        }
        _ => Err(cmp_error("max")),
    }
}

fn cmp_clamp(args: &[BuiltinArg<'_>]) -> Result<BuiltinReturn> {
    expect_arg_count("cmp::clamp", args, 3)?;
    match (&args[0], &args[1], &args[2]) {
        (BuiltinArg::Integer(v), BuiltinArg::Integer(lo), BuiltinArg::Integer(hi)) => {
            let (vv, l, h) = v
                .to_i128()
                .zip(lo.to_i128())
                .zip(hi.to_i128())
                .map(|((vv, l), h)| (vv, l, h))
                .ok_or_else(|| cmp_error("clamp"))?;
            val(if vv < l {
                Value::Integer(*lo)
            } else if vv > h {
                Value::Integer(*hi)
            } else {
                Value::Integer(*v)
            })
        }
        _ => Err(cmp_error("clamp")),
    }
}

fn cmp_error(name: &str) -> Error {
    EvalError::Builtin(format!("cmp::{name}: expected integer arguments")).into()
}
