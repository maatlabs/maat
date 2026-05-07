//! Code generation for the Maat compiler.
//!
//! This crate translates AST nodes into bytecode instructions that can be
//! executed by the virtual machine. The compiler performs a single-pass
//! traversal of the AST, emitting stack-based bytecode operations.

#![forbid(unsafe_code)]

mod compile;
mod registry;
mod symbol;

pub use compile::Compiler;
pub use symbol::{Symbol, SymbolScope, SymbolsTable};

#[cfg(test)]
mod tests {
    use maat_ast::{Expr, ExprStmt, MaatAst, NumKind, Number, PrefixExpr, Program, Radix, Stmt};
    use maat_bytecode::{MAX_CONSTANT_POOL_SIZE, Opcode};
    use maat_errors::{CompileError, CompileErrorKind, Error};
    use maat_runtime::{Integer, Value};
    use maat_span::Span;

    use super::Compiler;

    #[test]
    fn constant_pool_overflow() {
        let mut compiler = Compiler::new();
        for i in 0..=MAX_CONSTANT_POOL_SIZE as i64 {
            let result = compiler.add_constant(Value::Integer(Integer::I64(i)));
            assert!(result.is_ok(), "should succeed for index {i}");
        }
        let result = compiler.add_constant(Value::Integer(Integer::I64(999)));
        assert!(
            result.is_err(),
            "should fail when exceeding MAX_CONSTANT_POOL_SIZE"
        );
        match result.unwrap_err() {
            Error::Compile(CompileError {
                kind: CompileErrorKind::ConstantPoolOverflow { max, attempted },
                ..
            }) => {
                assert_eq!(max, MAX_CONSTANT_POOL_SIZE);
                assert_eq!(attempted, MAX_CONSTANT_POOL_SIZE + 1);
            }
            other => panic!("expected ConstantPoolOverflow, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_prefix_operator() {
        let expr = Expr::Prefix(PrefixExpr {
            operator: "~".to_string(),
            operand: Box::new(Expr::Number(Number {
                kind: NumKind::I64,
                value: 5,
                radix: Radix::Dec,
                span: Span::ZERO,
            })),
            span: Span::ZERO,
        });
        let program = Program {
            statements: vec![Stmt::Expr(ExprStmt {
                value: expr,
                span: Span::ZERO,
            })],
        };
        let mut compiler = Compiler::new();
        let result = compiler.compile(&MaatAst::Program(program));
        assert!(
            result.is_err(),
            "should fail on unsupported prefix operator"
        );
        match result.unwrap_err() {
            Error::Compile(CompileError {
                kind: CompileErrorKind::UnsupportedOperator { operator, context },
                ..
            }) => {
                assert_eq!(operator, "~");
                assert_eq!(context, "prefix expression");
            }
            other => panic!("expected UnsupportedOperator, got {:?}", other),
        }
    }

    #[test]
    fn scopes() {
        let mut compiler = Compiler::new();
        assert_eq!(compiler.scope_index, 0);
        compiler.emit(Opcode::Mul, &[], Span::ZERO);
        compiler.enter_scope();
        assert_eq!(compiler.scope_index, 1);
        compiler.emit(Opcode::Sub, &[], Span::ZERO);
        assert_eq!(compiler.scopes[compiler.scope_index].instructions.len(), 1);
        assert_eq!(
            compiler.scopes[compiler.scope_index]
                .last_instruction
                .unwrap()
                .opcode,
            Opcode::Sub
        );
        let (instructions, _source_map) = compiler.leave_scope().expect("should leave scope");
        assert_eq!(compiler.scope_index, 0);
        assert_eq!(instructions.len(), 1);
        assert_eq!(
            compiler.scopes[compiler.scope_index]
                .last_instruction
                .unwrap()
                .opcode,
            Opcode::Mul
        );
    }
}
