use std::collections::HashMap;
use std::rc::Rc;

use maat_ast::{
    BlockStmt, BreakExpr, CondExpr, ContinueExpr, EnumVariantKind, Expr, FieldAccessExpr, ForStmt,
    Ident, ImplBlock, InfixExpr, LoopStmt, MacroCallExpr, MatchExpr, MethodCallExpr, Node,
    NumberKind, PathExpr, Pattern, Program, ReAssignStmt, Stmt, StructLitExpr, TryExpr, TryKind,
    TypeExpr, WhileStmt,
};
use maat_bytecode::{
    Bytecode, Instruction, Instructions, MAX_CONSTANT_POOL_SIZE, MAX_ENUM_VARIANTS, Opcode,
    TypeTag, encode,
};
use maat_errors::{CompileError, CompileErrorKind, Error, Result};
use maat_runtime::{BUILTINS, CompiledFn, Integer, TypeDef, Value, VariantInfo};
use maat_span::{SourceMap, Span};

use crate::{Symbol, SymbolScope, SymbolsTable};

/// Built-in type prefixes for method dispatch fallback.
const BUILTIN_METHOD_PREFIXES: &[&str] =
    &["Vector", "str", "char", "Set", "Map", "Option", "Result"];

/// Enum types whose variants are available as bare names in patterns.
///
/// Mirrors Rust's prelude: `Option` (`Some`, `None`) and `Result`
/// (`Ok`, `Err`) are directly accessible. All other enum variants
/// require a qualified path (e.g., `ParseIntError::InvalidDigit`).
const PRELUDE_ENUMS: &[&str] = &["Option", "Result"];

/// A segment of a parsed format string.
enum FmtSegment {
    /// A literal text segment (between `{}` placeholders).
    Literal(String),
    /// A `{}` placeholder to be replaced by a positional argument.
    Arg,
    /// A `{name}` placeholder resolved as a variable capture.
    Capture(String),
}

/// Tracks jump targets for break/continue within a loop.
#[derive(Debug, Clone)]
struct LoopContext {
    label: Option<String>,
    continue_target: Option<usize>,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
}

/// Per-scope compilation state.
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

/// Pre-computed enum variant lookup entry, indexed by variant name.
#[derive(Debug, Clone, Copy)]
struct VariantEntry {
    registry_index: usize,
    tag: usize,
    field_count: usize,
}

