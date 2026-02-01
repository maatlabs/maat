use super::{BuiltinFn, NULL, Object};
use crate::{EvalError, Result};

/// Attempts to retrieve a builtin by name. Returns `Some(fn)` or `None`.
#[inline]
pub fn get_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "len" => Some(len),
        "puts" | "print" => Some(puts),
        "first" => Some(first),
        "last" => Some(last),
        "rest" => Some(rest),
        "push" => Some(push),
        _ => None,
    }
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

pub fn puts(args: &[Object]) -> Result<Object> {
    args.iter().for_each(|arg| println!("{arg}"));
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
