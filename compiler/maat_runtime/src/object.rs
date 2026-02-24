use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;
use maat_ast::{BlockStatement, Node};
use maat_errors::{Error, EvalError, Result};
use maat_span::SourceMap;
use serde::{Deserialize, Serialize};

use crate::Env;

pub type BuiltinFn = fn(&[Object]) -> Result<Object>;

pub const TRUE: Object = Object::Boolean(true);
pub const FALSE: Object = Object::Boolean(false);
pub const NULL: Object = Object::Null;

/// Runtime value representation in the interpreter.
///
/// Objects are the evaluated results of expressions and can be integers,
/// booleans, functions, or special values like null and return wrappers.
#[derive(Debug, Clone)]
pub enum Object {
    /// The null object, representing absence of a value.
    Null,

    /// 8-bit signed integer.
    I8(i8),
    /// 16-bit signed integer.
    I16(i16),
    /// 32-bit signed integer.
    I32(i32),
    /// 64-bit signed integer.
    I64(i64),
    /// 128-bit signed integer.
    I128(i128),
    /// Pointer-sized signed integer.
    Isize(isize),

    /// 8-bit unsigned integer.
    U8(u8),
    /// 16-bit unsigned integer.
    U16(u16),
    /// 32-bit unsigned integer.
    U32(u32),
    /// 64-bit unsigned integer.
    U64(u64),
    /// 128-bit unsigned integer.
    U128(u128),
    /// Pointer-sized unsigned integer.
    Usize(usize),

    /// 32-bit floating-point number.
    F32(f32),
    /// 64-bit floating-point number.
    F64(f64),

    /// A boolean value (true or false).
    Boolean(bool),
    /// A string literal.
    String(String),
    /// An array literal.
    Array(Vec<Object>),
    /// A hashable object.
    Hash(HashObject),
    /// A function object with parameters, body, and closure environment.
    Function(Function),
    /// A macro object with parameters, body, and closure environment.
    Macro(Macro),
    /// A quoted AST node for metaprogramming.
    Quote(Quote),
    /// Wraps a return value for early function/block termination.
    ReturnValue(Box<Object>),
    /// A builtin function.
    Builtin(BuiltinFn),
    /// A compiled function containing bytecode instructions.
    CompiledFunction(CompiledFunction),
    /// A closure wrapping a compiled function with captured free variables.
    Closure(Closure),
}

impl Object {
    /// Converts a runtime object back to an AST node.
    ///
    /// Used to splice evaluated values back into quoted code.
    pub fn to_ast_node(obj: &Self) -> Option<Node> {
        use maat_ast::{self as ast, *};
        use maat_span::Span;

        macro_rules! convert_int {
        ($($obj:ident => $ast_name:ident($ast_type:ident)),* $(,)?) => {
            match obj {
                $(
                    Self::$obj(v) => Some(Node::Expression(Expression::$ast_name(ast::$ast_type {
                        radix: Radix::Dec,
                        value: *v,
                        span: Span::ZERO,
                    }))),
                )*
                Self::Boolean(b) => Some(Node::Expression(Expression::Boolean(BooleanLiteral {
                    value: *b,
                    span: Span::ZERO,
                }))),
                Self::Quote(q) => Some(q.node.clone()),
                _ => None,
            }
        };
    }

        convert_int!(
            I8 => I8(I8),
            I16 => I16(I16),
            I32 => I32(I32),
            I64 => I64(I64),
            I128 => I128(I128),
            Isize => Isize(Isize),
            U8 => U8(U8),
            U16 => U16(U16),
            U32 => U32(U32),
            U64 => U64(U64),
            U128 => U128(U128),
            Usize => Usize(Usize),
        )
    }

    /// Determines whether this object is truthy.
    ///
    /// Booleans return their value directly; null is falsy;
    /// all other values (including integers) are truthy.
    #[inline]
    pub fn is_truthy(&self) -> bool {
        match self {
            Object::Boolean(b) => *b,
            Object::Null => false,
            _ => true,
        }
    }

