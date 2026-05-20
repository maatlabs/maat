//! Interpreter-specific runtime value system.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use maat_ast::{BlockStmt, MaatAst, Number};
use maat_runtime::{
    BuiltinArg, BuiltinFn, BuiltinReturn, Felt, Hashable, Integer, Map, Relocatable, Set, Value,
};
use maat_span::Span;

/// Interpreter-only value form.
///
/// The interpreter operates on `EvalValue` throughout. Conversion to the
/// runtime [`Value`] happens only at the `unquote` reification boundary (via
/// [`Self::to_ast_node`]) and at the builtin call boundary.
#[derive(Debug, Clone)]
pub enum EvalValue {
    Unit,
    Integer(Integer),
    Felt(Felt),
    Bool(bool),
    Char(char),
    Str(String),
    Tuple(Vec<EvalValue>),
    Vector(Vec<EvalValue>),
    Array(Vec<EvalValue>),
    Map(EvalMap),
    Set(EvalSet),
    Range(Integer, Integer),
    RangeInclusive(Integer, Integer),
    Function(EvalFunction),
    Macro(EvalMacro),
    Quote(Box<EvalQuote>),
    ReturnValue(Box<EvalValue>),
    Break(Box<EvalValue>),
    Continue,
    Builtin(BuiltinFn),
    Relocatable(Relocatable),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvalMap {
    pub pairs: IndexMap<Hashable, EvalValue>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvalSet(pub IndexSet<Hashable>);

#[derive(Debug, Clone, PartialEq)]
pub struct EvalFunction {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: EvalEnv,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalMacro {
    pub params: Vec<String>,
    pub body: BlockStmt,
    pub env: EvalEnv,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalQuote {
    pub node: MaatAst,
}

#[derive(Debug, Clone, Default)]
pub struct EvalEnv {
    inner: Rc<RefCell<EvalEnvInner>>,
}

#[derive(Debug, Default)]
struct EvalEnvInner {
    store: IndexMap<String, EvalValue>,
    outer: Option<EvalEnv>,
}

impl EvalEnv {
    pub fn new_enclosed(outer: &Self) -> Self {
        Self {
            inner: Rc::new(RefCell::new(EvalEnvInner {
                store: IndexMap::new(),
                outer: Some(outer.clone()),
            })),
        }
    }

    pub fn get(&self, name: &str) -> Option<EvalValue> {
        let inner = self.inner.borrow();
        match inner.store.get(name) {
            Some(val) => Some(val.clone()),
            None => inner.outer.as_ref().and_then(|outer| outer.get(name)),
        }
    }

    pub fn set(&self, name: String, value: &EvalValue) {
        self.inner.borrow_mut().store.insert(name, value.clone());
    }

    pub fn update(&self, name: String, value: &EvalValue) {
        if self.inner.borrow().store.contains_key(&name) {
            self.inner.borrow_mut().store.insert(name, value.clone());
            return;
        }
        if let Some(outer) = self.inner.borrow().outer.as_ref().cloned()
            && outer.contains(&name)
        {
            outer.update(name, value);
            return;
        }
        self.inner.borrow_mut().store.insert(name, value.clone());
    }

    fn contains(&self, name: &str) -> bool {
        let inner = self.inner.borrow();
        inner.store.contains_key(name)
            || inner
                .outer
                .as_ref()
                .is_some_and(|outer| outer.contains(name))
    }
}

impl PartialEq for EvalEnv {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl EvalValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "()",
            Self::Integer(n) => n.type_name(),
            Self::Felt(_) => "Felt",
            Self::Bool(_) => "bool",
            Self::Char(_) => "char",
            Self::Str(_) => "str",
            Self::Tuple(_) => "tuple",
            Self::Vector(_) => "Vector",
            Self::Array(_) => "Array",
            Self::Map(_) => "Map",
            Self::Set(_) => "Set",
            Self::Range(..) => "Range",
            Self::RangeInclusive(..) => "RangeInclusive",
            Self::Function(_) => "fn",
            Self::Macro(_) => "macro",
            Self::Quote(_) => "quote",
            Self::ReturnValue(_) => "return",
            Self::Break(_) => "break",
            Self::Continue => "continue",
            Self::Builtin(_) => "fn",
            Self::Relocatable(_) => "Relocatable",
        }
    }

    #[inline]
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Self::Bool(false))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn to_vector_index(&self) -> Option<usize> {
        match self {
            Self::Integer(n) => n.to_usize(),
            _ => None,
        }
    }

    pub fn from_number_literal(lit: &Number) -> std::result::Result<Self, String> {
        Value::from_number_literal(lit).map(|v| match v {
            Value::Integer(i) => Self::Integer(i),
            Value::Felt(f) => Self::Felt(f),
            other => unreachable!(
                "Value::from_number_literal produced non-numeric Value: {}",
                other.type_name()
            ),
        })
    }

    pub fn to_ast_node(val: &Self) -> Option<MaatAst> {
        use maat_ast::{BoolLit, Expr, Radix};

        match val {
            Self::Integer(i) => {
                let (kind, value) = i.to_ast_literal()?;
                Some(MaatAst::Expr(Expr::Number(Number {
                    kind,
                    value,
                    radix: Radix::Dec,
                    span: Span::ZERO,
                })))
            }
            Self::Bool(b) => Some(MaatAst::Expr(Expr::Bool(BoolLit {
                value: *b,
                span: Span::ZERO,
            }))),
            Self::Quote(q) => Some(q.node.clone()),
            _ => None,
        }
    }
}

impl PartialEq for EvalValue {
    fn eq(&self, other: &Self) -> bool {
        use EvalValue::*;
        match (self, other) {
            (Unit, Unit) => true,
            (Integer(a), Integer(b)) => a == b,
            (Felt(a), Felt(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Char(a), Char(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Tuple(a), Tuple(b)) => a == b,
            (Vector(a), Vector(b)) => a == b,
            (Array(a), Array(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (Set(a), Set(b)) => a == b,
            (Range(s1, e1), Range(s2, e2)) => s1 == s2 && e1 == e2,
            (RangeInclusive(s1, e1), RangeInclusive(s2, e2)) => s1 == s2 && e1 == e2,
            (Function(a), Function(b)) => a == b,
            (Macro(a), Macro(b)) => a == b,
            (Quote(a), Quote(b)) => a == b,
            (ReturnValue(a), ReturnValue(b)) => a == b,
            (Break(a), Break(b)) => a == b,
            (Continue, Continue) => true,
            (Builtin(a), Builtin(b)) => std::ptr::fn_addr_eq(*a, *b),
            (Relocatable(a), Relocatable(b)) => a == b,
            _ => false,
        }
    }
}

fn write_comma_separated<I, T>(f: &mut fmt::Formatter<'_>, iter: I) -> fmt::Result
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

impl fmt::Display for EvalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("()"),
            Self::Integer(v) => v.fmt(f),
            Self::Felt(v) => v.fmt(f),
            Self::Bool(v) => v.fmt(f),
            Self::Char(v) => v.fmt(f),
            Self::Str(s) => s.fmt(f),
            Self::Tuple(elems) => {
                f.write_str("(")?;
                write_comma_separated(f, elems)?;
                f.write_str(")")
            }
            Self::Vector(elems) | Self::Array(elems) => {
                f.write_str("[")?;
                write_comma_separated(f, elems)?;
                f.write_str("]")
            }
            Self::Map(m) => m.fmt(f),
            Self::Set(s) => s.fmt(f),
            Self::Range(s, e) => write!(f, "{s}..{e}"),
            Self::RangeInclusive(s, e) => write!(f, "{s}..={e}"),
            Self::Function(func) => func.fmt(f),
            Self::Macro(m) => m.fmt(f),
            Self::Quote(q) => q.fmt(f),
            Self::ReturnValue(v) => v.fmt(f),
            Self::Break(v) => write!(f, "break {v}"),
            Self::Continue => f.write_str("continue"),
            Self::Builtin(_) => f.write_str("builtin function"),
            Self::Relocatable(r) => r.fmt(f),
        }
    }
}

impl fmt::Display for EvalMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        let mut iter = self.pairs.iter();
        if let Some((k, v)) = iter.next() {
            write!(f, "{k}: {v}")?;
            for (k, v) in iter {
                write!(f, ", {k}: {v}")?;
            }
        }
        f.write_str("}")
    }
}

impl fmt::Display for EvalSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Set({")?;
        write_comma_separated(f, &self.0)?;
        f.write_str("})")
    }
}

impl fmt::Display for EvalFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fn(")?;
        write_comma_separated(f, &self.params)?;
        write!(f, ") {{\n{}\n}}", self.body)
    }
}

