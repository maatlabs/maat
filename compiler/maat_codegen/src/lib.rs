//! Code generation for the compiler.
//!
//! This crate translates AST nodes into bytecode instructions that can be
//! executed by the virtual machine. The compiler performs a single-pass
//! traversal of the AST, emitting stack-based bytecode operations.

use maat_ast::{Expression, Node, Program, Statement};
use maat_bytecode::{Bytecode, Instructions, MAX_CONSTANT_POOL_SIZE, Opcode, encode};
use maat_errors::{CompileError, Result};
use maat_eval::Object;

/// Compiler state for generating bytecode from AST nodes.
///
/// The compiler performs a recursive descent through the AST, emitting
/// bytecode instructions and tracking constants in a separate pool.
#[derive(Debug, Clone)]
pub struct Compiler {
    instructions: Instructions,
    constants: Vec<Object>,
}

impl Compiler {
    /// Creates a new compiler with empty instruction stream and constant pool.
    pub fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            constants: Vec::new(),
        }
    }

    /// Compiles an AST node into bytecode.
    ///
    /// This recursively traverses the AST, emitting instructions for each node.
    /// Returns an error if an unsupported node type or operator is encountered.
    pub fn compile(&mut self, node: &Node) -> Result<()> {
        match node {
            Node::Program(program) => self.compile_program(program),
            Node::Statement(stmt) => self.compile_statement(stmt),
            Node::Expression(expr) => self.compile_expression(expr),
        }
    }

    /// Compiles a program node (list of statements).
    fn compile_program(&mut self, program: &Program) -> Result<()> {
        for stmt in &program.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    /// Compiles a statement node.
    fn compile_statement(&mut self, stmt: &Statement) -> Result<()> {
        match stmt {
            Statement::Expression(expr_stmt) => {
                self.compile_expression(&expr_stmt.value)?;
                self.emit(Opcode::Pop, &[]);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Compiles an expression node into bytecode.
    fn compile_expression(&mut self, expr: &Expression) -> Result<()> {
        match expr {
            Expression::I64(int_lit) => {
                let constant = Object::I64(int_lit.value);
                let index = self.add_constant(constant)?;
                self.emit(Opcode::Constant, &[index]);
                Ok(())
            }

            Expression::Boolean(value) => {
                let opcode = if *value { Opcode::True } else { Opcode::False };
                self.emit(opcode, &[]);
                Ok(())
            }

            Expression::Prefix(prefix_expr) => {
                self.compile_expression(&prefix_expr.operand)?;

                let opcode = match prefix_expr.operator.as_str() {
                    "!" => Opcode::Bang,
                    "-" => Opcode::Minus,
                    op => {
                        return Err(CompileError::UnsupportedOperator {
                            operator: op.to_string(),
                            context: "prefix expression".to_string(),
                        }
                        .into());
                    }
                };

                self.emit(opcode, &[]);
                Ok(())
            }

            Expression::Infix(infix_expr) => {
                self.compile_expression(&infix_expr.lhs)?;
                self.compile_expression(&infix_expr.rhs)?;

                let opcode = match infix_expr.operator.as_str() {
                    "+" => Opcode::Add,
                    "-" => Opcode::Sub,
                    "*" => Opcode::Mul,
                    "/" => Opcode::Div,
                    ">" => Opcode::GreaterThan,
                    "<" => Opcode::LessThan,
                    "==" => Opcode::Equal,
                    "!=" => Opcode::NotEqual,
                    op => {
                        return Err(CompileError::UnsupportedOperator {
                            operator: op.to_string(),
                            context: "infix expression".to_string(),
                        }
                        .into());
                    }
                };

                self.emit(opcode, &[]);
                Ok(())
            }

            expr => Err(CompileError::UnsupportedExpression {
                expr_type: expr.type_name().to_string(),
            }
            .into()),
        }
    }

    /// Adds a constant value to the constant pool and returns its index.
    ///
    /// # Errors
    ///
    /// Returns `CompileError::ConstantPoolOverflow` if adding this constant
    /// would exceed the maximum constant pool size.
    fn add_constant(&mut self, obj: Object) -> Result<usize> {
        self.constants.push(obj);
        let index = self.constants.len() - 1;

        if index > MAX_CONSTANT_POOL_SIZE {
            return Err(CompileError::ConstantPoolOverflow {
                max: MAX_CONSTANT_POOL_SIZE,
                attempted: index,
            }
            .into());
        }

        Ok(index)
    }

    /// Emits a bytecode instruction with the given opcode and operands.
    ///
    /// Returns the starting position of the emitted instruction.
    fn emit(&mut self, opcode: Opcode, operands: &[usize]) -> usize {
        let instruction = encode(opcode, operands);
        self.add_instruction(&instruction)
    }

    /// Appends instruction bytes to the instruction stream.
    ///
    /// Returns the position where the instruction was inserted.
    fn add_instruction(&mut self, instruction: &[u8]) -> usize {
        let pos = self.instructions.len();
        self.instructions
            .extend(&Instructions::from(instruction.to_vec()));
        pos
    }

    /// Extracts the compiled bytecode and constants.
    ///
    /// This consumes the compiler instance and returns the final bytecode output.
    pub fn bytecode(self) -> Bytecode {
        Bytecode {
            instructions: self.instructions,
            constants: self.constants,
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use maat_errors::Error;
    use maat_lexer::Lexer;
    use maat_parse::Parser;

    use super::*;

    fn parse(input: &str) -> Program {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        parser.parse_program()
    }

    fn compile_program(input: &str) -> Bytecode {
        let program = parse(input);
        let mut compiler = Compiler::new();
        compiler
            .compile(&Node::Program(program))
            .expect("compilation failed");
        compiler.bytecode()
    }

    fn concat_instructions(instructions: &[Vec<u8>]) -> Instructions {
        let mut result = Instructions::new();
        for ins in instructions {
            result.extend(&Instructions::from(ins.clone()));
        }
        result
    }

    #[test]
    fn compile_integer_arithmetic() {
        let cases = vec![
            (
                "1 + 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Add, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1; 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Pop, &[]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1 - 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Sub, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1 * 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Mul, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "2 / 1",
                vec![2, 1],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Div, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "-1",
                vec![1],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Minus, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
        ];

        for (input, expected_constants, expected_instructions) in cases {
            let bytecode = compile_program(input);

            let expected_ins = concat_instructions(&expected_instructions);
            assert_eq!(
                bytecode.instructions.as_bytes(),
                expected_ins.as_bytes(),
                "wrong instructions for input: {input}"
            );

            assert_eq!(
                bytecode.constants.len(),
                expected_constants.len(),
                "wrong number of constants for input: {input}"
            );

            for (i, expected) in expected_constants.iter().enumerate() {
                match &bytecode.constants[i] {
                    Object::I64(value) => {
                        assert_eq!(*value, *expected, "constant {i} wrong for input: {input}")
                    }
                    _ => panic!("expected integer constant"),
                }
            }
        }
    }

    #[test]
    fn compile_boolean_expressions() {
        let tests = vec![
            (
                "true",
                vec![],
                vec![encode(Opcode::True, &[]), encode(Opcode::Pop, &[])],
            ),
            (
                "false",
                vec![],
                vec![encode(Opcode::False, &[]), encode(Opcode::Pop, &[])],
            ),
            (
                "1 > 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::GreaterThan, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1 < 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::LessThan, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1 == 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::Equal, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "1 != 2",
                vec![1, 2],
                vec![
                    encode(Opcode::Constant, &[0]),
                    encode(Opcode::Constant, &[1]),
                    encode(Opcode::NotEqual, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "true == false",
                vec![],
                vec![
                    encode(Opcode::True, &[]),
                    encode(Opcode::False, &[]),
                    encode(Opcode::Equal, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "true != false",
                vec![],
                vec![
                    encode(Opcode::True, &[]),
                    encode(Opcode::False, &[]),
                    encode(Opcode::NotEqual, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
            (
                "!true",
                vec![],
                vec![
                    encode(Opcode::True, &[]),
                    encode(Opcode::Bang, &[]),
                    encode(Opcode::Pop, &[]),
                ],
            ),
        ];

        for (input, expected_constants, expected_instructions) in tests {
            let bytecode = compile_program(input);

            let expected_ins = concat_instructions(&expected_instructions);
            assert_eq!(
                bytecode.instructions.as_bytes(),
                expected_ins.as_bytes(),
                "wrong instructions for input: {input}"
            );

            assert_eq!(
                bytecode.constants.len(),
                expected_constants.len(),
                "wrong number of constants for input: {input}"
            );
        }
    }

    #[test]
    fn constant_pool_overflow() {
        let mut compiler = Compiler::new();

        // Add constants up to the limit
        for i in 0..=MAX_CONSTANT_POOL_SIZE as i64 {
            let result = compiler.add_constant(Object::I64(i));
            assert!(result.is_ok(), "should succeed for index {i}");
        }

        // Next constant should overflow
        let result = compiler.add_constant(Object::I64(999));
        assert!(
            result.is_err(),
            "should fail when exceeding MAX_CONSTANT_POOL_SIZE"
        );

        match result.unwrap_err() {
            Error::Compile(CompileError::ConstantPoolOverflow { max, attempted }) => {
                assert_eq!(max, MAX_CONSTANT_POOL_SIZE);
                assert_eq!(attempted, MAX_CONSTANT_POOL_SIZE + 1);
            }
            other => panic!("expected ConstantPoolOverflow, got {:?}", other),
        }
    }

    #[test]
    fn unsupported_prefix_operator() {
        use maat_ast::{ExpressionStatement, I64, PrefixExpr, Radix};

        let expr = Expression::Prefix(PrefixExpr {
            operator: "~".to_string(), // Bitwise NOT - currently not supported
            operand: Box::new(Expression::I64(I64 {
                value: 5,
                radix: Radix::Dec,
            })),
        });

        let program = Program {
            statements: vec![Statement::Expression(ExpressionStatement { value: expr })],
        };

        let mut compiler = Compiler::new();
        let result = compiler.compile(&Node::Program(program));

        assert!(
            result.is_err(),
            "should fail on unsupported prefix operator"
        );

        match result.unwrap_err() {
            Error::Compile(CompileError::UnsupportedOperator { operator, context }) => {
                assert_eq!(operator, "~");
                assert_eq!(context, "prefix expression");
            }
            other => panic!("expected UnsupportedOperator, got {:?}", other),
        }
    }
}
