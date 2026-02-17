use maat_ast::{BlockStatement, Expression, Node, Program, Statement};
use maat_bytecode::{Bytecode, Instruction, Instructions, MAX_CONSTANT_POOL_SIZE, Opcode, encode};
use maat_errors::{CompileError, Result};
use maat_eval::{BUILTINS, CompiledFunction, Object};

use crate::{SymbolScope, SymbolsTable};

/// Per-scope compilation state.
///
/// Each function body (and the top-level program) gets its own scope
/// with an independent instruction stream and instruction history
/// for peephole optimizations.
#[derive(Debug, Clone)]
struct CompilationScope {
    instructions: Instructions,
    last_instruction: Option<Instruction>,
    previous_instruction: Option<Instruction>,
}

impl CompilationScope {
    fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            last_instruction: None,
            previous_instruction: None,
        }
    }
}

/// Compiler state for generating bytecode from AST nodes.
///
/// The compiler performs a recursive descent through the AST, emitting
/// bytecode instructions and tracking constants in a separate pool.
/// It maintains a stack of compilation scopes to support nested function
/// bodies, each with its own instruction stream and peephole history.
#[derive(Debug, Clone)]
pub struct Compiler {
    constants: Vec<Object>,
    symbols_table: SymbolsTable,
    scopes: Vec<CompilationScope>,
    scope_index: usize,
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
        let mut symbols_table = SymbolsTable::new();
        Self::register_builtins(&mut symbols_table);