impl fmt::Display for EvalMacro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("macro(")?;
        write_comma_separated(f, &self.params)?;
        write!(f, ") {{\n{}\n}}", self.body)
    }
}

impl fmt::Display for EvalQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "quote({})", self.node)
    }
}

impl TryFrom<EvalValue> for Hashable {
    type Error = maat_errors::Error;

    fn try_from(value: EvalValue) -> Result<Self, Self::Error> {
        match value {
            EvalValue::Integer(i) => Ok(Self::Integer(i)),
            EvalValue::Felt(f) => Ok(Self::Felt(f.as_int())),
            EvalValue::Bool(b) => Ok(Self::Bool(b)),
            EvalValue::Char(c) => Ok(Self::Char(c)),
            EvalValue::Str(s) => Ok(Self::Str(s)),
            other => Err(maat_errors::EvalError::NotHashable(other.type_name().to_owned()).into()),
        }
    }
}

pub fn eval_value_to_builtin_arg<'a>(
    value: &'a EvalValue,
    base_vec: &'a [Value],
    base_map: &'a Map,
    base_set: &'a Set,
) -> BuiltinArg<'a> {
    match value {
        EvalValue::Unit => BuiltinArg::Unit,
        EvalValue::Integer(i) => BuiltinArg::Integer(*i),
        EvalValue::Felt(f) => BuiltinArg::Felt(*f),
        EvalValue::Bool(b) => BuiltinArg::Bool(*b),
        EvalValue::Char(c) => BuiltinArg::Char(*c),
        EvalValue::Str(s) => BuiltinArg::Str(s.as_str()),
        EvalValue::Vector(_) => BuiltinArg::Vector(base_vec),
        EvalValue::Tuple(_) => BuiltinArg::Tuple(base_vec),
        EvalValue::Array(_) => BuiltinArg::Array(base_vec),
        EvalValue::Map(_) => BuiltinArg::Map(base_map),
        EvalValue::Set(_) => BuiltinArg::Set(base_set),
        EvalValue::Range(s, e) => BuiltinArg::Range(*s, *e),
        EvalValue::RangeInclusive(s, e) => BuiltinArg::RangeInclusive(*s, *e),
        EvalValue::Builtin(f) => BuiltinArg::Builtin(*f),
        EvalValue::Relocatable(r) => BuiltinArg::Relocatable(*r),
        EvalValue::Function(_)
        | EvalValue::Macro(_)
        | EvalValue::Quote(_)
        | EvalValue::ReturnValue(_)
        | EvalValue::Break(_)
        | EvalValue::Continue => {
            unreachable!(
                "interpreter-only EvalValue::{} cannot be passed to a builtin",
                value.type_name()
            )
        }
    }
}

