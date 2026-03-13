use std::rc::Rc;

use maat_ast::{
    BlockStmt, EnumVariantKind, Expr, FieldAccessExpr, ImplBlock, MatchExpr, MethodCallExpr, Node,
    PathExpr, Pattern, Program, Stmt, StructLitExpr, TypeAnnotation, TypeExpr,
};
use maat_bytecode::{
    Bytecode, Instruction, Instructions, MAX_CONSTANT_POOL_SIZE, Opcode, TypeTag, encode,
};
use maat_errors::{CompileError, CompileErrorKind, Error, Result};
use maat_runtime::{BUILTINS, CompiledFunction, Object, TypeDef, VariantInfo};
use maat_span::{SourceMap, Span};

use crate::{Symbol, SymbolScope, SymbolsTable};

/// Tracks jump targets for break/continue within a loop.
///
/// Each loop pushes a context onto the compiler's loop stack. `break`
/// emits a forward jump whose position is recorded in `break_jumps` for
/// back-patching once the loop exit address is known. `continue` either
/// jumps directly to `continue_target` (when the address is known at
/// compile time, e.g. `loop` and `while`) or records the jump position
/// in `continue_jumps` for back-patching (e.g. `for` loops where
/// `continue` must jump to the increment section, whose address is only
/// known after body compilation).
#[derive(Debug, Clone)]
struct LoopContext {
    continue_target: Option<usize>,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

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
    source_map: SourceMap,
}

impl CompilationScope {
    fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            last_instruction: None,
            previous_instruction: None,
            source_map: SourceMap::new(),
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
    loop_contexts: Vec<LoopContext>,
    for_loop_counter: usize,
    type_registry: Vec<TypeDef>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// A deterministic dummy target to jump to.
    /// Ultimately replaced by the actual index downstream.
    const JUMP: usize = 9999;

    /// Creates a new compiler with empty instruction stream and constant pool.
    pub fn new() -> Self {
        let mut symbols_table = SymbolsTable::new();
        Self::register_builtins(&mut symbols_table);

        Self {
            constants: Vec::new(),
            symbols_table,
            scopes: vec![CompilationScope::new()],
            scope_index: 0,
            loop_contexts: Vec::new(),
            for_loop_counter: 0,
            type_registry: Self::builtin_type_registry(),
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
            loop_contexts: Vec::new(),
            for_loop_counter: 0,
            type_registry: Self::builtin_type_registry(),
        }
    }

    /// Returns a mutable reference to the compiler's symbols table.
    ///
    /// Used by the module pipeline to define imported symbols before
    /// compilation, so that cross-module references resolve correctly.
    pub fn symbols_table_mut(&mut self) -> &mut SymbolsTable {
        &mut self.symbols_table
    }

    /// Returns a mutable reference to the compiler's type registry.
    ///
    /// Used by the module pipeline to register imported type definitions
    /// (structs, enums) so that construction and field access work
    /// across module boundaries.
    pub fn type_registry_mut(&mut self) -> &mut Vec<TypeDef> {
        &mut self.type_registry
    }

    /// Registers all built-in functions in the given symbols table.
    fn register_builtins(table: &mut SymbolsTable) {
        for (i, (name, _)) in BUILTINS.iter().enumerate() {
            table.define_builtin(i, name);
        }
    }