/// Compiler state for generating bytecode from AST nodes.
#[derive(Debug, Clone)]
pub struct Compiler {
    constants: Vec<Value>,
    symbols_table: SymbolsTable,
    scopes: Vec<CompilationScope>,
    scope_index: usize,
    loop_contexts: Vec<LoopContext>,
    for_loop_counter: usize,
    type_registry: Vec<TypeDef>,
    variant_index: HashMap<String, VariantEntry>,
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
        let type_registry = Self::builtin_type_registry();
        let variant_index = Self::build_variant_index(&type_registry);
        Self {
            constants: Vec::new(),
            symbols_table,
            scopes: vec![CompilationScope::new()],
            scope_index: 0,
            loop_contexts: Vec::new(),
            for_loop_counter: 0,
            type_registry,
            variant_index,
        }
    }

    /// Creates a compiler with existing symbols table and constants.
    ///
    /// This enables REPL sessions where variable definitions and constants
    /// persist across multiple compilation passes.
    pub fn with_state(mut symbols_table: SymbolsTable, constants: Vec<Value>) -> Self {
        Self::register_builtins(&mut symbols_table);
        let type_registry = Self::builtin_type_registry();
        let variant_index = Self::build_variant_index(&type_registry);
        Self {
            constants,
            symbols_table,
            scopes: vec![CompilationScope::new()],
            scope_index: 0,
            loop_contexts: Vec::new(),
            for_loop_counter: 0,
            type_registry,
            variant_index,
        }
    }

    /// Returns a mutable reference to the compiler's symbols table.
    ///
    /// Used by the module pipeline to define imported symbols before
    /// compilation, so that cross-module references resolve correctly.
    pub fn symbols_table_mut(&mut self) -> &mut SymbolsTable {
        &mut self.symbols_table
    }

    /// Registers a type definition (struct or enum) in the type registry.
    ///
    /// Used by the module pipeline to register imported type
    /// definitions so that construction and field access work across module
    /// boundaries.
    pub fn register_type(&mut self, typedef: TypeDef) {
        let registry_index = self.type_registry.len();
        if let TypeDef::Enum {
            ref name,
            ref variants,
        } = typedef
        {
            // User-defined enums always get bare variant names in scope.
            Self::index_variants(
                &mut self.variant_index,
                registry_index,
                variants,
                name,
                true,
            );
        }
        self.type_registry.push(typedef);
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
            TypeDef::Enum {
                name: "ParseIntError".to_string(),
                variants: vec![
                    VariantInfo {
                        name: "Empty".to_string(),
                        field_count: 0,
                    },
                    VariantInfo {
                        name: "InvalidDigit".to_string(),
                        field_count: 0,
                    },
                    VariantInfo {
                        name: "Overflow".to_string(),
                        field_count: 0,
                    },
                ],
            },
        ]
    }

    /// Builds the enum variant index from an existing type registry.
    fn build_variant_index(registry: &[TypeDef]) -> HashMap<String, VariantEntry> {
        let mut index = HashMap::new();
        for (registry_index, td) in registry.iter().enumerate() {
            if let TypeDef::Enum { name, variants } = td {
                let in_prelude = PRELUDE_ENUMS.contains(&name.as_str());
                Self::index_variants(&mut index, registry_index, variants, name, in_prelude);
            }
        }
        index
    }

    /// Inserts entries for each variant of an enum at the given registry index.
    ///
    /// Qualified keys (`EnumName::VariantName`) are always registered. Bare
    /// keys (`VariantName`) are only registered when `include_bare` is true,
    /// which is the case for prelude enums like `Option` and `Result`.
    fn index_variants(
        index: &mut HashMap<String, VariantEntry>,
        registry_index: usize,
        variants: &[VariantInfo],
        enum_name: &str,
        include_bare: bool,
    ) {
        for (tag, v) in variants.iter().enumerate() {
            let entry = VariantEntry {
                registry_index,
                tag,
                field_count: v.field_count as usize,
            };
            if include_bare {
                index.insert(v.name.clone(), entry);
            }
            index.insert(format!("{enum_name}::{}", v.name), entry);
        }
    }

    /// Extracts the compiled bytecode and constants.
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
                if let Some(pattern) = &let_stmt.pattern {
                    self.compile_expression(&let_stmt.value)?;
                    self.compile_let_destructure(pattern, span)?;
                    Ok(())
                } else {
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
                    self.define_and_set(&let_stmt.ident, let_stmt.mutable, span)?;
                    Ok(())
                }
            }
            Stmt::ReAssign(assign_stmt) => self.compile_reassign(assign_stmt),
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
                self.define_and_set(&fn_item.name, false, span)?;
                Ok(())
            }
            Stmt::Loop(loop_stmt) => self.compile_loop(loop_stmt),
            Stmt::While(while_stmt) => self.compile_while(while_stmt),
            Stmt::StructDecl(decl) => {
                self.register_type(TypeDef::Struct {
                    name: decl.name.clone(),
                    field_names: decl.fields.iter().map(|f| f.name.clone()).collect(),
                });
                Ok(())
            }
            Stmt::EnumDecl(decl) => {
                let span = decl.span;
                if decl.variants.len() > MAX_ENUM_VARIANTS {
                    return Err(CompileErrorKind::VariantTagOverflow {
                        name: decl.name.clone(),
                        count: decl.variants.len(),
                        max: MAX_ENUM_VARIANTS,
                    }
                    .at(span)
                    .into());
                }
                let variants = decl
                    .variants
                    .iter()
                    .map(|v| {
                        let count = match &v.kind {
                            EnumVariantKind::Unit => 0,
                            EnumVariantKind::Tuple(fields) => fields.len(),
                            EnumVariantKind::Struct(fields) => fields.len(),
                        };
                        let field_count = u8::try_from(count).map_err(|_| {
                            Error::from(
                                CompileErrorKind::UnsupportedExpr {
                                    expr_type: format!(
                                        "variant `{}` has {count} fields, exceeding the u8 maximum",
                                        v.name
                                    ),
                                }
                                .at(span),
                            )
                        })?;
                        Ok(VariantInfo {
                            name: v.name.clone(),
                            field_count,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.register_type(TypeDef::Enum {
                    name: decl.name.clone(),
                    variants,
                });
                Ok(())
            }
            Stmt::TraitDecl(_) => Ok(()),
            Stmt::ImplBlock(impl_block) => self.compile_impl_block(impl_block),
            // Module declarations and import statements are resolved by the
            // module orchestrator before per-module compilation. No-op here.
            Stmt::Use(_) | Stmt::Mod(_) => Ok(()),
            Stmt::For(for_stmt) => {
                if matches!(*for_stmt.iterable, Expr::Range(_)) {
                    self.compile_for_range(for_stmt)?;
                } else {
                    self.compile_for_array(for_stmt)?;
                }
                Ok(())
            }
        }
    }

    /// Compiles a variable reassignment (`x = expr`).
    fn compile_reassign(&mut self, assign_stmt: &ReAssignStmt) -> Result<()> {
        let span = assign_stmt.span;
        let symbol = self.resolve_or_error(&assign_stmt.ident, span)?;
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
            SymbolScope::Local | SymbolScope::Free => {
                self.emit(Opcode::SetLocal, &[symbol.index], span)
            }
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

    /// Compiles an infinite `loop { body }`.
    fn compile_loop(&mut self, loop_stmt: &LoopStmt) -> Result<()> {
        let start = self.current_instructions().len();
        self.loop_contexts.push(LoopContext {
            label: loop_stmt.label.clone(),
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

    /// Compiles a `while condition { body }` loop.
    fn compile_while(&mut self, while_stmt: &WhileStmt) -> Result<()> {
        let start = self.current_instructions().len();
        self.loop_contexts.push(LoopContext {
            label: while_stmt.label.clone(),
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

    /// Compiles a `for x in start..end` or `for x in start..=end` loop.
    ///
    /// Desugars to a counter-based loop without allocating a vector:
    ///
    /// ```text
    /// let __end = <end>;
    /// let __i   = <start>;
    /// loop_start:
    ///     if !(__i < __end) goto loop_exit   // or __i <= __end for inclusive
    ///     let <ident> = __i;
    ///     <body>
    /// continue_target:
    ///     __i = __i + 1
    ///     goto loop_start
    /// loop_exit:
    /// ```
    fn compile_for_range(&mut self, for_stmt: &ForStmt) -> Result<()> {
        let span = for_stmt.span;
        let Expr::Range(ref range) = *for_stmt.iterable else {
            unreachable!("compile_for_range called with non-range iterable");
        };
        let inclusive = range.inclusive;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let end_name = format!("__end_{id}");
        let i_name = format!("__i_{id}");

        // __end_N = <end>
        self.compile_expression(&range.end)?;
        let end_sym = self.define_and_set(&end_name, false, span)?;

        // __i_N = <start>
        self.compile_expression(&range.start)?;
        let i_sym = self.define_and_set(&i_name, false, span)?;

        // loop_start: condition check
        let loop_start = self.current_instructions().len();
        self.loop_contexts.push(LoopContext {
            label: for_stmt.label.clone(),
            continue_target: None,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        });
        self.load_symbol(&i_sym, span);
        self.load_symbol(&end_sym, span);
        if inclusive {
            // __i <= __end ~= !(__i > __end)
            self.emit(Opcode::GreaterThan, &[], span);
            self.emit(Opcode::Bang, &[], span);
        } else {
            self.emit(Opcode::LessThan, &[], span);
        }
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        // let <ident> = __i
        self.load_symbol(&i_sym, span);
        let _elem_sym = self.define_and_set(&for_stmt.ident, false, span)?;

        // body
        self.compile_block_statement(&for_stmt.body)?;

        self.finalize_counting_loop(&i_sym, loop_start, exit_jump, span)
    }

    /// Compiles a `for x in array_expr` loop via index-based desugaring.
    ///
    /// ```text
    /// let __iter = <iterable>;
    /// let __len  = Vector::len(__iter);
    /// let __i    = 0;
    /// loop_start:
    ///     if !(__i < __len) goto loop_exit
    ///     let <ident> = __iter[__i];
    ///     <body>
    /// continue_target:
    ///     __i = __i + 1
    ///     goto loop_start
    /// loop_exit:
    /// ```
    fn compile_for_array(&mut self, for_stmt: &ForStmt) -> Result<()> {
        let span = for_stmt.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let iter_name = format!("__iter_{id}");
        let len_name = format!("__len_{id}");
        let i_name = format!("__i_{id}");

        // __iter_N
        self.compile_expression(&for_stmt.iterable)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        // __len_N = Vector::len(__iter_N)
        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        // __i_N = 0
        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, false, span)?;

        // loop_start: condition check (__i < __len)
        let loop_start = self.current_instructions().len();

        self.loop_contexts.push(LoopContext {
            label: for_stmt.label.clone(),
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
        let _elem_sym = self.define_and_set(&for_stmt.ident, false, span)?;

        // body
        self.compile_block_statement(&for_stmt.body)?;

        self.finalize_counting_loop(&i_sym, loop_start, exit_jump, span)
    }

    /// Emits the shared tail of a counting for-loop: increment, jump back,
    /// and patch break/continue targets.
    fn finalize_counting_loop(
        &mut self,
        i_sym: &Symbol,
        loop_start: usize,
        exit_jump: usize,
        span: Span,
    ) -> Result<()> {
        let continue_target = self.current_instructions().len();
        self.load_symbol(i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(i_sym, span);

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

    /// Compiles a sequence of statements within a lexical block scope.
    fn compile_block_statement(&mut self, block: &BlockStmt) -> Result<()> {
        self.symbols_table.push_block_scope();
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        self.symbols_table.pop_block_scope();
        Ok(())
    }

    /// Emits a constant-load instruction for a numeric literal.
    fn compile_numeric_constant(&mut self, val: Value, span: Span) -> Result<()> {
        let index = self.add_constant(val)?;
        self.emit(Opcode::Constant, &[index], span);
        Ok(())
    }

    /// Compiles an expression node into bytecode.
    fn compile_expression(&mut self, expr: &Expr) -> Result<()> {
        let span = expr.span();
        match expr {
            Expr::Number(lit) => {
                let val = Value::from_number_literal(lit)
                    .map_err(|msg| CompileErrorKind::UnsupportedExpr { expr_type: msg }.at(span))?;
                self.compile_numeric_constant(val, span)
            }
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
            Expr::Infix(infix_expr) => self.compile_infix(infix_expr, span),
            Expr::Cond(cond) => self.compile_conditional(cond),
            Expr::Ident(ident) => {
                if let Some(symbol) = self.symbols_table.resolve_symbol(&ident.value) {
                    self.load_symbol(&symbol, span);
                    Ok(())
                } else if let Some((registry_index, variant_tag, field_count)) =
                    self.find_variant_in_registry(&ident.value)
                {
                    self.emit_variant_constructor(registry_index, variant_tag, field_count, span)
                } else {
                    Err(CompileErrorKind::UndefinedVariable {
                        name: ident.value.clone(),
                    }
                    .at(ident.span)
                    .into())
                }
            }
            Expr::CharLit(c) => {
                let index = self.add_constant(Value::Char(c.value))?;
                self.emit(Opcode::Constant, &[index], span);
                Ok(())
            }
            Expr::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.compile_expression(element)?;
                }
                self.emit(Opcode::Tuple, &[tuple.elements.len()], span);
                Ok(())
            }
            Expr::Str(s) => {
                let constant = Value::Str(maat_ast::unescape_string(&s.value));
                let index = self.add_constant(constant)?;
                self.emit(Opcode::Constant, &[index], span);
                Ok(())
            }
            Expr::Vector(array) => {
                for element in &array.elements {
                    self.compile_expression(element)?;
                }
                self.emit(Opcode::Vector, &[array.elements.len()], span);
                Ok(())
            }
            Expr::Map(map) => {
                for (key, value) in &map.pairs {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.emit(Opcode::Map, &[map.pairs.len() * 2], span);
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
                let tag = num_kind_to_tag(cast.target);
                self.emit(Opcode::Convert, &[tag.to_byte() as usize], span);
                Ok(())
            }
            Expr::Break(break_expr) => self.compile_break(break_expr),
            Expr::Continue(cont_expr) => self.compile_continue(cont_expr),
            Expr::MacroCall(mc) => self.compile_macro_call(mc),
            Expr::Macro(_) => Err(CompileErrorKind::UnsupportedExpr {
                expr_type: "macro literal".to_string(),
            }
            .at(span)
            .into()),
            Expr::StructLit(struct_lit) => self.compile_struct_literal(struct_lit),
            Expr::PathExpr(path_expr) => self.compile_path_expression(path_expr),
            Expr::Try(try_expr) => self.compile_try(try_expr),
            Expr::Match(match_expr) => self.compile_match(match_expr),
            Expr::FieldAccess(field_access) => self.compile_field_access(field_access),
            Expr::MethodCall(method_call) => self.compile_method_call(method_call),
            Expr::Range(range) => {
                self.compile_expression(&range.start)?;
                self.compile_expression(&range.end)?;
                let opcode = if range.inclusive {
                    Opcode::MakeRangeInclusive
                } else {
                    Opcode::MakeRange
                };
                self.emit(opcode, &[], span);
                Ok(())
            }
        }
    }

    /// Compiles a `break` expression inside a loop.
    fn compile_break(&mut self, expr: &BreakExpr) -> Result<()> {
        if self.loop_contexts.is_empty() {
            return Err(CompileErrorKind::BreakOutsideLoop.at(expr.span).into());
        }
        let ctx_index = self.resolve_loop_label(&expr.label, expr.span)?;
        match &expr.value {
            Some(val) => self.compile_expression(val)?,
            None => {
                self.emit(Opcode::Null, &[], expr.span);
            }
        }
        let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], expr.span);
        self.loop_contexts[ctx_index].break_jumps.push(jump_pos);
        Ok(())
    }

    /// Compiles a `continue` expression inside a loop.
    fn compile_continue(&mut self, expr: &ContinueExpr) -> Result<()> {
        if self.loop_contexts.is_empty() {
            return Err(CompileErrorKind::ContinueOutsideLoop.at(expr.span).into());
        }
        let ctx_index = self.resolve_loop_label(&expr.label, expr.span)?;
        match self.loop_contexts[ctx_index].continue_target {
            Some(target) => {
                self.emit(Opcode::Jump, &[target], expr.span);
            }
            None => {
                let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], expr.span);
                self.loop_contexts[ctx_index].continue_jumps.push(jump_pos);
            }
        }
        Ok(())
    }

    /// Resolves a loop label to the index into `self.loop_contexts`.
    ///
    /// Returns the innermost loop's index when no label is specified.
    /// When a label is given, searches outward for a matching loop context.
    fn resolve_loop_label(&self, label: &Option<String>, span: Span) -> Result<usize> {
        match label {
            None => Ok(self.loop_contexts.len() - 1),
            Some(name) => self
                .loop_contexts
                .iter()
                .rposition(|ctx| ctx.label.as_deref() == Some(name))
                .ok_or_else(|| {
                    CompileErrorKind::UndeclaredLabel {
                        label: name.clone(),
                    }
                    .at(span)
                    .into()
                }),
        }
    }

    /// Compiles an infix (binary) expression, including short-circuit `&&`/`||`.
    fn compile_infix(&mut self, infix_expr: &InfixExpr, span: Span) -> Result<()> {
        match infix_expr.operator.as_str() {
            "&&" => {
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
            "||" => {
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
            _ => {
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
        }
    }

    /// Compiles an `if`/`else` conditional expression.
    fn compile_conditional(&mut self, cond: &CondExpr) -> Result<()> {
        self.compile_expression(&cond.condition)?;

        let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP], cond.span);

        self.compile_block_statement(&cond.consequence)?;
        if self.last_instruction_is(Opcode::Pop) {
            self.remove_last_pop();
        } else {
            self.emit(Opcode::Null, &[], cond.span);
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
                } else {
                    self.emit(Opcode::Null, &[], cond.span);
                }
            }
        }
        let alt_pos = self.current_instructions().len();
        self.replace_operand(jump_pos, alt_pos)?;
        Ok(())
    }

    /// Compiles a struct literal expression (e.g., `Point { x: 1, y: 2 }`)
    /// or with functional update syntax (e.g., `Point { x: 10, ..other }`).
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
        let base_sym = lit
            .base
            .as_ref()
            .map(|base_expr| {
                self.compile_expression(base_expr)?;
                let id = self.for_loop_counter;
                self.for_loop_counter += 1;
                let hidden = format!("__struct_base_{id}");
                self.define_and_set(&hidden, false, span)
            })
            .transpose()?;
        for (field_index, field_name) in field_names.iter().enumerate() {
            match lit.fields.iter().find(|(name, _)| name == field_name) {
                Some((_, expr)) => self.compile_expression(expr)?,
                None => {
                    let sym = base_sym.as_ref().ok_or_else(|| {
                        CompileErrorKind::UndefinedVariable {
                            name: format!(
                                "missing field `{}` in struct `{}`",
                                field_name, lit.name
                            ),
                        }
                        .at(span)
                    })?;
                    self.load_symbol(sym, span);
                    self.emit(Opcode::GetField, &[field_index], span);
                }
            }
        }
        let type_index = registry_index << 8;
        self.emit(Opcode::Construct, &[type_index, field_names.len()], span);
        Ok(())
    }

    /// Compiles a path expression (e.g., `Option::None`, `Color::Red`).
    fn compile_path_expression(&mut self, path: &PathExpr) -> Result<()> {
        let span = path.span;
        if path.segments.len() == 2 {
            let type_name = &path.segments[0];
            let variant_name = &path.segments[1];
            if let Some((registry_index, variant_tag, field_count)) =
                self.resolve_enum_variant(type_name, variant_name)
            {
                return self.emit_variant_constructor(
                    registry_index,
                    variant_tag,
                    field_count,
                    span,
                );
            }
            let qualified_name = format!("{type_name}::{variant_name}");
            if let Some(symbol) = self.symbols_table.resolve_symbol(&qualified_name) {
                self.load_symbol(&symbol, span);
                return Ok(());
            }
        }
        let full_name = path.segments.join("::");
        let symbol = self.resolve_or_error(&full_name, span)?;
        self.load_symbol(&symbol, span);
        Ok(())
    }

    /// Emits bytecode to construct an enum variant.
    fn emit_variant_constructor(
        &mut self,
        registry_index: usize,
        variant_tag: usize,
        field_count: usize,
        span: Span,
    ) -> Result<()> {
        let type_index = (registry_index << 8) | (variant_tag & 0xFF);
        if field_count == 0 {
            self.emit(Opcode::Construct, &[type_index, 0], span);
        } else {
            self.enter_scope();
            let mut param_names = Vec::with_capacity(field_count);
            for i in 0..field_count {
                let name = format!("__field_{i}");
                if let Err(e) = self.symbols_table.define_symbol(&name, false) {
                    return Err(self.attach_span(e, span));
                }
                param_names.push(name);
            }
            for name in &param_names {
                let sym = self.resolve_or_error(name, span)?;
                self.load_symbol(&sym, span);
            }
            self.emit(Opcode::Construct, &[type_index, field_count], span);
            self.emit(Opcode::ReturnValue, &[], span);

            let num_locals = self.symbols_table.max_definitions();
            let (instructions, inner_source_map) = self.leave_scope()?;
            let compiled_fn = Value::CompiledFn(CompiledFn {
                instructions: Rc::from(instructions.as_bytes()),
                num_locals,
                num_parameters: field_count,
                source_map: inner_source_map,
            });
            let index = self.add_constant(compiled_fn)?;
            self.emit(Opcode::Closure, &[index, 0], span);
        }
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
    fn compile_match(&mut self, match_expr: &MatchExpr) -> Result<()> {
        let span = match_expr.span;
        self.compile_expression(&match_expr.scrutinee)?;

        let mut end_jumps = Vec::with_capacity(match_expr.arms.len());
        for arm in &match_expr.arms {
            match &arm.pattern {
                Pattern::TupleStruct { path, fields, .. } => {
                    if let Some((_, variant_tag, _)) = self.find_variant_in_registry(path) {
                        let match_tag_pos =
                            self.emit(Opcode::MatchTag, &[variant_tag, Self::JUMP], span);

                        let (nested_positions, scrutinee_var) =
                            self.bind_variant_fields(fields, span)?;
                        end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);

                        if nested_positions.is_empty() {
                            let next_arm = self.current_instructions().len();
                            self.replace_match_tag_target(match_tag_pos, next_arm)?;
                        } else {
                            let cleanup = self.current_instructions().len();
                            for nested_pos in nested_positions {
                                self.replace_match_tag_target(nested_pos, cleanup)?;
                            }
                            self.emit(Opcode::Pop, &[], span);
                            if let Some(ref var_name) = scrutinee_var {
                                let sym = self.resolve_or_error(var_name, span)?;
                                self.load_symbol(&sym, span);
                            }
                            let next_arm = self.current_instructions().len();
                            self.replace_match_tag_target(match_tag_pos, next_arm)?;
                        }
                    }
                }
                Pattern::Ident { name, mutable, .. } if name != "_" => {
                    if let Some((_, variant_tag, _)) = self.find_variant_in_registry(name) {
                        let match_tag_pos =
                            self.emit(Opcode::MatchTag, &[variant_tag, Self::JUMP], span);
                        self.emit(Opcode::Pop, &[], span);
                        end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);
                        let next_arm = self.current_instructions().len();
                        self.replace_match_tag_target(match_tag_pos, next_arm)?;
                    } else {
                        self.define_and_set(name, *mutable, span)?;
                        end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);
                    }
                }
                Pattern::Wildcard(_) | Pattern::Ident { .. } => {
                    self.emit(Opcode::Pop, &[], span);
                    end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);
                }
                Pattern::Tuple(..) => {
                    self.compile_let_destructure(&arm.pattern, span)?;
                    self.emit(Opcode::Pop, &[], span);
                    end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);
                }
                Pattern::Literal(lit_expr) => {
                    self.compile_expression(lit_expr)?;
                    self.emit(Opcode::Equal, &[], span);
                    let cond_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);
                    end_jumps.push(self.compile_match_arm_body(&arm.body, span)?);
                    let next_arm = self.current_instructions().len();
                    self.replace_operand(cond_jump, next_arm)?;
                }
                _ => {
                    self.emit(Opcode::Pop, &[], span);
                    self.emit(Opcode::Null, &[], span);
                    end_jumps.push(self.emit(Opcode::Jump, &[Self::JUMP], span));
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

    /// Compiles a match arm body and emits the end-jump.
    ///
    /// Returns the jump position for later back-patching.
    fn compile_match_arm_body(&mut self, body: &Expr, span: Span) -> Result<usize> {
        self.compile_expression(body)?;
        if self.last_instruction_is(Opcode::Pop) {
            self.remove_last_pop();
        }
        Ok(self.emit(Opcode::Jump, &[Self::JUMP], span))
    }

    /// Looks up an enum variant by name in the pre-built variant index.
    fn find_variant_in_registry(&self, variant: &str) -> Option<(usize, usize, usize)> {
        self.variant_index
            .get(variant)
            .map(|entry| (entry.registry_index, entry.tag, entry.field_count))
    }

    /// Binds variant payload fields to local variables via `GetField`.
    ///
    /// Returns `(nested_match_positions, scrutinee_var)` where:
    /// - `nested_match_positions` are bytecode offsets of inner `MatchTag`
    ///   instructions whose jump targets must be patched to cleanup code
    /// - `scrutinee_var` is the hidden variable name storing the outer
    ///   scrutinee, needed to restore the stack on inner match failure
    fn bind_variant_fields(
        &mut self,
        fields: &[Pattern],
        span: Span,
    ) -> Result<(Vec<usize>, Option<String>)> {
        let mut nested_match_positions = Vec::new();
        let mut scrutinee_var = None;
        for (i, field) in fields.iter().enumerate() {
            if let Pattern::Ident { name, mutable, .. } = field {
                if i == 0 {
                    let hidden = format!("__match_scrutinee_{}", self.for_loop_counter);
                    self.for_loop_counter += 1;
                    let hidden_sym = self.define_and_set(&hidden, false, span)?;
                    scrutinee_var = Some(hidden.clone());
                    self.load_symbol(&hidden_sym, span);
                    self.emit(Opcode::GetField, &[i], span);
                } else {
                    let hidden = format!("__match_scrutinee_{}", self.for_loop_counter - 1);
                    let hidden_sym = self.resolve_or_error(&hidden, span)?;
                    self.load_symbol(&hidden_sym, span);
                    self.emit(Opcode::GetField, &[i], span);
                }

                if name == "_" {
                    self.emit(Opcode::Pop, &[], span);
                } else if let Some((_, variant_tag, _)) = self.find_variant_in_registry(name) {
                    let pos = self.emit(Opcode::MatchTag, &[variant_tag, Self::JUMP], span);
                    self.emit(Opcode::Pop, &[], span);
                    nested_match_positions.push(pos);
                } else {
                    self.define_and_set(name, *mutable, span)?;
                }
            }
        }
        Ok((nested_match_positions, scrutinee_var))
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
    fn compile_field_access(&mut self, fa: &FieldAccessExpr) -> Result<()> {
        let span = fa.span;
        self.compile_expression(&fa.object)?;
        let field_index = fa
            .field
            .parse::<usize>()
            .ok()
            .or_else(|| {
                self.type_registry.iter().find_map(|td| match td {
                    TypeDef::Struct { field_names, .. } => {
                        field_names.iter().position(|f| f == &fa.field)
                    }
                    _ => None,
                })
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

    /// Compiles a destructuring `let` binding (e.g., `let (x, y) = expr;`).
    fn compile_let_destructure(&mut self, pattern: &Pattern, span: Span) -> Result<()> {
        match pattern {
            Pattern::Tuple(fields, _) => {
                let temp = self.define_anonymous_local(span)?;
                for (i, field) in fields.iter().enumerate() {
                    match field {
                        Pattern::Ident { name, mutable, .. } => {
                            self.load_symbol(&temp, span);
                            self.emit(Opcode::GetField, &[i], span);
                            self.define_and_set(name, *mutable, span)?;
                        }
                        Pattern::Wildcard(_) => {}
                        Pattern::Tuple(..) => {
                            self.load_symbol(&temp, span);
                            self.emit(Opcode::GetField, &[i], span);
                            self.compile_let_destructure(field, span)?;
                        }
                        _ => {
                            return Err(CompileErrorKind::UnsupportedExpr {
                                expr_type: "unsupported pattern in tuple destructuring".to_string(),
                            }
                            .at(span)
                            .into());
                        }
                    }
                }
                Ok(())
            }
            _ => Err(CompileErrorKind::UnsupportedExpr {
                expr_type: "expected tuple pattern in let destructuring".to_string(),
            }
            .at(span)
            .into()),
        }
    }

    /// Defines an anonymous variable and sets it from the stack top.
    fn define_anonymous_local(&mut self, span: Span) -> Result<Symbol> {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("__destructure_{id}");
        self.define_and_set(&name, false, span)
    }

    /// Compiles a method call expression (e.g., `point.distance(other)`).
    fn compile_method_call(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        if self.is_desugared_higher_order(mc) {
            return self.compile_desugared_higher_order(mc);
        }
        let qualified_name = self.resolve_method_name(mc).ok_or_else(|| {
            CompileErrorKind::UndefinedVariable {
                name: format!("unknown method `{}`", mc.method),
            }
            .at(span)
        })?;
        let symbol = self.resolve_or_error(&qualified_name, span)?;
        self.load_symbol(&symbol, span);
        self.compile_expression(&mc.object)?;
        for arg in &mc.arguments {
            self.compile_expression(arg)?;
        }
        let total_args = 1 + mc.arguments.len();
        self.emit(Opcode::Call, &[total_args], span);
        Ok(())
    }

    /// Returns `true` if this method call should be desugared into inline
    /// bytecode rather than dispatched as a builtin call.
    ///
    /// `Option::map`, `Option::and_then`, `Result::map`, and `Result::and_then`
    /// accept closures that require VM-level invocation. These are compiled
    /// as inline match + call sequences instead of builtin function calls.
    fn is_desugared_higher_order(&self, mc: &MethodCallExpr) -> bool {
        matches!(
            (mc.receiver.as_deref(), mc.method.as_str()),
            (Some("Option" | "Result"), "map" | "and_then")
        )
    }

    /// Compiles `Option::map`, `Option::and_then`, `Result::map`, or
    /// `Result::and_then` as inline bytecode equivalent to a match expression.
    ///
    /// For `opt.map(f)`:
    /// ```text
    /// match opt { Some(x) => Some(f(x)), None => None }
    /// ```
    ///
    /// For `opt.and_then(f)`:
    /// ```text
    /// match opt { Some(x) => f(x), None => None }
    /// ```
    ///
    /// Result variants follow the same pattern with Ok/Err.
    fn compile_desugared_higher_order(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let is_option = mc.receiver.as_deref() == Some("Option");
        let is_map = mc.method == "map";

        let success_tag: usize = 0;
        let type_index: usize = if is_option { 0 } else { 1 };
        let some_or_ok = type_index << 8;
        let none_or_err = (type_index << 8) | 1;

        // Unique temp local names.
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let fn_name = format!("__hof_fn_{id}");
        let val_name = format!("__hof_val_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;

        let match_tag_pos = self.emit(Opcode::MatchTag, &[success_tag, Self::JUMP], span);

        self.emit(Opcode::GetField, &[0], span);
        let val_sym = self.define_and_set(&val_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&val_sym, span);
        self.emit(Opcode::Call, &[1], span);

        if is_map {
            self.emit(Opcode::Construct, &[some_or_ok, 1], span);
        }
        let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let fail_arm = self.current_instructions().len();
        self.replace_match_tag_target(match_tag_pos, fail_arm)?;

        if is_option {
            self.emit(Opcode::Pop, &[], span);
            self.emit(Opcode::Construct, &[none_or_err, 0], span);
        } else {
            // `Result::Err`:  the scrutinee is already the Err variant; keep it.
            // Nothing to do: the Err value stays on the stack as-is.
        }
        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    /// Compiles the try operator (`expr?`).
    ///
    /// Desugars to an inline match:
    /// - `Option<T>`: `Some(val) => val`, `None => return None`
    /// - `Result<T, E>`: `Ok(val) => val`, `Err(e) => return Err(e)`
    fn compile_try(&mut self, try_expr: &TryExpr) -> Result<()> {
        let span = try_expr.span;
        let is_option = try_expr.kind == TryKind::Option;

        let success_tag: usize = 0;
        let type_index: usize = if is_option { 0 } else { 1 };
        let none_or_err = (type_index << 8) | 1;

        self.compile_expression(&try_expr.expr)?;

        let match_tag_pos = self.emit(Opcode::MatchTag, &[success_tag, Self::JUMP], span);

        self.emit(Opcode::GetField, &[0], span);
        let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let fail_arm = self.current_instructions().len();
        self.replace_match_tag_target(match_tag_pos, fail_arm)?;

        if is_option {
            self.emit(Opcode::Pop, &[], span);
            self.emit(Opcode::Construct, &[none_or_err, 0], span);
        }
        // For Result, the Err variant is already on the stack.
        self.emit(Opcode::ReturnValue, &[], span);

        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    /// Resolves the fully-qualified builtin name for a method call.
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
            .chain(BUILTIN_METHOD_PREFIXES.iter().copied())
            .find_map(|type_name| {
                let candidate = format!("{type_name}::{}", mc.method);
                self.symbols_table
                    .resolve_symbol(&candidate)
                    .map(|_| candidate)
            })
    }

    /// Resolves a symbol by name, returning an error if undefined.
    fn resolve_or_error(&mut self, name: &str, span: Span) -> Result<Symbol> {
        self.symbols_table.resolve_symbol(name).ok_or_else(|| {
            CompileErrorKind::UndefinedVariable {
                name: name.to_string(),
            }
            .at(span)
            .into()
        })
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
    fn add_constant(&mut self, val: Value) -> Result<usize> {
        let index = self.constants.len();
        if index > MAX_CONSTANT_POOL_SIZE {
            return Err(CompileError::new(CompileErrorKind::ConstantPoolOverflow {
                max: MAX_CONSTANT_POOL_SIZE,
                attempted: index,
            })
            .into());
        }
        self.constants.push(val);
        Ok(index)
    }

    /// Returns a reference to the current scope's instruction stream.
    fn current_instructions(&self) -> &Instructions {
        &self.scopes[self.scope_index].instructions
    }

    /// Enters a new compilation scope for a function body.
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
        let compiled_fn = Value::CompiledFn(CompiledFn {
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
            self.define_and_set(&qualified_name, false, span)?;
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
        scope.instructions.extend_from_bytes(instruction);
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

    /// Dispatches a builtin macro call to the appropriate expansion routine.
    fn compile_macro_call(&mut self, mc: &MacroCallExpr) -> Result<()> {
        let span = mc.span;
        match mc.name.as_str() {
            "print" => self.compile_print_macro(&mc.arguments, false, span),
            "println" => self.compile_print_macro(&mc.arguments, true, span),
            "assert" => self.compile_assert_macro(&mc.arguments, span),
            "assert_eq" => self.compile_assert_eq_macro(&mc.arguments, span),
            "panic" => self.compile_panic_macro(&mc.arguments, span),
            "todo" => self.emit_builtin_call(
                "__panic",
                &[Value::Str("not yet implemented".to_string())],
                span,
            ),
            "unimplemented" => self.emit_builtin_call(
                "__panic",
                &[Value::Str("not implemented".to_string())],
                span,
            ),
            _ => Err(CompileErrorKind::UnknownMacro {
                name: mc.name.clone(),
            }
            .at(span)
            .into()),
        }
    }

    /// Compiles `print!(fmt, args...)` or `println!(fmt, args...)`.
    fn compile_print_macro(&mut self, args: &[Expr], newline: bool, span: Span) -> Result<()> {
        let macro_name = if newline { "println" } else { "print" };

        if args.is_empty() {
            if newline {
                return self.emit_builtin_call(
                    "__print_str_ln",
                    &[Value::Str(String::new())],
                    span,
                );
            }
            return self.emit_builtin_call("__print_str", &[Value::Str(String::new())], span);
        }
        let fmt = match &args[0] {
            Expr::Str(s) => maat_ast::unescape_string(&s.value),
            _ => {
                return Err(CompileErrorKind::MacroExpectsFormatString {
                    macro_name: macro_name.to_string(),
                }
                .at(span)
                .into());
            }
        };
        let segments = parse_format_string(&fmt);
        let placeholder_count = segments
            .iter()
            .filter(|s| matches!(s, FmtSegment::Arg))
            .count();
        let value_args = &args[1..];
        if placeholder_count != value_args.len() {
            return Err(CompileErrorKind::FormatArgCountMismatch {
                placeholders: placeholder_count,
                arguments: value_args.len(),
            }
            .at(span)
            .into());
        }

        // Each builtin call pushes a Null result. We pop all intermediate
        // results and keep only the final call's result as the expression value.
        let mut arg_idx = 0;
        let mut emitted_calls = 0usize;

        for segment in &segments {
            if emitted_calls > 0 {
                self.emit(Opcode::Pop, &[], span);
            }
            match segment {
                FmtSegment::Literal(text) => {
                    self.emit_builtin_call("__print_str", &[Value::Str(text.clone())], span)?;
                    emitted_calls += 1;
                }
                FmtSegment::Arg => {
                    self.emit_builtin_call_expr("__to_string", &value_args[arg_idx], span)?;
                    self.emit_builtin_call_stack("__print_str", span)?;
                    arg_idx += 1;
                    emitted_calls += 1;
                }
                FmtSegment::Capture(name) => {
                    let ident_expr = Expr::Ident(Ident {
                        value: name.clone(),
                        span,
                    });
                    self.emit_builtin_call_expr("__to_string", &ident_expr, span)?;
                    self.emit_builtin_call_stack("__print_str", span)?;
                    emitted_calls += 1;
                }
            }
        }
        if newline {
            if emitted_calls > 0 {
                self.emit(Opcode::Pop, &[], span);
            }
            self.emit_builtin_call("__print_str_ln", &[Value::Str(String::new())], span)?;
        }
        Ok(())
    }

    /// Compiles `assert!(condition)` or `assert!(condition, message)`.
    fn compile_assert_macro(&mut self, args: &[Expr], span: Span) -> Result<()> {
        if args.is_empty() || args.len() > 2 {
            return Err(CompileErrorKind::FormatArgCountMismatch {
                placeholders: 1,
                arguments: args.len(),
            }
            .at(span)
            .into());
        }

        self.compile_expression(&args[0])?;
        let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP], span);
        let skip_pos = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let panic_start = self.current_instructions().len();
        self.replace_operand(cond_jump_pos, panic_start)?;

        if args.len() == 2 {
            self.compile_expression(&args[1])?;
            self.emit_builtin_call_stack("__panic", span)?;
        } else {
            self.emit_builtin_call(
                "__panic",
                &[Value::Str("assertion failed".to_string())],
                span,
            )?;
        }
        self.emit(Opcode::Pop, &[], span);

        let after = self.current_instructions().len();
        self.replace_operand(skip_pos, after)?;
        self.emit(Opcode::Null, &[], span);
        Ok(())
    }

    /// Compiles `assert_eq!(left, right)`.
    fn compile_assert_eq_macro(&mut self, args: &[Expr], span: Span) -> Result<()> {
        if args.len() != 2 {
            return Err(CompileErrorKind::FormatArgCountMismatch {
                placeholders: 2,
                arguments: args.len(),
            }
            .at(span)
            .into());
        }

        self.compile_expression(&args[0])?;
        self.compile_expression(&args[1])?;
        self.emit(Opcode::Equal, &[], span);

        let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP], span);
        let skip_pos = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let panic_start = self.current_instructions().len();
        self.replace_operand(cond_jump_pos, panic_start)?;
        self.emit_builtin_call(
            "__panic",
            &[Value::Str("assertion `left == right` failed".to_string())],
            span,
        )?;
        self.emit(Opcode::Pop, &[], span);

        let after = self.current_instructions().len();
        self.replace_operand(skip_pos, after)?;
        self.emit(Opcode::Null, &[], span);
        Ok(())
    }

    /// Compiles `panic!()`, `panic!("message")`, or `panic!("fmt {}", args...)`.
    ///
    /// With no arguments, emits `__panic("explicit panic")`. With a plain
    /// string literal, emits `__panic(literal)`. With a format string and
    /// interpolation arguments, builds the message by concatenating segments
    /// via `__to_string` and `__str_concat`, then emits `__panic(result)`.
    fn compile_panic_macro(&mut self, args: &[Expr], span: Span) -> Result<()> {
        if args.is_empty() {
            return self.emit_builtin_call(
                "__panic",
                &[Value::Str("explicit panic".to_string())],
                span,
            );
        }

        let fmt = match &args[0] {
            Expr::Str(s) => maat_ast::unescape_string(&s.value),
            _ => {
                return Err(CompileErrorKind::MacroExpectsFormatString {
                    macro_name: "panic".to_string(),
                }
                .at(span)
                .into());
            }
        };

        let segments = parse_format_string(&fmt);
        let placeholder_count = segments
            .iter()
            .filter(|s| matches!(s, FmtSegment::Arg))
            .count();
        let value_args = &args[1..];
        if placeholder_count != value_args.len() {
            return Err(CompileErrorKind::FormatArgCountMismatch {
                placeholders: placeholder_count,
                arguments: value_args.len(),
            }
            .at(span)
            .into());
        }

        if placeholder_count == 0 {
            return self.emit_builtin_call("__panic", &[Value::Str(fmt)], span);
        }

        let mut arg_idx = 0;
        let mut first = true;

        for segment in &segments {
            let segment_str = match segment {
                FmtSegment::Literal(text) => {
                    let idx = self.add_constant(Value::Str(text.clone()))?;
                    self.emit(Opcode::Constant, &[idx], span);
                    true
                }
                FmtSegment::Arg => {
                    self.emit_builtin_call_expr("__to_string", &value_args[arg_idx], span)?;
                    arg_idx += 1;
                    true
                }
                FmtSegment::Capture(name) => {
                    let ident_expr = Expr::Ident(Ident {
                        value: name.clone(),
                        span,
                    });
                    self.emit_builtin_call_expr("__to_string", &ident_expr, span)?;
                    true
                }
            };

            if segment_str && !first {
                let rhs_tmp = format!("__panic_rhs_{}", self.current_instructions().len());
                let rhs_sym = self.define_and_set(&rhs_tmp, false, span)?;
                let lhs_tmp = format!("__panic_lhs_{}", self.current_instructions().len());
                let lhs_sym = self.define_and_set(&lhs_tmp, false, span)?;
                let concat_idx = resolve_builtin_index("__str_concat");
                self.emit(Opcode::GetBuiltin, &[concat_idx], span);
                self.load_symbol(&lhs_sym, span);
                self.load_symbol(&rhs_sym, span);
                self.emit(Opcode::Call, &[2], span);
            }
            if segment_str {
                first = false;
            }
        }

        self.emit_builtin_call_stack("__panic", span)?;
        Ok(())
    }

    /// Emits a call to a builtin function with constant arguments.
    fn emit_builtin_call(&mut self, name: &str, const_args: &[Value], span: Span) -> Result<()> {
        let builtin_idx = resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        for arg in const_args {
            let idx = self.add_constant(arg.clone())?;
            self.emit(Opcode::Constant, &[idx], span);
        }
        self.emit(Opcode::Call, &[const_args.len()], span);
        Ok(())
    }

    /// Emits a call to a builtin function where the single argument is an
    /// expression to be compiled (pushed onto the stack).
    fn emit_builtin_call_expr(&mut self, name: &str, arg: &Expr, span: Span) -> Result<()> {
        let builtin_idx = resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        self.compile_expression(arg)?;
        self.emit(Opcode::Call, &[1], span);
        Ok(())
    }

    /// Emits a call to a builtin function where the single argument is
    /// already on the stack (from a prior call's return value).
    fn emit_builtin_call_stack(&mut self, name: &str, span: Span) -> Result<()> {
        let temp_name = format!("__macro_tmp_{}", self.current_instructions().len());
        let symbol = self.define_and_set(&temp_name, false, span)?;
        let builtin_idx = resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        self.load_symbol(&symbol, span);
        self.emit(Opcode::Call, &[1], span);
        Ok(())
    }
}

/// Resolves a builtin function name to its index in the [`BUILTINS`] registry.
///
/// Panics if the name is not found. Internal builtins are guaranteed
/// to be present at compile time.
fn resolve_builtin_index(name: &str) -> usize {
    BUILTINS
        .iter()
        .position(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("internal builtin `{name}` not found in registry"))
}

/// Parses a format string into a sequence of literal, positional, and capture segments.
///
/// Handles `{{` and `}}` as escaped braces. `{}` is a positional placeholder,
/// `{name}` is a variable capture (where `name` matches `[a-zA-Z_][a-zA-Z0-9_]*`).
fn parse_format_string(fmt: &str) -> Vec<FmtSegment> {
    let mut segments = Vec::new();
    let mut buf = String::new();
    let mut chars = fmt.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    buf.push('{');
                } else if chars.peek() == Some(&'}') {
                    chars.next();
                    if !buf.is_empty() {
                        segments.push(FmtSegment::Literal(std::mem::take(&mut buf)));
                    }
                    segments.push(FmtSegment::Arg);
                } else {
                    // Try to parse `{identifier}`.
                    let mut name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '}' {
                            break;
                        }
                        name.push(c);
                        chars.next();
                    }
                    if chars.peek() == Some(&'}') && is_identifier(&name) {
                        chars.next();
                        if !buf.is_empty() {
                            segments.push(FmtSegment::Literal(std::mem::take(&mut buf)));
                        }
                        segments.push(FmtSegment::Capture(name));
                    } else {
                        // Not a valid capture; emit as literal text.
                        buf.push('{');
                        buf.push_str(&name);
                    }
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    buf.push('}');
                } else {
                    buf.push('}');
                }
            }
            _ => buf.push(ch),
        }
    }

    if !buf.is_empty() {
        segments.push(FmtSegment::Literal(buf));
    }
    segments
}

/// Returns `true` if `s` is a valid Maat identifier (`[a-zA-Z_][a-zA-Z0-9_]*`).
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Maps a source-level type annotation to a bytecode type tag.
fn num_kind_to_tag(t: NumberKind) -> TypeTag {
    match t {
        NumberKind::I8 => TypeTag::I8,
        NumberKind::I16 => TypeTag::I16,
        NumberKind::I32 => TypeTag::I32,
        NumberKind::I64 => TypeTag::I64,
        NumberKind::I128 => TypeTag::I128,
        NumberKind::Isize => TypeTag::Isize,
        NumberKind::U8 => TypeTag::U8,
        NumberKind::U16 => TypeTag::U16,
        NumberKind::U32 => TypeTag::U32,
        NumberKind::U64 => TypeTag::U64,
        NumberKind::U128 => TypeTag::U128,
        NumberKind::Usize => TypeTag::Usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use maat_ast::{ExprStmt, Number, NumberKind, PrefixExpr, Radix};

        let expr = Expr::Prefix(PrefixExpr {
            operator: "~".to_string(),
            operand: Box::new(Expr::Number(Number {
                kind: NumberKind::I64,
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