pub fn return_to_eval_value(ret: BuiltinReturn) -> EvalValue {
    match ret {
        BuiltinReturn::Value(v) => value_to_eval_value(v),
        BuiltinReturn::Str(s) => EvalValue::Str(s),
        BuiltinReturn::Vector(entries) => {
            let elements = entries.into_iter().map(return_to_eval_value).collect();
            EvalValue::Vector(elements)
        }
    }
}

pub fn value_to_eval_value(value: Value) -> EvalValue {
    match value {
        Value::Unit => EvalValue::Unit,
        Value::Integer(i) => EvalValue::Integer(i),
        Value::Felt(f) => EvalValue::Felt(f),
        Value::Bool(b) => EvalValue::Bool(b),
        Value::Char(c) => EvalValue::Char(c),
        Value::Str(s) => EvalValue::Str(s),
        Value::Tuple(t) => EvalValue::Tuple(t.into_iter().map(value_to_eval_value).collect()),
        Value::Array(a) => EvalValue::Array(a.into_iter().map(value_to_eval_value).collect()),
        Value::Map(m) => {
            let pairs = m
                .pairs
                .into_iter()
                .map(|(k, v)| (k, value_to_eval_value(v)))
                .collect();
            EvalValue::Map(EvalMap { pairs })
        }
        Value::Set(s) => EvalValue::Set(EvalSet(s.0)),
        Value::Range(s, e) => EvalValue::Range(s, e),
        Value::RangeInclusive(s, e) => EvalValue::RangeInclusive(s, e),
        Value::Builtin(f) => EvalValue::Builtin(f),
        Value::Relocatable(r) => EvalValue::Relocatable(r),
        Value::Struct(_) | Value::EnumVariant(_) => EvalValue::Unit,
        Value::CompiledFn(_) | Value::Closure(_) => EvalValue::Unit,
        Value::Vector { .. } => EvalValue::Unit,
    }
}

pub fn eval_value_to_value(value: EvalValue) -> Value {
    match value {
        EvalValue::Unit => Value::Unit,
        EvalValue::Integer(i) => Value::Integer(i),
        EvalValue::Felt(f) => Value::Felt(f),
        EvalValue::Bool(b) => Value::Bool(b),
        EvalValue::Char(c) => Value::Char(c),
        EvalValue::Str(s) => Value::Str(s),
        EvalValue::Tuple(t) => Value::Tuple(t.into_iter().map(eval_value_to_value).collect()),
        EvalValue::Array(a) => Value::Array(a.into_iter().map(eval_value_to_value).collect()),
        EvalValue::Vector(_) => Value::Unit,
        EvalValue::Map(m) => {
            let pairs = m
                .pairs
                .into_iter()
                .map(|(k, v)| (k, eval_value_to_value(v)))
                .collect();
            Value::Map(maat_runtime::Map { pairs })
        }
        EvalValue::Set(s) => Value::Set(maat_runtime::Set(s.0)),
        EvalValue::Range(s, e) => Value::Range(s, e),
        EvalValue::RangeInclusive(s, e) => Value::RangeInclusive(s, e),
        EvalValue::Builtin(f) => Value::Builtin(f),
        EvalValue::Relocatable(r) => Value::Relocatable(r),
        EvalValue::Function(_)
        | EvalValue::Macro(_)
        | EvalValue::Quote(_)
        | EvalValue::ReturnValue(_)
        | EvalValue::Break(_)
        | EvalValue::Continue => Value::Unit,
    }
}