    /// Returns the type registry pre-populated with built-in enum types.
    fn builtin_type_registry() -> Vec<TypeDef> {
        vec![
            TypeDef::Enum {
                name: "Option".to_string(),
                variants: vec![
                    VariantInfo {
                        name: "Some".to_string(),
                        field_count: 1,
                    },
                    VariantInfo {
                        name: "None".to_string(),
                        field_count: 0,
                    },
                ],
            },
            TypeDef::Enum {
                name: "Result".to_string(),
                variants: vec![
                    VariantInfo {
                        name: "Ok".to_string(),
                        field_count: 1,
                    },
                    VariantInfo {
                        name: "Err".to_string(),
                        field_count: 1,
                    },
                ],
            },
        ]
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
        let scope = self
            .scopes
            .pop()
            .ok_or(CompileError::new(CompileErrorKind::ScopeUnderflow))?;
        Ok(Bytecode {
            instructions: scope.instructions,
            constants: self.constants,
            source_map: scope.source_map,
            type_registry: self.type_registry,
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
            Node::Stmt(stmt) => self.compile_statement(stmt),
            Node::Expr(expr) => self.compile_expression(expr),
        }
    }

    /// Compiles a program node (list of statements).
    pub fn compile_program(&mut self, program: &Program) -> Result<()> {
        for stmt in &program.statements {
            if let Stmt::FuncDef(fn_item) = stmt {
                let span = fn_item.span;
                match self.symbols_table.define_symbol(&fn_item.name, false) {
                    Ok(_) => {}
                    Err(e) => return Err(self.attach_span(e, span)),
                }
            }
        }

        for stmt in &program.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    /// Compiles a statement node.
    fn compile_statement(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                self.compile_expression(&expr_stmt.value)?;
                self.emit(Opcode::Pop, &[], expr_stmt.span);
                Ok(())
            }
            Stmt::Block(block) => self.compile_block_statement(block),
            Stmt::Let(let_stmt) => {
                let span = let_stmt.span;
                if let Expr::Lambda(lambda) = &let_stmt.value {
                    self.compile_fn_body(
                        Some(&let_stmt.ident),
                        lambda.param_names(),
                        lambda.params.len(),
                        &lambda.body,
                        lambda.span,
                    )?;
                } else {
                    self.compile_expression(&let_stmt.value)?;
                }
                let symbol = match self
                    .symbols_table
                    .define_symbol(&let_stmt.ident, let_stmt.mutable)
                {
                    Ok(s) => s,
                    Err(e) => return Err(self.attach_span(e, span)),
                };
                let (scope, index) = (symbol.scope, symbol.index);
                match scope {
                    SymbolScope::Global => self.emit(Opcode::SetGlobal, &[index], span),
                    SymbolScope::Local => self.emit(Opcode::SetLocal, &[index], span),
                    SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {
                        unreachable!("define_symbol never produces this scope")
                    }
                };
                Ok(())
            }
            Stmt::ReAssign(assign_stmt) => {
                let span = assign_stmt.span;
                let symbol = self
                    .symbols_table
                    .resolve_symbol(&assign_stmt.ident)
                    .ok_or_else(|| {
                        CompileErrorKind::UndefinedVariable {
                            name: assign_stmt.ident.clone(),
                        }
                        .at(span)
                    })?;
                if !symbol.mutable {
                    return Err(CompileErrorKind::ImmutableAssignment {
                        name: assign_stmt.ident.clone(),
                    }
                    .at(span)
                    .into());
                }
                self.compile_expression(&assign_stmt.value)?;
                match symbol.scope {
                    SymbolScope::Global => self.emit(Opcode::SetGlobal, &[symbol.index], span),
                    SymbolScope::Local => self.emit(Opcode::SetLocal, &[symbol.index], span),
                    SymbolScope::Free => self.emit(Opcode::SetLocal, &[symbol.index], span),
                    SymbolScope::Builtin | SymbolScope::Function => {
                        return Err(CompileErrorKind::ImmutableAssignment {
                            name: assign_stmt.ident.clone(),
                        }
                        .at(span)
                        .into());
                    }
                };
                Ok(())
            }

            Stmt::Return(ret_stmt) => {
                self.compile_expression(&ret_stmt.value)?;
                self.emit(Opcode::ReturnValue, &[], ret_stmt.span);
                Ok(())
            }

            Stmt::FuncDef(fn_item) => {
                let span = fn_item.span;
                self.compile_fn_body(
                    Some(&fn_item.name),
                    fn_item.param_names(),
                    fn_item.params.len(),
                    &fn_item.body,
                    span,
                )?;
                let symbol = match self.symbols_table.define_symbol(&fn_item.name, false) {
                    Ok(s) => s,
                    Err(e) => return Err(self.attach_span(e, span)),
                };
                let (scope, index) = (symbol.scope, symbol.index);
                match scope {
                    SymbolScope::Global => self.emit(Opcode::SetGlobal, &[index], span),
                    SymbolScope::Local => self.emit(Opcode::SetLocal, &[index], span),
                    SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {
                        unreachable!("define_symbol never produces this scope")
                    }
                };
                Ok(())
            }

            Stmt::Loop(loop_stmt) => {
                let start = self.current_instructions().len();

                self.loop_contexts.push(LoopContext {
                    continue_target: Some(start),
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                });

                self.compile_block_statement(&loop_stmt.body)?;

                self.emit(Opcode::Jump, &[start], loop_stmt.span);

                let exit = self.current_instructions().len();
                let ctx = self
                    .loop_contexts
                    .pop()
                    .expect("loop context was just pushed");
                for jump_pos in ctx.break_jumps {
                    self.replace_operand(jump_pos, exit)?;
                }

                Ok(())
            }

            Stmt::While(while_stmt) => {
                let start = self.current_instructions().len();

                self.loop_contexts.push(LoopContext {
                    continue_target: Some(start),
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                });

                self.compile_expression(&while_stmt.condition)?;
                let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], while_stmt.span);

                self.compile_block_statement(&while_stmt.body)?;

                self.emit(Opcode::Jump, &[start], while_stmt.span);

                let loop_exit = self.current_instructions().len();
                self.replace_operand(exit_jump, loop_exit)?;

                let ctx = self
                    .loop_contexts
                    .pop()
                    .expect("loop context was just pushed");
                for jump_pos in ctx.break_jumps {
                    self.replace_operand(jump_pos, loop_exit)?;
                }

                Ok(())
            }

            Stmt::StructDecl(decl) => {
                self.type_registry.push(TypeDef::Struct {
                    name: decl.name.clone(),
                    field_names: decl.fields.iter().map(|f| f.name.clone()).collect(),
                });
                Ok(())
            }
            Stmt::EnumDecl(decl) => {
                self.type_registry.push(TypeDef::Enum {
                    name: decl.name.clone(),
                    variants: decl
                        .variants
                        .iter()
                        .map(|v| VariantInfo {
                            name: v.name.clone(),
                            field_count: match &v.kind {
                                EnumVariantKind::Unit => 0,
                                EnumVariantKind::Tuple(fields) => fields.len() as u8,
                                EnumVariantKind::Struct(fields) => fields.len() as u8,
                            },
                        })
                        .collect(),
                });
                Ok(())
            }
            Stmt::TraitDecl(_) => Ok(()),
            Stmt::ImplBlock(impl_block) => self.compile_impl_block(impl_block),

            // Module declarations and import statements are resolved by the
            // module orchestrator before per-module compilation. No-op here.
            Stmt::Use(_) | Stmt::Mod(_) => Ok(()),