    /// Attempts to convert this object to a `usize` array index.
    ///
    /// Returns `Some(index)` for any integer type whose value fits in `usize`.
    /// Returns `None` for negative values, out-of-range values, or non-integer types.
    pub fn to_array_index(&self) -> Option<usize> {
        match self {
            Self::I8(v) => usize::try_from(*v).ok(),
            Self::I16(v) => usize::try_from(*v).ok(),
            Self::I32(v) => usize::try_from(*v).ok(),
            Self::I64(v) => usize::try_from(*v).ok(),
            Self::I128(v) => usize::try_from(*v).ok(),
            Self::Isize(v) => usize::try_from(*v).ok(),
            Self::U8(v) => Some(*v as usize),
            Self::U16(v) => Some(*v as usize),
            Self::U32(v) => Some(*v as usize),
            Self::U64(v) => usize::try_from(*v).ok(),
            Self::U128(v) => usize::try_from(*v).ok(),
            Self::Usize(v) => Some(*v),
            _ => None,
        }
    }

    /// Converts any integer variant to `i128` for cross-type comparison.
    ///
    /// Returns `None` for non-integer types or `U128` values exceeding `i128::MAX`.
    pub fn to_i128(&self) -> Option<i128> {
        match self {
            Self::I8(v) => Some(*v as i128),
            Self::I16(v) => Some(*v as i128),
            Self::I32(v) => Some(*v as i128),
            Self::I64(v) => Some(*v as i128),
            Self::I128(v) => Some(*v),
            Self::Isize(v) => Some(*v as i128),
            Self::U8(v) => Some(*v as i128),
            Self::U16(v) => Some(*v as i128),
            Self::U32(v) => Some(*v as i128),
            Self::U64(v) => Some(*v as i128),
            Self::U128(v) => i128::try_from(*v).ok(),
            Self::Usize(v) => Some(*v as i128),
            _ => None,
        }
    }

    /// Returns `true` if this object is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::I8(_)
                | Self::I16(_)
                | Self::I32(_)
                | Self::I64(_)
                | Self::I128(_)
                | Self::Isize(_)
                | Self::U8(_)
                | Self::U16(_)
                | Self::U32(_)
                | Self::U64(_)
                | Self::U128(_)
                | Self::Usize(_)
        )
    }

    /// Returns a string representation of the object's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::I8(_) => "I8",
            Self::I16(_) => "I16",
            Self::I32(_) => "I32",
            Self::I64(_) => "I64",
            Self::I128(_) => "I128",
            Self::Isize(_) => "Isize",
            Self::U8(_) => "U8",
            Self::U16(_) => "U16",
            Self::U32(_) => "U32",
            Self::U64(_) => "U64",
            Self::U128(_) => "U128",
            Self::Usize(_) => "Usize",
            Self::F32(_) => "F32",
            Self::F64(_) => "F64",
            Self::Boolean(_) => "Boolean",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Hash(_) => "Hashable",
            Self::Function(_) => "Function",
            Self::Macro(_) => "Macro",
            Self::Quote(_) => "Quote",
            Self::ReturnValue(_) => "ReturnValue",
            Self::Builtin(_) => "BuiltinFn",
            Self::CompiledFunction(_) => "CompiledFunction",
            Self::Closure(_) => "Closure",
        }
    }
}

/// Serialization proxy containing only the [`Object`] variants that can be
/// represented in the bytecode binary format.
///
/// Non-serializable variants (`Function`, `Macro`, `Quote`, `ReturnValue`,
/// `Builtin`) exist only at runtime and cannot appear in compiled bytecode.
/// Attempting to serialize them produces an error.
#[derive(Serialize, Deserialize)]
enum SerializableObject {
    Null,
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    F32(f32),
    F64(f64),
    Boolean(bool),
    String(String),
    Array(Vec<Object>),
    Hash(HashObject),
    CompiledFunction(CompiledFunction),
    Closure(Closure),
}