        Self {
            constants: Vec::new(),
            symbols_table,
            scopes: vec![CompilationScope::new()],
            scope_index: 0,
        }
    }

    /// Creates a compiler with existing symbols table and constants.
    ///
    /// This enables REPL sessions where variable definitions and constants
    /// persist across multiple compilation passes.
    pub fn with_state(mut symbols_table: SymbolsTable, constants: Vec<Object>) -> Self {
        Self::register_builtins(&mut symbols_table);

        Self {
            constants,
            symbols_table,
            scopes: vec![CompilationScope::new()],
            scope_index: 0,
        }
    }

    /// Registers all built-in functions in the given symbols table.
    fn register_builtins(table: &mut SymbolsTable) {
        for (i, (name, _)) in BUILTINS.iter().enumerate() {
            table.define_builtin(i, name);
        }
    }

    /// Extracts the compiled bytecode and constants.
    ///
    /// This consumes the compiler instance and returns the final bytecode output.
    ///
    /// # Errors
    ///
    /// Returns `CompileError::ScopeUnderflow` if the scope stack is empty,
    /// which indicates an internal compiler invariant violation.
    pub fn bytecode(mut self) -> Result<Bytecode> {
        let scope = self.scopes.pop().ok_or(CompileError::ScopeUnderflow)?;
        Ok(Bytecode {
            instructions: scope.instructions,
            constants: self.constants,
        })
    }

    /// Returns a reference to the compiler's symbols table.
    ///
    /// Used for state persistence in REPL sessions where symbol definitions
    /// must carry over across compilation passes.
    pub fn symbols_table(&self) -> &SymbolsTable {
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
                let symbol = self.symbols_table.define_symbol(&let_stmt.ident)?;
                let (scope, index) = (symbol.scope, symbol.index);
                match scope {
                    SymbolScope::Global => self.emit(Opcode::SetGlobal, &[index]),
                    SymbolScope::Local => self.emit(Opcode::SetLocal, &[index]),
                    SymbolScope::Builtin => unreachable!("cannot bind to a builtin scope"),
                };
                Ok(())
            }
            Statement::Return(ret_stmt) => {
                self.compile_expression(&ret_stmt.value)?;
                self.emit(Opcode::ReturnValue, &[]);
                Ok(())
            }
        }
    }

    /// Compiles a sequence of statements.
    fn compile_block_statement(&mut self, block: &BlockStatement) -> Result<()> {
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    /// Emits a constant-load instruction for a numeric literal.
    fn compile_numeric_constant(&mut self, obj: Object) -> Result<()> {
        let index = self.add_constant(obj)?;
        self.emit(Opcode::Constant, &[index]);
        Ok(())
    }

    /// Compiles an expression node into bytecode.
    fn compile_expression(&mut self, expr: &Expression) -> Result<()> {
        match expr {
            Expression::I8(lit) => self.compile_numeric_constant(Object::I8(lit.value)),
            Expression::I16(lit) => self.compile_numeric_constant(Object::I16(lit.value)),
            Expression::I32(lit) => self.compile_numeric_constant(Object::I32(lit.value)),
            Expression::I64(lit) => self.compile_numeric_constant(Object::I64(lit.value)),
            Expression::I128(lit) => self.compile_numeric_constant(Object::I128(lit.value)),
            Expression::Isize(lit) => self.compile_numeric_constant(Object::Isize(lit.value)),

            Expression::U8(lit) => self.compile_numeric_constant(Object::U8(lit.value)),
            Expression::U16(lit) => self.compile_numeric_constant(Object::U16(lit.value)),
            Expression::U32(lit) => self.compile_numeric_constant(Object::U32(lit.value)),
            Expression::U64(lit) => self.compile_numeric_constant(Object::U64(lit.value)),
            Expression::U128(lit) => self.compile_numeric_constant(Object::U128(lit.value)),
            Expression::Usize(lit) => self.compile_numeric_constant(Object::Usize(lit.value)),

            Expression::F32(lit) => self.compile_numeric_constant(Object::F32(f32::from(*lit))),
            Expression::F64(lit) => self.compile_numeric_constant(Object::F64(f64::from(*lit))),

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

                let cons_pos = self.current_instructions().len();
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

                let alt_pos = self.current_instructions().len();
                self.replace_operand(jump_pos, alt_pos)?;
                Ok(())
            }

            Expression::Identifier(name) => {
                let symbol = self
                    .symbols_table
                    .resolve_symbol(name)
                    .ok_or_else(|| CompileError::UndefinedVariable { name: name.clone() })?;
                let (scope, index) = (symbol.scope, symbol.index);
                match scope {
                    SymbolScope::Global => self.emit(Opcode::GetGlobal, &[index]),
                    SymbolScope::Local => self.emit(Opcode::GetLocal, &[index]),
                    SymbolScope::Builtin => self.emit(Opcode::GetBuiltin, &[index]),
                };
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

            Expression::Function(func) => {
                self.enter_scope();

                for param in &func.params {
                    self.symbols_table.define_symbol(param)?;
                }

                self.compile_block_statement(&func.body)?;

                if self.last_instruction_is(Opcode::Pop) {
                    self.replace_last_pop_with_return_value();
                }
                if !self.last_instruction_is(Opcode::ReturnValue) {
                    self.emit(Opcode::Return, &[]);
                }

                let num_locals = self.symbols_table.num_definitions();
                let instructions = self.leave_scope()?;

                let compiled_fn = Object::CompiledFunction(CompiledFunction {
                    instructions: instructions.into(),
                    num_locals,
                    num_parameters: func.params.len(),
                });
                let index = self.add_constant(compiled_fn)?;
                self.emit(Opcode::Constant, &[index]);
                Ok(())
            }

            Expression::Call(call) => {
                self.compile_expression(&call.function)?;
                for arg in &call.arguments {
                    self.compile_expression(arg)?;
                }
                self.emit(Opcode::Call, &[call.arguments.len()]);
                Ok(())
            }

            Expression::Macro(_) => Err(CompileError::UnsupportedExpression {
                expr_type: "macro literal".to_string(),
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

    /// Returns a reference to the current scope's instruction stream.
    fn current_instructions(&self) -> &Instructions {
        &self.scopes[self.scope_index].instructions
    }

    /// Enters a new compilation scope for a function body.
    ///
    /// Creates a fresh instruction stream and an enclosed symbol table
    /// that chains to the current one.
    fn enter_scope(&mut self) {
        self.scopes.push(CompilationScope::new());
        self.scope_index += 1;

        let outer = std::mem::take(&mut self.symbols_table);
        self.symbols_table = SymbolsTable::new_enclosed(outer);
    }

    /// Leaves the current compilation scope, returning its instructions.
    ///
    /// Restores the outer symbol table and pops the scope stack.
    ///
    /// # Errors
    ///
    /// Returns `CompileError::ScopeUnderflow` if the scope stack is empty
    /// or the enclosed symbol table has no outer table to restore.
    fn leave_scope(&mut self) -> Result<Instructions> {
        if self.scope_index == 0 {
            return Err(CompileError::ScopeUnderflow.into());
        }
        let scope = self.scopes.pop().ok_or(CompileError::ScopeUnderflow)?;
        self.scope_index -= 1;

        let current = std::mem::take(&mut self.symbols_table);
        self.symbols_table = current.take_outer().ok_or(CompileError::ScopeUnderflow)?;

        Ok(scope.instructions)
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

    /// Appends instruction bytes to the current scope's instruction stream.
    ///
    /// Returns the position where the instruction was inserted.
    fn add_instruction(&mut self, instruction: &[u8]) -> usize {
        let scope = &mut self.scopes[self.scope_index];
        let pos = scope.instructions.len();
        scope
            .instructions
            .extend(&Instructions::from(instruction.to_vec()));
        pos
    }

    /// Updates the last/previous instruction tracking in the current scope.
    fn set_last_instruction(&mut self, opcode: Opcode, position: usize) {
        let scope = &mut self.scopes[self.scope_index];
        scope.previous_instruction = scope.last_instruction;
        scope.last_instruction = Some(Instruction { opcode, position });
    }

    /// Returns `true` if the last emitted instruction matches the given opcode.
    fn last_instruction_is(&self, opcode: Opcode) -> bool {
        self.scopes[self.scope_index]
            .last_instruction
            .is_some_and(|last| last.opcode == opcode)
    }

    /// Removes the last `OpPop` instruction from the stream.
    ///
    /// This is used when compiling block expressions within conditionals:
    /// the block's expression statements emit `OpPop`, but conditionals
    /// need the value to remain on the stack.
    fn remove_last_pop(&mut self) {
        let scope = &mut self.scopes[self.scope_index];
        if let Some(last) = scope.last_instruction {
            scope.instructions.truncate(last.position);
            scope.last_instruction = scope.previous_instruction;
        }
    }

    /// Replaces the last `OpPop` with `OpReturnValue`.
    ///
    /// Used at the end of function bodies to convert the final expression
    /// statement's implicit pop into an explicit return.
    fn replace_last_pop_with_return_value(&mut self) {
        let scope = &mut self.scopes[self.scope_index];
        if let Some(last) = scope.last_instruction {
            let new_inst = encode(Opcode::ReturnValue, &[]);
            scope.instructions.replace_bytes(last.position, &new_inst);
            scope.last_instruction = Some(Instruction {
                opcode: Opcode::ReturnValue,
                position: last.position,
            });
        }
    }

    /// Replaces the operand of an instruction at the given position.
    ///
    /// Re-encodes the full instruction (opcode + new operand) and patches
    /// it into the instruction stream. Used for back-patching forward jumps.
    fn replace_operand(&mut self, op_pos: usize, operand: usize) -> Result<()> {
        let scope = &mut self.scopes[self.scope_index];
        let byte = scope.instructions.as_bytes()[op_pos];
        let op = Opcode::from_byte(byte).ok_or(CompileError::InvalidOpcode {
            opcode: byte,
            position: op_pos,
        })?;
        let new_inst = encode(op, &[operand]);
        scope.instructions.replace_bytes(op_pos, &new_inst);
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

    #[test]
    fn scopes() {
        let mut compiler = Compiler::new();
        assert_eq!(compiler.scope_index, 0);

        compiler.emit(Opcode::Mul, &[]);

        compiler.enter_scope();
        assert_eq!(compiler.scope_index, 1);

        compiler.emit(Opcode::Sub, &[]);
        assert_eq!(compiler.scopes[compiler.scope_index].instructions.len(), 1);
        assert_eq!(
            compiler.scopes[compiler.scope_index]
                .last_instruction
                .unwrap()
                .opcode,
            Opcode::Sub
        );

        let instructions = compiler.leave_scope().expect("should leave scope");
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
