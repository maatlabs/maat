use maat_ast::{BlockStatement, Expression, Node, Program, Statement};
use maat_bytecode::{Bytecode, Instruction, Instructions, MAX_CONSTANT_POOL_SIZE, Opcode, encode};
use maat_errors::{CompileError, Result};
use maat_eval::Object;

use crate::symbol::Table;

/// Compiler state for generating bytecode from AST nodes.
///
/// The compiler performs a recursive descent through the AST, emitting
/// bytecode instructions and tracking constants in a separate pool.
/// It maintains a two-instruction history to support peephole operations
/// like removing trailing pops from block expressions.
#[derive(Debug, Clone)]
pub struct Compiler {
    instructions: Instructions,
    constants: Vec<Object>,
    last_instruction: Option<Instruction>,
    previous_instruction: Option<Instruction>,
    symbols_table: Table,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    const JUMP_DUMMY_TARGET: usize = 9999;

    /// Creates a new compiler with empty instruction stream and constant pool.
    pub fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            constants: Vec::new(),
            last_instruction: None,
            previous_instruction: None,
            symbols_table: Table::new(),
        }
    }

    /// Creates a compiler with existing symbols table and constants.
    ///
    /// This enables REPL sessions where variable definitions and constants
    /// persist across multiple compilation passes.
    pub fn with_state(symbols_table: Table, constants: Vec<Object>) -> Self {
        Self {
            instructions: Instructions::new(),
            constants,
            last_instruction: None,
            previous_instruction: None,
            symbols_table,
        }
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

    /// Returns a reference to the compiler's symbols table.
    ///
    /// Used for state persistence in REPL sessions where symbol definitions
    /// must carry over across compilation passes.
    pub fn symbols_table(&self) -> &Table {
        &self.symbols_table
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
            Statement::Block(block) => self.compile_block_statement(block),
            Statement::Let(let_stmt) => {
                self.compile_expression(&let_stmt.value)?;
                let index = self.symbols_table.define_symbol(&let_stmt.ident)?.index;
                self.emit(Opcode::SetGlobal, &[index]);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Compiles a sequence of statements.
    fn compile_block_statement(&mut self, block: &BlockStatement) -> Result<()> {
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
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

            Expression::Conditional(cond) => {
                self.compile_expression(&cond.condition)?;

                let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP_DUMMY_TARGET]);

                self.compile_block_statement(&cond.consequence)?;
                if self.last_instruction_is(Opcode::Pop) {
                    self.remove_last_pop();
                }

                let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP_DUMMY_TARGET]);

                let cons_pos = self.instructions.len();
                self.replace_operand(cond_jump_pos, cons_pos)?;

                match &cond.alternative {
                    None => {
                        self.emit(Opcode::Null, &[]);
                    }
                    Some(alt_block) => {
                        self.compile_block_statement(alt_block)?;
                        if self.last_instruction_is(Opcode::Pop) {
                            self.remove_last_pop();
                        }
                    }
                }

                let alt_pos = self.instructions.len();
                self.replace_operand(jump_pos, alt_pos)?;
                Ok(())
            }

            Expression::Identifier(name) => {
                let symbol = self
                    .symbols_table
                    .resolve_symbol(name)
                    .ok_or_else(|| CompileError::UndefinedVariable { name: name.clone() })?;
                self.emit(Opcode::GetGlobal, &[symbol.index]);
                Ok(())
            }

            Expression::String(value) => {
                let constant = Object::String(value.clone());
                let index = self.add_constant(constant)?;
                self.emit(Opcode::Constant, &[index]);
                Ok(())
            }

            Expression::Array(array) => {
                for element in &array.elements {
                    self.compile_expression(element)?;
                }
                self.emit(Opcode::Array, &[array.elements.len()]);
                Ok(())
            }

            Expression::Hash(hash) => {
                for (key, value) in &hash.pairs {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.emit(Opcode::Hash, &[hash.pairs.len() * 2]);
                Ok(())
            }

            Expression::Index(index_expr) => {
                self.compile_expression(&index_expr.expr)?;
                self.compile_expression(&index_expr.index)?;
                self.emit(Opcode::Index, &[]);
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
        let pos = self.add_instruction(&instruction);
        self.set_last_instruction(opcode, pos);
        pos
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

    /// Updates the last/previous instruction tracking.
    fn set_last_instruction(&mut self, opcode: Opcode, position: usize) {
        self.previous_instruction = self.last_instruction;
        self.last_instruction = Some(Instruction { opcode, position });
    }

    /// Returns `true` if the last emitted instruction matches the given opcode.
    fn last_instruction_is(&self, opcode: Opcode) -> bool {
        self.last_instruction
            .is_some_and(|last| last.opcode == opcode)
    }

    /// Removes the last `OpPop` instruction from the stream.
    ///
    /// This is used when compiling block expressions within conditionals:
    /// the block's expression statements emit `OpPop`, but conditionals
    /// need the value to remain on the stack.
    fn remove_last_pop(&mut self) {
        if let Some(last) = self.last_instruction {
            self.instructions.truncate(last.position);
            self.last_instruction = self.previous_instruction;
        }
    }

    /// Replaces the operand of an instruction at the given position.
    ///
    /// Re-encodes the full instruction (opcode + new operand) and patches
    /// it into the instruction stream. Used for back-patching forward jumps.
    fn replace_operand(&mut self, op_pos: usize, operand: usize) -> Result<()> {
        let byte = self.instructions.as_bytes()[op_pos];
        let op = Opcode::from_byte(byte).ok_or(CompileError::InvalidOpcode {
            opcode: byte,
            position: op_pos,
        })?;
        let new_inst = encode(op, &[operand]);
        self.instructions.replace_bytes(op_pos, &new_inst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use maat_errors::Error;

    use super::*;

    #[test]
    fn constant_pool_overflow() {
        let mut compiler = Compiler::new();

        for i in 0..=MAX_CONSTANT_POOL_SIZE as i64 {
            let result = compiler.add_constant(Object::I64(i));
            assert!(result.is_ok(), "should succeed for index {i}");
        }

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
            operator: "~".to_string(),
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