impl Serialize for Object {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let obj = match self {
            Self::Null => SerializableObject::Null,
            Self::I8(v) => SerializableObject::I8(*v),
            Self::I16(v) => SerializableObject::I16(*v),
            Self::I32(v) => SerializableObject::I32(*v),
            Self::I64(v) => SerializableObject::I64(*v),
            Self::I128(v) => SerializableObject::I128(*v),
            Self::Isize(v) => SerializableObject::Isize(*v),
            Self::U8(v) => SerializableObject::U8(*v),
            Self::U16(v) => SerializableObject::U16(*v),
            Self::U32(v) => SerializableObject::U32(*v),
            Self::U64(v) => SerializableObject::U64(*v),
            Self::U128(v) => SerializableObject::U128(*v),
            Self::Usize(v) => SerializableObject::Usize(*v),
            Self::F32(v) => SerializableObject::F32(*v),
            Self::F64(v) => SerializableObject::F64(*v),
            Self::Boolean(v) => SerializableObject::Boolean(*v),
            Self::String(v) => SerializableObject::String(v.clone()),
            Self::Array(v) => SerializableObject::Array(v.clone()),
            Self::Hash(v) => SerializableObject::Hash(v.clone()),
            Self::CompiledFunction(v) => SerializableObject::CompiledFunction(v.clone()),
            Self::Closure(v) => SerializableObject::Closure(v.clone()),
            other => {
                return Err(serde::ser::Error::custom(format!(
                    "non-serializable object type: {}",
                    other.type_name()
                )));
            }
        };
        obj.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        SerializableObject::deserialize(deserializer).map(|obj| match obj {
            SerializableObject::Null => Self::Null,
            SerializableObject::I8(v) => Self::I8(v),
            SerializableObject::I16(v) => Self::I16(v),
            SerializableObject::I32(v) => Self::I32(v),
            SerializableObject::I64(v) => Self::I64(v),
            SerializableObject::I128(v) => Self::I128(v),
            SerializableObject::Isize(v) => Self::Isize(v),
            SerializableObject::U8(v) => Self::U8(v),
            SerializableObject::U16(v) => Self::U16(v),
            SerializableObject::U32(v) => Self::U32(v),
            SerializableObject::U64(v) => Self::U64(v),
            SerializableObject::U128(v) => Self::U128(v),
            SerializableObject::Usize(v) => Self::Usize(v),
            SerializableObject::F32(v) => Self::F32(v),
            SerializableObject::F64(v) => Self::F64(v),
            SerializableObject::Boolean(v) => Self::Boolean(v),
            SerializableObject::String(v) => Self::String(v),
            SerializableObject::Array(v) => Self::Array(v),
            SerializableObject::Hash(v) => Self::Hash(v),
            SerializableObject::CompiledFunction(v) => Self::CompiledFunction(v),
            SerializableObject::Closure(v) => Self::Closure(v),
        })
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        use Object::*;
        match (self, other) {
            (Null, Null) => true,
            (I8(a), I8(b)) => a == b,
            (I16(a), I16(b)) => a == b,
            (I32(a), I32(b)) => a == b,
            (I64(a), I64(b)) => a == b,
            (I128(a), I128(b)) => a == b,
            (Isize(a), Isize(b)) => a == b,
            (U8(a), U8(b)) => a == b,
            (U16(a), U16(b)) => a == b,
            (U32(a), U32(b)) => a == b,
            (U64(a), U64(b)) => a == b,
            (U128(a), U128(b)) => a == b,
            (Usize(a), Usize(b)) => a == b,
            (F32(a), F32(b)) => a.total_cmp(b).is_eq(),
            (F64(a), F64(b)) => a.total_cmp(b).is_eq(),
            (Boolean(a), Boolean(b)) => a == b,
            (String(a), String(b)) => a == b,
            (Array(a1), Array(a2)) => a1 == a2,
            (Hash(h1), Hash(h2)) => h1 == h2,
            (Function(f1), Function(f2)) => f1 == f2,
            (Macro(m1), Macro(m2)) => m1 == m2,
            (Quote(q1), Quote(q2)) => q1 == q2,
            (ReturnValue(o1), ReturnValue(o2)) => o1 == o2,
            (Builtin(f1), Builtin(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            (CompiledFunction(c1), CompiledFunction(c2)) => c1 == c2,
            (Closure(c1), Closure(c2)) => c1 == c2,
            _ => false,
        }
    }
}

/// Represents a function object.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub env: Env,
}

/// Represents a macro object.
#[derive(Debug, Clone, PartialEq)]
pub struct Macro {
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub env: Env,
}

/// Represents a quoted AST node.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub node: Node,
}

/// A compiled function object containing bytecode instructions.
///
/// Functions are compiled into bytecode and stored in the constant pool.
/// The VM creates a new call frame for each invocation, using the
/// `num_locals` field to reserve stack space for local bindings.
///
/// Instructions are stored behind `Rc<[u8]>` so that closures created
/// from the same function literal share instruction memory rather than
/// cloning the entire byte vector on every call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledFunction {
    /// The bytecode instructions for this function's body.
    ///
    /// Reference-counted to allow zero-copy sharing across closures
    /// instantiated from the same compiled function.
    pub instructions: Rc<[u8]>,
    /// The number of local bindings (parameters + let bindings) in this function.
    pub num_locals: usize,
    /// The number of parameters this function expects.
    pub num_parameters: usize,
    /// Maps instruction byte offsets to source spans within this function.
    pub source_map: SourceMap,
}

