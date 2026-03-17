/// Matches a pair of `Object` values when both are the same integer variant.
///
/// The `$callback!` macro is invoked per variant with `(Variant, l, r, ...args)`,
/// where `Variant` is the `Object` variant name (e.g., `I8`), `l` and `r` are
/// references to the inner values, and `...args` are forwarded from the invocation.
/// Returns `None` if the operands are not the same integer type.
macro_rules! dispatch_int_pair {
    ($left:expr, $right:expr, $callback:ident ! ($($args:tt)*)) => {
        match ($left, $right) {
            (Object::I8(l), Object::I8(r)) => Some($callback!(I8, l, r, $($args)*)),
            (Object::I16(l), Object::I16(r)) => Some($callback!(I16, l, r, $($args)*)),
            (Object::I32(l), Object::I32(r)) => Some($callback!(I32, l, r, $($args)*)),
            (Object::I64(l), Object::I64(r)) => Some($callback!(I64, l, r, $($args)*)),
            (Object::I128(l), Object::I128(r)) => Some($callback!(I128, l, r, $($args)*)),
            (Object::Isize(l), Object::Isize(r)) => Some($callback!(Isize, l, r, $($args)*)),
            (Object::U8(l), Object::U8(r)) => Some($callback!(U8, l, r, $($args)*)),
            (Object::U16(l), Object::U16(r)) => Some($callback!(U16, l, r, $($args)*)),
            (Object::U32(l), Object::U32(r)) => Some($callback!(U32, l, r, $($args)*)),
            (Object::U64(l), Object::U64(r)) => Some($callback!(U64, l, r, $($args)*)),
            (Object::U128(l), Object::U128(r)) => Some($callback!(U128, l, r, $($args)*)),
            (Object::Usize(l), Object::Usize(r)) => Some($callback!(Usize, l, r, $($args)*)),
            _ => None,
        }
    };
}

/// Matches a single `Object` value when it is a signed integer variant.
macro_rules! dispatch_signed_unary {
    ($val:expr, $callback:ident ! ($($args:tt)*)) => {
        match $val {
            Object::I8(v) => Some($callback!(I8, v, $($args)*)),
            Object::I16(v) => Some($callback!(I16, v, $($args)*)),
            Object::I32(v) => Some($callback!(I32, v, $($args)*)),
            Object::I64(v) => Some($callback!(I64, v, $($args)*)),
            Object::I128(v) => Some($callback!(I128, v, $($args)*)),
            Object::Isize(v) => Some($callback!(Isize, v, $($args)*)),
            _ => None,
        }
    };
}

/// Checked arithmetic: `l.method(r)` returning `Option<Object>`.
macro_rules! checked_binop_arm {
    ($V:ident, $l:ident, $r:ident, $method:ident) => {
        $l.$method(*$r).map(Object::$V)
    };
}

/// Ordered comparison: `*l op *r` returning `bool`.
macro_rules! cmp_arm {
    ($V:ident, $l:ident, $r:ident, $op:tt) => {
        *$l $op *$r
    };
}

/// Bitwise operation: `Object::V(*l op *r)`.
macro_rules! bitwise_arm {
    ($V:ident, $l:ident, $r:ident, $op:tt) => {
        Object::$V(*$l $op *$r)
    };
}

/// Checked shift: converts RHS to `u32`, then `l.method(shift)`.
macro_rules! checked_shift_arm {
    ($V:ident, $l:ident, $r:ident, $method:ident) => {
        u32::try_from(*$r)
            .ok()
            .and_then(|s| $l.$method(s).map(Object::$V))
    };
}

/// Checked negation for a signed value.
macro_rules! checked_neg_arm {
    ($V:ident, $v:ident,) => {
        $v.checked_neg().map(Object::$V)
    };
}

/// Dispatches checked integer arithmetic across all 12 integer variants.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operands are not the same integer type
/// - inner `None`: arithmetic overflow
macro_rules! int_binop {
    ($left:expr, $right:expr, $method:ident) => {
        dispatch_int_pair!($left, $right, checked_binop_arm!($method))
    };
}

/// Dispatches ordered comparison across all 12 integer variants.
///
/// Returns `None` if the operands are not the same integer type.
macro_rules! int_cmp {
    ($left:expr, $right:expr, $op:tt) => {
        dispatch_int_pair!($left, $right, cmp_arm!($op))
    };
}

/// Dispatches a bitwise binary operation across all 12 integer variants.
///
/// Returns `Option<Object>`:
/// - `None`: operands are not the same integer type
/// - `Some(result)`: the result of the operation
macro_rules! int_bitwise {
    ($left:expr, $right:expr, $op:tt) => {
        dispatch_int_pair!($left, $right, bitwise_arm!($op))
    };
}

/// Dispatches a checked shift operation across all 12 integer variants.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operands are not the same integer type
/// - inner `None`: shift amount too large
macro_rules! int_shift {
    ($left:expr, $right:expr, $method:ident) => {
        dispatch_int_pair!($left, $right, checked_shift_arm!($method))
    };
}

/// Dispatches checked negation for signed integer types.
///
/// Returns `Option<Option<Object>>`:
/// - outer `None`: operand is not a signed integer
/// - inner `None`: negation overflow (e.g., `i8::MIN`)
macro_rules! checked_neg {
    ($val:expr) => {
        dispatch_signed_unary!($val, checked_neg_arm!())
    };
}

/// Intermediate representation for type conversion.
///
/// All numeric source values are widened into one of these variants
/// before narrowing to the target type, simplifying the conversion matrix.
pub enum WideNum {
    Int(i128),
    Uint(u128),
}