            Stmt::For(for_stmt) => {
                // Desugar: evaluate iterable, bind a hidden counter, iterate via index.
                //
                //   let __iter = <iterable>;
                //   let __len  = len(__iter);
                //   let __i    = 0;
                //   loop_start:
                //       if !(__i < __len) goto loop_exit
                //       let <ident> = __iter[__i];
                //       <body>
                //   continue_target:
                //       __i = __i + 1
                //       goto loop_start
                //   loop_exit:
                //       null

                let span = for_stmt.span;
                let id = self.for_loop_counter;
                self.for_loop_counter += 1;

                let iter_name = format!("__iter_{id}");
                let len_name = format!("__len_{id}");
                let i_name = format!("__i_{id}");

                // __iter_N
                self.compile_expression(&for_stmt.iterable)?;
                let iter_sym = self.define_and_set(&iter_name, false, span)?;

                // __len_N = Array::len(__iter_N)
                let len_builtin = self
                    .symbols_table
                    .resolve_symbol("Array::len")
                    .ok_or_else(|| {
                        CompileErrorKind::UndefinedVariable {
                            name: "Array::len".to_string(),
                        }
                        .at(span)
                    })?
                    .clone();
                self.load_symbol(&len_builtin, span);
                self.load_symbol(&iter_sym, span);
                self.emit(Opcode::Call, &[1], span);
                let len_sym = self.define_and_set(&len_name, false, span)?;

                // __i_N = 0
                let zero_idx = self.add_constant(Object::I64(0))?;
                self.emit(Opcode::Constant, &[zero_idx], span);
                let i_sym = self.define_and_set(&i_name, false, span)?;

                // loop_start: condition check (__i < __len)
                let loop_start = self.current_instructions().len();

                // continue_target is None — continue jumps are deferred and
                // patched to point to the increment section after body compilation.
                self.loop_contexts.push(LoopContext {
                    continue_target: None,
                    break_jumps: Vec::new(),
                    continue_jumps: Vec::new(),
                });

                self.load_symbol(&i_sym, span);
                self.load_symbol(&len_sym, span);
                self.emit(Opcode::LessThan, &[], span);

                let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

                // let <ident> = __iter[__i]
                self.load_symbol(&iter_sym, span);
                self.load_symbol(&i_sym, span);
                self.emit(Opcode::Index, &[], span);
                let elem_sym = self.define_and_set(&for_stmt.ident, false, span)?;
                let _ = elem_sym; // used only for side effect of defining the binding

                // body
                self.compile_block_statement(&for_stmt.body)?;

                // continue_target: __i = __i + 1
                let continue_target = self.current_instructions().len();
                self.load_symbol(&i_sym, span);
                let one_idx = self.add_constant(Object::I64(1))?;
                self.emit(Opcode::Constant, &[one_idx], span);
                self.emit(Opcode::Add, &[], span);
                self.emit_set_symbol(&i_sym, span);

                self.emit(Opcode::Jump, &[loop_start], span);

                let loop_exit = self.current_instructions().len();
                self.replace_operand(exit_jump, loop_exit)?;

                let ctx = self
                    .loop_contexts
                    .pop()
                    .expect("loop context was just pushed");
                for jump_pos in ctx.break_jumps {
                    self.replace_operand(jump_pos, loop_exit)?;
                }
                for jump_pos in ctx.continue_jumps {
                    self.replace_operand(jump_pos, continue_target)?;
                }

                Ok(())
            }
        }
    }

    /// Compiles a sequence of statements within a lexical block scope.
    ///
    /// Variables defined inside the block are scoped to it and become
    /// invisible after the block exits, matching Rust's lexical scoping.
    /// Used for `if`/`else` branches and standalone blocks where variable
    /// isolation is desired.
    fn compile_block_statement(&mut self, block: &BlockStmt) -> Result<()> {
        self.symbols_table.push_block_scope();
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        self.symbols_table.pop_block_scope();
        Ok(())
    }

    /// Emits a constant-load instruction for a numeric literal.
    fn compile_numeric_constant(&mut self, obj: Object, span: Span) -> Result<()> {
        let index = self.add_constant(obj)?;
        self.emit(Opcode::Constant, &[index], span);
        Ok(())
    }

    /// Compiles an expression node into bytecode.
    fn compile_expression(&mut self, expr: &Expr) -> Result<()> {
        let span = expr.span();
        match expr {
            Expr::I8(lit) => self.compile_numeric_constant(Object::I8(lit.value), span),
            Expr::I16(lit) => self.compile_numeric_constant(Object::I16(lit.value), span),
            Expr::I32(lit) => self.compile_numeric_constant(Object::I32(lit.value), span),
            Expr::I64(lit) => self.compile_numeric_constant(Object::I64(lit.value), span),
            Expr::I128(lit) => self.compile_numeric_constant(Object::I128(lit.value), span),
            Expr::Isize(lit) => self.compile_numeric_constant(Object::Isize(lit.value), span),

            Expr::U8(lit) => self.compile_numeric_constant(Object::U8(lit.value), span),
            Expr::U16(lit) => self.compile_numeric_constant(Object::U16(lit.value), span),
            Expr::U32(lit) => self.compile_numeric_constant(Object::U32(lit.value), span),
            Expr::U64(lit) => self.compile_numeric_constant(Object::U64(lit.value), span),
            Expr::U128(lit) => self.compile_numeric_constant(Object::U128(lit.value), span),
            Expr::Usize(lit) => self.compile_numeric_constant(Object::Usize(lit.value), span),

            Expr::Bool(b) => {
                let opcode = if b.value { Opcode::True } else { Opcode::False };
                self.emit(opcode, &[], span);
                Ok(())
            }

            Expr::Prefix(prefix_expr) => {
                self.compile_expression(&prefix_expr.operand)?;

                let opcode = match prefix_expr.operator.as_str() {
                    "!" => Opcode::Bang,
                    "-" => Opcode::Minus,
                    op => {
                        return Err(CompileErrorKind::UnsupportedOperator {
                            operator: op.to_string(),
                            context: "prefix expression".to_string(),
                        }
                        .at(span)
                        .into());
                    }
                };

                self.emit(opcode, &[], span);
                Ok(())
            }

            Expr::Infix(infix_expr) if infix_expr.operator == "&&" => {
                self.compile_expression(&infix_expr.lhs)?;
                let cond_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);
                self.compile_expression(&infix_expr.rhs)?;
                let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);

                let false_pos = self.current_instructions().len();
                self.replace_operand(cond_jump, false_pos)?;
                self.emit(Opcode::False, &[], span);
                let end_pos = self.current_instructions().len();

                self.replace_operand(end_jump, end_pos)?;
                Ok(())
            }

            Expr::Infix(infix_expr) if infix_expr.operator == "||" => {
                self.compile_expression(&infix_expr.lhs)?;
                let cond_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);
                self.emit(Opcode::True, &[], span);
                let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);

                let rhs_pos = self.current_instructions().len();
                self.replace_operand(cond_jump, rhs_pos)?;
                self.compile_expression(&infix_expr.rhs)?;
                let end_pos = self.current_instructions().len();

                self.replace_operand(end_jump, end_pos)?;
                Ok(())
            }

            Expr::Infix(infix_expr) => {
                self.compile_expression(&infix_expr.lhs)?;
                self.compile_expression(&infix_expr.rhs)?;

                match infix_expr.operator.as_str() {
                    ">=" => {
                        self.emit(Opcode::LessThan, &[], span);
                        self.emit(Opcode::Bang, &[], span);
                    }
                    "<=" => {
                        self.emit(Opcode::GreaterThan, &[], span);
                        self.emit(Opcode::Bang, &[], span);
                    }
                    op => {
                        let opcode = match op {
                            "+" => Opcode::Add,
                            "-" => Opcode::Sub,
                            "*" => Opcode::Mul,
                            "/" => Opcode::Div,
                            "%" => Opcode::Mod,
                            ">" => Opcode::GreaterThan,
                            "<" => Opcode::LessThan,
                            "==" => Opcode::Equal,
                            "!=" => Opcode::NotEqual,
                            "&" => Opcode::BitAnd,
                            "|" => Opcode::BitOr,
                            "^" => Opcode::BitXor,
                            "<<" => Opcode::Shl,
                            ">>" => Opcode::Shr,
                            _ => {
                                return Err(CompileErrorKind::UnsupportedOperator {
                                    operator: op.to_string(),
                                    context: "infix expression".to_string(),
                                }
                                .at(span)
                                .into());
                            }
                        };
                        self.emit(opcode, &[], span);
                    }
                }
                Ok(())
            }

            Expr::Cond(cond) => {
                self.compile_expression(&cond.condition)?;

                let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP], cond.span);

                self.compile_block_statement(&cond.consequence)?;
                if self.last_instruction_is(Opcode::Pop) {
                    self.remove_last_pop();
                }

                let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], cond.span);

                let cons_pos = self.current_instructions().len();
                self.replace_operand(cond_jump_pos, cons_pos)?;

                match &cond.alternative {
                    None => {
                        self.emit(Opcode::Null, &[], cond.span);
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

            Expr::Ident(ident) => {
                let symbol = self
                    .symbols_table
                    .resolve_symbol(&ident.value)
                    .ok_or_else(|| {
                        CompileErrorKind::UndefinedVariable {
                            name: ident.value.clone(),
                        }
                        .at(ident.span)
                    })?;
                self.load_symbol(&symbol, span);
                Ok(())
            }

            Expr::Str(s) => {
                let constant = Object::Str(maat_ast::unescape_string(&s.value));
                let index = self.add_constant(constant)?;
                self.emit(Opcode::Constant, &[index], span);
                Ok(())
            }

            Expr::Array(array) => {
                for element in &array.elements {
                    self.compile_expression(element)?;
                }
                self.emit(Opcode::Array, &[array.elements.len()], span);
                Ok(())
            }

            Expr::Map(hash) => {
                for (key, value) in &hash.pairs {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.emit(Opcode::Hash, &[hash.pairs.len() * 2], span);
                Ok(())
            }

            Expr::Index(index_expr) => {
                self.compile_expression(&index_expr.expr)?;
                self.compile_expression(&index_expr.index)?;
                self.emit(Opcode::Index, &[], span);
                Ok(())
            }

            Expr::Lambda(lambda) => {
                self.compile_fn_body(
                    None,
                    lambda.param_names(),
                    lambda.params.len(),
                    &lambda.body,
                    lambda.span,
                )?;
                Ok(())
            }

            Expr::Call(call) => {
                self.compile_expression(&call.function)?;
                for arg in &call.arguments {
                    self.compile_expression(arg)?;
                }
                self.emit(Opcode::Call, &[call.arguments.len()], span);
                Ok(())
            }

            Expr::Cast(cast) => {
                self.compile_expression(&cast.expr)?;
                let tag = Self::type_annotation_to_tag(cast.target);
                self.emit(Opcode::Convert, &[tag.to_byte() as usize], span);
                Ok(())
            }

            Expr::Break(break_expr) => {
                if self.loop_contexts.is_empty() {
                    return Err(CompileErrorKind::BreakOutsideLoop
                        .at(break_expr.span)
                        .into());
                }

                match &break_expr.value {
                    Some(val) => self.compile_expression(val)?,
                    None => {
                        self.emit(Opcode::Null, &[], break_expr.span);
                    }
                }

                let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], break_expr.span);
                self.loop_contexts
                    .last_mut()
                    .expect("loop context was just verified")
                    .break_jumps
                    .push(jump_pos);

                Ok(())
            }

            Expr::Continue(cont_expr) => {
                let ctx = self
                    .loop_contexts
                    .last()
                    .ok_or_else(|| CompileErrorKind::ContinueOutsideLoop.at(cont_expr.span))?;

                match ctx.continue_target {
                    Some(target) => {
                        self.emit(Opcode::Jump, &[target], cont_expr.span);
                    }
                    None => {
                        let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], cont_expr.span);
                        self.loop_contexts
                            .last_mut()
                            .expect("loop context was just verified")
                            .continue_jumps
                            .push(jump_pos);
                    }
                }

                Ok(())
            }

            Expr::Macro(_) => Err(CompileErrorKind::UnsupportedExpr {
                expr_type: "macro literal".to_string(),
            }
            .at(span)
            .into()),

            Expr::StructLit(struct_lit) => self.compile_struct_literal(struct_lit),

            Expr::PathExpr(path_expr) => self.compile_path_expression(path_expr),

            Expr::Match(match_expr) => self.compile_match(match_expr),

            Expr::FieldAccess(field_access) => self.compile_field_access(field_access),

            Expr::MethodCall(method_call) => self.compile_method_call(method_call),
        }
    }

    /// Compiles a struct literal expression (e.g., `Point { x: 1, y: 2 }`).
    ///
    /// Looks up the struct in the type registry to determine field ordering,
    /// compiles each field value in declaration order, and emits a `Construct`
    /// instruction.
    fn compile_struct_literal(&mut self, lit: &StructLitExpr) -> Result<()> {
        let span = lit.span;
        let (registry_index, field_names) = self
            .type_registry
            .iter()
            .enumerate()
            .find_map(|(i, td)| match td {
                TypeDef::Struct { name, field_names } if name == &lit.name => {
                    Some((i, field_names.clone()))
                }
                _ => None,
            })
            .ok_or_else(|| {
                CompileErrorKind::UndefinedVariable {
                    name: lit.name.clone(),
                }
                .at(span)
            })?;

        for field_name in &field_names {
            let value_expr = lit
                .fields
                .iter()
                .find(|(name, _)| name == field_name)
                .map(|(_, expr)| expr)
                .ok_or_else(|| {
                    CompileErrorKind::UndefinedVariable {
                        name: format!("missing field `{}` in struct `{}`", field_name, lit.name),
                    }
                    .at(span)
                })?;
            self.compile_expression(value_expr)?;
        }

        let type_index = registry_index << 8;
        self.emit(Opcode::Construct, &[type_index, field_names.len()], span);
        Ok(())
    }

    /// Compiles a path expression (e.g., `Option::None`, `Color::Red`).
    ///
    /// For a two-segment path `Enum::Variant`, resolves the enum and variant
    /// in the type registry. Unit variants emit a `Construct` with zero fields.
    /// For non-unit variants used as constructors (followed by call), this
    /// pushes a closure that wraps the `Construct` instruction.
    fn compile_path_expression(&mut self, path: &PathExpr) -> Result<()> {
        let span = path.span;

        if path.segments.len() == 2 {
            let type_name = &path.segments[0];
            let variant_name = &path.segments[1];

            // Check if it refers to an enum variant.
            if let Some((registry_index, variant_tag, field_count)) =
                self.resolve_enum_variant(type_name, variant_name)
            {
                if field_count == 0 {
                    // Unit variant: construct immediately.
                    let type_index = (registry_index << 8) | (variant_tag & 0xFF);
                    self.emit(Opcode::Construct, &[type_index, 0], span);
                } else {
                    // Tuple/struct variant: emit a closure that takes `field_count` params
                    // and constructs the variant.
                    self.enter_scope();
                    let mut param_names = Vec::with_capacity(field_count);
                    for i in 0..field_count {
                        let name = format!("__field_{i}");
                        if let Err(e) = self.symbols_table.define_symbol(&name, false) {
                            return Err(self.attach_span(e, span));
                        }
                        param_names.push(name);
                    }

                    // Load each parameter onto the stack.
                    for name in &param_names {
                        let sym = self.symbols_table.resolve_symbol(name).unwrap();
                        self.load_symbol(&sym, span);
                    }

                    let type_index = (registry_index << 8) | (variant_tag & 0xFF);
                    self.emit(Opcode::Construct, &[type_index, field_count], span);
                    self.emit(Opcode::ReturnValue, &[], span);

                    let num_locals = self.symbols_table.max_definitions();
                    let (instructions, inner_source_map) = self.leave_scope()?;
                    let compiled_fn = Object::CompiledFunction(CompiledFunction {
                        instructions: Rc::from(instructions.as_bytes()),
                        num_locals,
                        num_parameters: field_count,
                        source_map: inner_source_map,
                    });
                    let index = self.add_constant(compiled_fn)?;
                    self.emit(Opcode::Closure, &[index, 0], span);
                }
                return Ok(());
            }

            let qualified_name = format!("{type_name}::{variant_name}");
            if let Some(symbol) = self.symbols_table.resolve_symbol(&qualified_name) {
                self.load_symbol(&symbol, span);
                return Ok(());
            }
        }

        let full_name = path.segments.join("::");
        let symbol = self
            .symbols_table
            .resolve_symbol(&full_name)
            .ok_or_else(|| CompileErrorKind::UndefinedVariable { name: full_name }.at(span))?;
        self.load_symbol(&symbol, span);
        Ok(())
    }

    /// Resolves an enum variant by type name and variant name.
    ///
    /// Returns `(registry_index, variant_tag, field_count)` if found.
    fn resolve_enum_variant(
        &self,
        type_name: &str,
        variant_name: &str,
    ) -> Option<(usize, usize, usize)> {
        self.type_registry
            .iter()
            .enumerate()
            .find_map(|(i, td)| match td {
                TypeDef::Enum { name, variants } if name == type_name => variants
                    .iter()
                    .enumerate()
                    .find(|(_, v)| v.name == variant_name)
                    .map(|(tag, v)| (i, tag, v.field_count as usize)),
                _ => None,
            })
    }

    /// Compiles a `match` expression as a chain of `MatchTag` / conditional
    /// jump instructions.
    ///
    /// The scrutinee is compiled once and left on the stack. Each arm tests
    /// the variant tag (for enums) or pattern, emitting the arm body on match.
    /// At the end all forward jumps are patched to the exit point.
    fn compile_match(&mut self, match_expr: &MatchExpr) -> Result<()> {
        let span = match_expr.span;

        self.compile_expression(&match_expr.scrutinee)?;

        let mut end_jumps = Vec::new();

        for arm in &match_expr.arms {
            match &arm.pattern {
                Pattern::TupleStruct { path, fields, .. } => {
                    if let Some((_, variant_tag, _)) = self.find_variant_in_registry(path) {
                        let match_tag_pos =
                            self.emit(Opcode::MatchTag, &[variant_tag, Self::JUMP], span);

                        self.bind_variant_fields(fields, span)?;

                        self.compile_expression(&arm.body)?;
                        if self.last_instruction_is(Opcode::Pop) {
                            self.remove_last_pop();
                        }

                        let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                        end_jumps.push(end_jump);

                        let next_arm = self.current_instructions().len();
                        self.replace_match_tag_target(match_tag_pos, next_arm)?;
                    }
                }
                Pattern::Ident(name, _)
                    if name != "_" && self.find_variant_in_registry(name).is_some() =>
                {
                    let (_, variant_tag, _) = self.find_variant_in_registry(name).unwrap();
                    let match_tag_pos =
                        self.emit(Opcode::MatchTag, &[variant_tag, Self::JUMP], span);

                    self.emit(Opcode::Pop, &[], span);

                    self.compile_expression(&arm.body)?;
                    if self.last_instruction_is(Opcode::Pop) {
                        self.remove_last_pop();
                    }

                    let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                    end_jumps.push(end_jump);

                    let next_arm = self.current_instructions().len();
                    self.replace_match_tag_target(match_tag_pos, next_arm)?;
                }
                Pattern::Ident(name, _) if name != "_" => {
                    self.define_and_set(name, false, span)?;
                    self.compile_expression(&arm.body)?;
                    if self.last_instruction_is(Opcode::Pop) {
                        self.remove_last_pop();
                    }
                    let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                    end_jumps.push(end_jump);
                }
                Pattern::Wildcard(_) | Pattern::Ident(_, _) => {
                    self.emit(Opcode::Pop, &[], span);
                    self.compile_expression(&arm.body)?;
                    if self.last_instruction_is(Opcode::Pop) {
                        self.remove_last_pop();
                    }
                    let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                    end_jumps.push(end_jump);
                }
                Pattern::Literal(lit_expr) => {
                    self.compile_expression(lit_expr)?;
                    self.emit(Opcode::Equal, &[], span);
                    let cond_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

                    self.compile_expression(&arm.body)?;
                    if self.last_instruction_is(Opcode::Pop) {
                        self.remove_last_pop();
                    }
                    let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                    end_jumps.push(end_jump);

                    let next_arm = self.current_instructions().len();
                    self.replace_operand(cond_jump, next_arm)?;
                }
                _ => {
                    self.emit(Opcode::Pop, &[], span);
                    self.emit(Opcode::Null, &[], span);
                    let end_jump = self.emit(Opcode::Jump, &[Self::JUMP], span);
                    end_jumps.push(end_jump);
                }
            }
        }

        self.emit(Opcode::Null, &[], span);

        let exit = self.current_instructions().len();
        for jump_pos in end_jumps {
            self.replace_operand(jump_pos, exit)?;
        }

        Ok(())
    }

    /// Finds an enum variant across the entire type registry by variant name.
    fn find_variant_in_registry(&self, variant_name: &str) -> Option<(usize, usize, usize)> {
        self.type_registry
            .iter()
            .enumerate()
            .find_map(|(i, td)| match td {
                TypeDef::Enum { variants, .. } => variants
                    .iter()
                    .enumerate()
                    .find(|(_, v)| v.name == variant_name)
                    .map(|(tag, v)| (i, tag, v.field_count as usize)),
                _ => None,
            })
    }

    /// Binds variant payload fields to local variables via `GetField`.
    fn bind_variant_fields(&mut self, fields: &[Pattern], span: Span) -> Result<()> {
        for (i, field) in fields.iter().enumerate() {
            if let Pattern::Ident(name, _) = field {
                // For enum variants with payloads, the scrutinee is on top of stack.
                if i == 0 {
                    let hidden = format!("__match_scrutinee_{}", self.for_loop_counter);
                    self.for_loop_counter += 1;
                    let hidden_sym = self.define_and_set(&hidden, false, span)?;
                    self.load_symbol(&hidden_sym, span);
                    self.emit(Opcode::GetField, &[i], span);
                    self.define_and_set(name, false, span)?;
                    continue;
                }

                let hidden = format!("__match_scrutinee_{}", self.for_loop_counter - 1);
                let hidden_sym = self.symbols_table.resolve_symbol(&hidden).ok_or_else(|| {
                    CompileErrorKind::UndefinedVariable {
                        name: hidden.clone(),
                    }
                    .at(span)
                })?;
                self.load_symbol(&hidden_sym, span);
                self.emit(Opcode::GetField, &[i], span);
                self.define_and_set(name, false, span)?;
            }
        }
        Ok(())
    }

    /// Replaces the jump-on-mismatch target of a `MatchTag` instruction.
    ///
    /// `MatchTag` has operands `[u16 variant_tag, u16 jump_target]`. The jump
    /// target is at offset `op_pos + 3` (1 opcode + 2 tag bytes).
    fn replace_match_tag_target(&mut self, op_pos: usize, target: usize) -> Result<()> {
        let scope = &mut self.scopes[self.scope_index];
        let target_bytes = (target as u16).to_be_bytes();
        scope.instructions.replace_bytes(op_pos + 3, &target_bytes);
        Ok(())
    }

    /// Compiles a field access expression (e.g., `point.x`).
    ///
    /// Resolves the field index from the type registry and emits a `GetField`
    /// instruction.
    fn compile_field_access(&mut self, fa: &FieldAccessExpr) -> Result<()> {
        let span = fa.span;
        self.compile_expression(&fa.object)?;

        let field_index = self
            .type_registry
            .iter()
            .find_map(|td| match td {
                TypeDef::Struct { field_names, .. } => {
                    field_names.iter().position(|f| f == &fa.field)
                }
                _ => None,
            })
            .ok_or_else(|| {
                CompileErrorKind::UndefinedVariable {
                    name: format!("unknown field `{}`", fa.field),
                }
                .at(span)
            })?;

        self.emit(Opcode::GetField, &[field_index], span);
        Ok(())
    }

    /// Compiles a method call expression (e.g., `point.distance(other)`).
    ///
    /// Resolves the method as a qualified function name (`Type::method`),
    /// pushes the receiver as the first argument, and emits a regular `Call`.
    fn compile_method_call(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;

        let qualified_name = self.resolve_method_name(mc).ok_or_else(|| {
            CompileErrorKind::UndefinedVariable {
                name: format!("unknown method `{}`", mc.method),
            }
            .at(span)
        })?;

        let symbol = self.symbols_table.resolve_symbol(&qualified_name).unwrap();
        self.load_symbol(&symbol, span);

        self.compile_expression(&mc.object)?;

        for arg in &mc.arguments {
            self.compile_expression(arg)?;
        }

        let total_args = 1 + mc.arguments.len();
        self.emit(Opcode::Call, &[total_args], span);
        Ok(())
    }

    /// Built-in type prefixes for method dispatch fallback.
    ///
    /// Used only when `receiver` is absent (e.g. REPL, tests that
    /// skip type checking). With type-directed dispatch these are never consulted.
    const BUILTIN_METHOD_PREFIXES: &[&str] = &["Array", "str", "Set"];

    /// Resolves the fully-qualified builtin name for a method call.
    ///
    /// Uses the type-checker-annotated `receiver` for direct dispatch
    /// when available. Falls back to a linear search through user-defined types
    /// and built-in type prefixes for unannotated ASTs (e.g. from the REPL or
    /// test paths that bypass type checking).
    fn resolve_method_name(&mut self, mc: &MethodCallExpr) -> Option<String> {
        if let Some(ref receiver) = mc.receiver {
            let candidate = format!("{receiver}::{}", mc.method);
            if self.symbols_table.resolve_symbol(&candidate).is_some() {
                return Some(candidate);
            }
        }

        self.type_registry
            .iter()
            .map(|td| match td {
                TypeDef::Struct { name, .. } | TypeDef::Enum { name, .. } => name.as_str(),
            })
            .chain(Self::BUILTIN_METHOD_PREFIXES.iter().copied())
            .find_map(|type_name| {
                let candidate = format!("{type_name}::{}", mc.method);
                self.symbols_table
                    .resolve_symbol(&candidate)
                    .map(|_| candidate)
            })
    }

    /// Maps a source-level type annotation to a bytecode type tag.
    fn type_annotation_to_tag(t: TypeAnnotation) -> TypeTag {
        match t {
            TypeAnnotation::I8 => TypeTag::I8,
            TypeAnnotation::I16 => TypeTag::I16,
            TypeAnnotation::I32 => TypeTag::I32,
            TypeAnnotation::I64 => TypeTag::I64,
            TypeAnnotation::I128 => TypeTag::I128,
            TypeAnnotation::Isize => TypeTag::Isize,
            TypeAnnotation::U8 => TypeTag::U8,
            TypeAnnotation::U16 => TypeTag::U16,
            TypeAnnotation::U32 => TypeTag::U32,
            TypeAnnotation::U64 => TypeTag::U64,
            TypeAnnotation::U128 => TypeTag::U128,
            TypeAnnotation::Usize => TypeTag::Usize,
        }
    }

    /// Attaches a span to a compile error that lacks one.
    fn attach_span(&self, err: Error, span: Span) -> Error {
        match err {
            Error::Compile(ce) if ce.span.is_none() => CompileError {
                kind: ce.kind,
                span: Some(span),
            }
            .into(),
            other => other,
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
            return Err(CompileError::new(CompileErrorKind::ConstantPoolOverflow {
                max: MAX_CONSTANT_POOL_SIZE,
                attempted: index,
            })
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
    /// Compiles a function body (shared by `Stmt::FuncDef` and `Expr::Lambda`).
    ///
    /// Enters a new scope, optionally defines a function name for recursive
    /// self-reference, compiles parameters and body, and emits the closure
    /// instruction.
    fn compile_fn_body<'a>(
        &mut self,
        name: Option<&str>,
        param_names: impl Iterator<Item = &'a str>,
        num_params: usize,
        body: &BlockStmt,
        span: Span,
    ) -> Result<()> {
        self.enter_scope();

        if let Some(name) = name {
            self.symbols_table.define_function_name(name);
        }

        for param in param_names {
            if let Err(e) = self.symbols_table.define_symbol(param, false) {
                return Err(self.attach_span(e, span));
            }
        }

        self.compile_block_statement(body)?;

        if self.last_instruction_is(Opcode::Pop) {
            self.replace_last_pop_with_return_value();
        }
        if !self.last_instruction_is(Opcode::ReturnValue) {
            self.emit(Opcode::Return, &[], span);
        }

        let free_vars = self.symbols_table.free_vars().to_vec();
        let num_free = free_vars.len();
        let num_locals = self.symbols_table.max_definitions();
        let (instructions, inner_source_map) = self.leave_scope()?;

        for sym in &free_vars {
            self.load_symbol(sym, span);
        }

        let compiled_fn = Object::CompiledFunction(CompiledFunction {
            instructions: Rc::from(instructions.as_bytes()),
            num_locals,
            num_parameters: num_params,
            source_map: inner_source_map,
        });
        let index = self.add_constant(compiled_fn)?;
        self.emit(Opcode::Closure, &[index, num_free], span);
        Ok(())
    }

    /// Compiles an `impl` block by emitting each method as a named function
    /// binding. Methods with a `self` parameter have it compiled as the first
    /// regular parameter.
    fn compile_impl_block(&mut self, impl_block: &ImplBlock) -> Result<()> {
        let type_name = match &impl_block.self_type {
            TypeExpr::Named(n) => &n.name,
            TypeExpr::Generic(name, _, _) => name,
            _ => return Ok(()),
        };

        for method in &impl_block.methods {
            let span = method.span;
            let qualified_name = format!("{}::{}", type_name, method.name);

            self.compile_fn_body(
                Some(&qualified_name),
                method.param_names(),
                method.params.len(),
                &method.body,
                span,
            )?;

            let symbol = match self.symbols_table.define_symbol(&qualified_name, false) {
                Ok(s) => s,
                Err(e) => return Err(self.attach_span(e, span)),
            };
            let (scope, index) = (symbol.scope, symbol.index);
            match scope {
                SymbolScope::Global => self.emit(Opcode::SetGlobal, &[index], span),
                SymbolScope::Local => self.emit(Opcode::SetLocal, &[index], span),
                SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {
                    unreachable!("define_symbol never produces this scope")
                }
            };
        }
        Ok(())
    }

    fn enter_scope(&mut self) {
        self.scopes.push(CompilationScope::new());
        self.scope_index += 1;

        let outer = std::mem::take(&mut self.symbols_table);
        self.symbols_table = SymbolsTable::new_enclosed(outer);
    }

    /// Leaves the current compilation scope, returning its instructions
    /// and source map.
    ///
    /// Restores the outer symbol table and pops the scope stack.
    ///
    /// # Errors
    ///
    /// Returns `CompileError::ScopeUnderflow` if the scope stack is empty
    /// or the enclosed symbol table has no outer table to restore.
    fn leave_scope(&mut self) -> Result<(Instructions, SourceMap)> {
        if self.scope_index == 0 {
            return Err(CompileError::new(CompileErrorKind::ScopeUnderflow).into());
        }
        let scope = self
            .scopes
            .pop()
            .ok_or(CompileError::new(CompileErrorKind::ScopeUnderflow))?;
        self.scope_index -= 1;

        let current = std::mem::take(&mut self.symbols_table);
        self.symbols_table = current
            .take_outer()
            .ok_or(CompileError::new(CompileErrorKind::ScopeUnderflow))?;

        Ok((scope.instructions, scope.source_map))
    }

    /// Defines a symbol and emits the corresponding store instruction.
    fn define_and_set(&mut self, name: &str, mutable: bool, span: Span) -> Result<Symbol> {
        let symbol = match self.symbols_table.define_symbol(name, mutable) {
            Ok(s) => s.clone(),
            Err(e) => return Err(self.attach_span(e, span)),
        };
        self.emit_set_symbol(&symbol, span);
        Ok(symbol)
    }

    /// Emits the appropriate store instruction for a resolved symbol.
    ///
    /// Dispatches to `SetGlobal` or `SetLocal` based on the symbol's scope.
    fn emit_set_symbol(&mut self, symbol: &Symbol, span: Span) {
        match symbol.scope {
            SymbolScope::Global => self.emit(Opcode::SetGlobal, &[symbol.index], span),
            SymbolScope::Local => self.emit(Opcode::SetLocal, &[symbol.index], span),
            SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {
                unreachable!("define_symbol never produces this scope")
            }
        };
    }

    /// Emits the appropriate load instruction for a resolved symbol.
    ///
    /// Dispatches to the correct opcode based on the symbol's scope:
    /// global, local, builtin, free variable, or current closure.
    fn load_symbol(&mut self, symbol: &Symbol, span: Span) {
        match symbol.scope {
            SymbolScope::Global => self.emit(Opcode::GetGlobal, &[symbol.index], span),
            SymbolScope::Local => self.emit(Opcode::GetLocal, &[symbol.index], span),
            SymbolScope::Builtin => self.emit(Opcode::GetBuiltin, &[symbol.index], span),
            SymbolScope::Free => self.emit(Opcode::GetFree, &[symbol.index], span),
            SymbolScope::Function => self.emit(Opcode::CurrentClosure, &[], span),
        };
    }

    /// Emits a bytecode instruction with the given opcode and operands.
    ///
    /// Records the source span in the current scope's source map.
    /// Returns the starting position of the emitted instruction.
    fn emit(&mut self, opcode: Opcode, operands: &[usize], span: Span) -> usize {
        let instruction = encode(opcode, operands);
        let pos = self.add_instruction(&instruction);
        self.scopes[self.scope_index].source_map.add(pos, span);
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
        let op =
            Opcode::from_byte(byte).ok_or(CompileError::new(CompileErrorKind::InvalidOpcode {
                opcode: byte,
                position: op_pos,
            }))?;
        let new_inst = encode(op, &[operand]);
        scope.instructions.replace_bytes(op_pos, &new_inst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
        use maat_ast::{ExprStmt, I64, PrefixExpr, Radix};

        let expr = Expr::Prefix(PrefixExpr {
            operator: "~".to_string(),
            operand: Box::new(Expr::I64(I64 {
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
        let result = compiler.compile(&Node::Program(program));

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