/// A closure binding a compiled function with its captured free variables.
///
/// At runtime, every function is wrapped in a closure, even those with zero
/// free variables. This uniform representation simplifies the VM's call
/// dispatch: there is a single code path for invoking user-defined functions.
///
/// Free variables are resolved at closure-creation time (`OpClosure`) and
/// stored by value. Nested closures capture through the chain: an inner
/// closure's free variable may itself be a free variable of its enclosing
/// closure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Closure {
    /// The underlying compiled function.
    pub func: CompiledFunction,
    /// Captured free variables from enclosing scopes.
    pub free_vars: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HashObject {
    pub pairs: IndexMap<Hashable, Object>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hashable {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    Boolean(bool),
    String(String),
}

impl TryFrom<Object> for Hashable {
    type Error = Error;

    fn try_from(value: Object) -> Result<Self> {
        match value {
            Object::I8(i) => Ok(Self::I8(i)),
            Object::I16(i) => Ok(Self::I16(i)),
            Object::I32(i) => Ok(Self::I32(i)),
            Object::I64(i) => Ok(Self::I64(i)),
            Object::I128(i) => Ok(Self::I128(i)),
            Object::Isize(i) => Ok(Self::Isize(i)),
            Object::U8(i) => Ok(Self::U8(i)),
            Object::U16(i) => Ok(Self::U16(i)),
            Object::U32(i) => Ok(Self::U32(i)),
            Object::U64(i) => Ok(Self::U64(i)),
            Object::U128(i) => Ok(Self::U128(i)),
            Object::Usize(i) => Ok(Self::Usize(i)),
            Object::Boolean(b) => Ok(Self::Boolean(b)),
            Object::String(s) => Ok(Self::String(s)),
            obj => Err(EvalError::NotHashable(obj.type_name().to_owned()).into()),
        }
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::I8(v) => v.fmt(f),
            Self::I16(v) => v.fmt(f),
            Self::I32(v) => v.fmt(f),
            Self::I64(v) => v.fmt(f),
            Self::I128(v) => v.fmt(f),
            Self::Isize(v) => v.fmt(f),
            Self::U8(v) => v.fmt(f),
            Self::U16(v) => v.fmt(f),
            Self::U32(v) => v.fmt(f),
            Self::U64(v) => v.fmt(f),
            Self::U128(v) => v.fmt(f),
            Self::Usize(v) => v.fmt(f),
            Self::F32(v) => v.fmt(f),
            Self::F64(v) => v.fmt(f),
            Self::Boolean(boolean) => boolean.fmt(f),
            Self::String(string) => string.fmt(f),
            Self::Array(array) => {
                write!(
                    f,
                    "[{}]",
                    array
                        .iter()
                        .map(|obj| format!("{obj}"))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Self::Hash(hash) => hash.fmt(f),
            Self::Function(func) => func.fmt(f),
            Self::Macro(macro_obj) => macro_obj.fmt(f),
            Self::Quote(quote) => quote.fmt(f),
            Self::ReturnValue(ret_val) => ret_val.fmt(f),
            Self::Builtin(_) => write!(f, "builtin function"),
            Self::CompiledFunction(cf) => write!(f, "CompiledFunction[{:p}]", cf),
            Self::Closure(cl) => write!(f, "Closure[{:p}]", &cl.func),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macro({}) {{\n{}\n}}", self.params.join(", "), self.body)
    }
}

impl fmt::Display for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote({})", self.node)
    }
}

impl fmt::Display for HashObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}",
            self.pairs
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl fmt::Display for Hashable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8(v) => v.fmt(f),
            Self::I16(v) => v.fmt(f),
            Self::I32(v) => v.fmt(f),
            Self::I64(v) => v.fmt(f),
            Self::I128(v) => v.fmt(f),
            Self::Isize(v) => v.fmt(f),
            Self::U8(v) => v.fmt(f),
            Self::U16(v) => v.fmt(f),
            Self::U32(v) => v.fmt(f),
            Self::U64(v) => v.fmt(f),
            Self::U128(v) => v.fmt(f),
            Self::Usize(v) => v.fmt(f),
            Self::Boolean(b) => b.fmt(f),
            Self::String(s) => s.fmt(f),
        }
    }
}
