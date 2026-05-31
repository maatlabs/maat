mod array;
mod composite;
mod control_flow;
mod expression;
mod macro_call;
mod method;
mod statement;

use std::collections::HashMap;
use std::mem;

use maat_ast::*;
use maat_bytecode::{
    Bytecode, Constant, Instruction, Instructions, MAX_CONSTANT_POOL_SIZE, Opcode, encode,
};
use maat_errors::{CompileError, CompileErrorKind, Error, Result};
use maat_runtime::{Integer, Relocatable, SEG_PUBLIC_INPUT, SEG_PUBLIC_OUTPUT, TypeDef};
use maat_span::{SourceMap, Span};

use crate::registry::{self, VariantEntry};
use crate::symbol::{Symbol, SymbolScope, SymbolsTable};

#[derive(Debug, Clone)]
pub struct Compiler {
    pub(crate) constants: Vec<Constant>,
    pub(crate) symbols_table: SymbolsTable,
    pub(crate) scopes: Vec<CompilationScope>,
    pub(crate) scope_index: usize,
    pub(crate) loop_contexts: Vec<LoopContext>,
    pub(crate) for_loop_counter: usize,
    pub(crate) type_registry: Vec<TypeDef>,
    pub(crate) variant_index: HashMap<String, VariantEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompilationScope {
    pub(crate) instructions: Instructions,
    pub(crate) last_instruction: Option<Instruction>,
    pub(crate) previous_instruction: Option<Instruction>,
    pub(crate) source_map: SourceMap,
}

impl CompilationScope {
    pub(crate) fn new() -> Self {
        Self {
            instructions: Instructions::new(),
            last_instruction: None,
            previous_instruction: None,
            source_map: SourceMap::new(),
        }
    }
}

/// Tracks jump targets for break/continue within a loop.
#[derive(Debug, Clone)]
pub(crate) struct LoopContext {
    pub(crate) label: Option<String>,
    pub(crate) continue_target: Option<usize>,
    pub(crate) break_jumps: Vec<usize>,
    pub(crate) continue_jumps: Vec<usize>,
}

/// Top-level `fn main` entry point facts.
struct MainEntry {
    span: Span,
    param_count: usize,
}

/// Returns the entry-point facts for a top-level `fn main`, if the program
/// declares one.
fn main_entry(program: &Program) -> Option<MainEntry> {
    program.statements.iter().find_map(|stmt| match stmt {
        Stmt::FuncDef(fn_item) if fn_item.name == "main" => Some(MainEntry {
            span: fn_item.span,
            param_count: fn_item.params.len(),
        }),
        _ => None,
    })
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// A deterministic dummy target to jump to.
    /// Ultimately replaced by the actual index downstream.
    pub(crate) const JUMP: usize = 9999;

    pub fn new() -> Self {
        let mut symbols_table = SymbolsTable::new();
        registry::register_builtins(&mut symbols_table);
        let type_registry = registry::builtin_type_registry();
        let variant_index = registry::build_variant_index(&type_registry);

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

    pub fn with_state(mut symbols_table: SymbolsTable, constants: Vec<Constant>) -> Self {
        registry::register_builtins(&mut symbols_table);
        let type_registry = registry::builtin_type_registry();
        let variant_index = registry::build_variant_index(&type_registry);

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

    pub fn symbols_table_mut(&mut self) -> &mut SymbolsTable {
        &mut self.symbols_table
    }

    pub fn register_type(&mut self, typedef: TypeDef) {
        let registry_index = self.type_registry.len();
        if let TypeDef::Enum {
            ref name,
            ref variants,
        } = typedef
        {
            // User-defined enums always get bare variant names in scope.
            registry::index_variants(
                &mut self.variant_index,
                registry_index,
                variants,
                name,
                true,
            );
        }
        self.type_registry.push(typedef);
    }

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

    pub fn symbols_table(&self) -> &SymbolsTable {
        &self.symbols_table
    }

    pub fn compile(&mut self, node: &MaatAst) -> Result<()> {
        match node {
            MaatAst::Program(program) => self.compile_program(program),
            MaatAst::Stmt(stmt) => self.compile_statement(stmt),
            MaatAst::Expr(expr) => self.compile_expression(expr),
        }
    }

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

        if let Some(MainEntry { span, param_count }) = main_entry(program) {
            for stmt in &program.statements {
                self.compile_statement(stmt)?;
            }
            self.emit_main_entry_call(param_count, span)?;
            if program.publishes_main_vector {
                self.publish_vector_on_stack(span)?;
            } else {
                self.emit(Opcode::Pop, &[], span);
            }
            return Ok(());
        }

        let last_idx = program.statements.len().checked_sub(1);
        for (idx, stmt) in program.statements.iter().enumerate() {
            if Some(idx) == last_idx
                && program.publishes_main_vector
                && let Stmt::Expr(expr_stmt) = stmt
            {
                self.compile_main_vector_publication(expr_stmt)?;
            } else {
                self.compile_statement(stmt)?;
            }
        }
        Ok(())
    }

    fn emit_main_entry_call(&mut self, param_count: usize, span: Span) -> Result<()> {
        let main_sym = self.resolve_or_error("main", span)?;
        self.load_symbol(&main_sym, span);
        for offset in 0..param_count {
            let off = u32::try_from(offset).map_err(|_| {
                Error::from(
                    CompileErrorKind::UnsupportedExpr {
                        expr_type: "`fn main` parameter count exceeds the u32 address space"
                            .to_string(),
                    }
                    .at(span),
                )
            })?;
            let addr_idx = self.add_constant(Constant::Relocatable(Relocatable::new(
                SEG_PUBLIC_INPUT,
                off,
            )))?;
            self.emit(Opcode::Constant, &[addr_idx], span);
            self.emit(Opcode::HeapRead, &[], span);
        }
        let arg_count = u8::try_from(param_count).map_err(|_| {
            Error::from(
                CompileErrorKind::UnsupportedExpr {
                    expr_type: "`fn main` accepts at most 255 public parameters".to_string(),
                }
                .at(span),
            )
        })?;
        self.emit(Opcode::Call, &[usize::from(arg_count)], span);
        Ok(())
    }

    fn compile_main_vector_publication(&mut self, expr_stmt: &ExprStmt) -> Result<()> {
        self.compile_expression(&expr_stmt.value)?;
        self.publish_vector_on_stack(expr_stmt.span)
    }

    fn publish_vector_on_stack(&mut self, span: Span) -> Result<()> {
        let iter_sym = self.define_and_set("__main_publish_iter", false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set("__main_publish_len", false, span)?;

        let zero_idx = self.add_constant(Constant::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set("__main_publish_i", true, span)?;

        let output_base_idx = self.add_constant(Constant::Relocatable(Relocatable::new(
            SEG_PUBLIC_OUTPUT,
            0,
        )))?;
        let one_idx = self.add_constant(Constant::Integer(Integer::I64(1)))?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.emit(Opcode::Constant, &[output_base_idx], span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Add, &[], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);

        self.emit(Opcode::HeapWrite, &[], span);

        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);

        self.emit(Opcode::Jump, &[loop_start], span);
        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;

        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Pop, &[], span);
        Ok(())
    }

    pub(crate) fn compile_numeric_constant(&mut self, entry: Constant, span: Span) -> Result<()> {
        let index = self.add_constant(entry)?;
        self.emit(Opcode::Constant, &[index], span);
        Ok(())
    }

    pub(crate) fn add_constant(&mut self, entry: Constant) -> Result<usize> {
        let index = self.constants.len();
        if index > MAX_CONSTANT_POOL_SIZE {
            return Err(CompileError::new(CompileErrorKind::ConstantPoolOverflow {
                max: MAX_CONSTANT_POOL_SIZE,
                attempted: index,
            })
            .into());
        }
        self.constants.push(entry);
        Ok(index)
    }

    pub(crate) fn emit_builtin_call(
        &mut self,
        name: &str,
        const_args: &[Constant],
        span: Span,
    ) -> Result<()> {
        let builtin_idx = registry::resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        for arg in const_args {
            let idx = self.add_constant(arg.clone())?;
            self.emit(Opcode::Constant, &[idx], span);
        }
        self.emit(Opcode::Call, &[const_args.len()], span);
        Ok(())
    }

    pub(crate) fn emit_builtin_call_expr(
        &mut self,
        name: &str,
        arg: &Expr,
        span: Span,
    ) -> Result<()> {
        let builtin_idx = registry::resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        self.compile_expression(arg)?;
        self.emit(Opcode::Call, &[1], span);
        Ok(())
    }

    pub(crate) fn emit_builtin_call_stack(&mut self, name: &str, span: Span) -> Result<()> {
        let temp_name = format!("__macro_tmp_{}", self.current_instructions().len());
        let symbol = self.define_and_set(&temp_name, false, span)?;
        let builtin_idx = registry::resolve_builtin_index(name);
        self.emit(Opcode::GetBuiltin, &[builtin_idx], span);
        self.load_symbol(&symbol, span);
        self.emit(Opcode::Call, &[1], span);
        Ok(())
    }

    pub(crate) fn emit(&mut self, opcode: Opcode, operands: &[usize], span: Span) -> usize {
        let instruction = encode(opcode, operands);
        let pos = self.add_instruction(&instruction);
        self.scopes[self.scope_index].source_map.add(pos, span);
        self.set_last_instruction(opcode, pos);
        pos
    }

    pub(crate) fn add_instruction(&mut self, instruction: &[u8]) -> usize {
        let scope = &mut self.scopes[self.scope_index];
        let pos = scope.instructions.len();
        scope.instructions.extend_from_bytes(instruction);
        pos
    }

    pub(crate) fn set_last_instruction(&mut self, opcode: Opcode, position: usize) {
        let scope = &mut self.scopes[self.scope_index];
        scope.previous_instruction = scope.last_instruction;
        scope.last_instruction = Some(Instruction { opcode, position });
    }

    pub(crate) fn define_and_set(
        &mut self,
        name: &str,
        mutable: bool,
        span: Span,
    ) -> Result<Symbol> {
        let symbol = match self.symbols_table.define_symbol(name, mutable) {
            Ok(s) => s.clone(),
            Err(e) => return Err(self.attach_span(e, span)),
        };
        self.emit_set_symbol(&symbol, span);
        Ok(symbol)
    }

    pub(crate) fn define_anonymous_local(&mut self, span: Span) -> Result<Symbol> {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("__destructure_{id}");
        self.define_and_set(&name, false, span)
    }

    pub(crate) fn emit_set_symbol(&mut self, symbol: &Symbol, span: Span) {
        match symbol.scope {
            SymbolScope::Global => self.emit(Opcode::SetGlobal, &[symbol.index], span),
            SymbolScope::Local => self.emit(Opcode::SetLocal, &[symbol.index], span),
            SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {
                unreachable!("define_symbol never produces this scope")
            }
        };
    }

    pub(crate) fn load_symbol(&mut self, symbol: &Symbol, span: Span) {
        match symbol.scope {
            SymbolScope::Global => self.emit(Opcode::GetGlobal, &[symbol.index], span),
            SymbolScope::Local => self.emit(Opcode::GetLocal, &[symbol.index], span),
            SymbolScope::Builtin => self.emit(Opcode::GetBuiltin, &[symbol.index], span),
            SymbolScope::Free => self.emit(Opcode::GetFree, &[symbol.index], span),
            SymbolScope::Function => self.emit(Opcode::CurrentClosure, &[], span),
        };
    }

    pub(crate) fn current_instructions(&self) -> &Instructions {
        &self.scopes[self.scope_index].instructions
    }

    pub(crate) fn last_instruction_is(&self, opcode: Opcode) -> bool {
        self.scopes[self.scope_index]
            .last_instruction
            .is_some_and(|last| last.opcode == opcode)
    }

    pub(crate) fn remove_last_pop(&mut self) {
        let scope = &mut self.scopes[self.scope_index];
        if let Some(last) = scope.last_instruction {
            scope.instructions.truncate(last.position);
            scope.last_instruction = scope.previous_instruction;
        }
    }

    pub(crate) fn replace_last_pop_with_return_value(&mut self) {
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

    pub(crate) fn replace_operand(&mut self, op_pos: usize, operand: usize) -> Result<()> {
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

    pub(crate) fn replace_match_tag_target(&mut self, op_pos: usize, target: usize) -> Result<()> {
        let scope = &mut self.scopes[self.scope_index];
        let target_bytes = (target as u16).to_be_bytes();
        scope.instructions.replace_bytes(op_pos + 3, &target_bytes);
        Ok(())
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(CompilationScope::new());
        self.scope_index += 1;
        let outer = mem::take(&mut self.symbols_table);
        self.symbols_table = SymbolsTable::new_enclosed(outer);
    }

    pub(crate) fn leave_scope(&mut self) -> Result<(Instructions, SourceMap)> {
        if self.scope_index == 0 {
            return Err(CompileError::new(CompileErrorKind::ScopeUnderflow).into());
        }
        let scope = self
            .scopes
            .pop()
            .ok_or(CompileError::new(CompileErrorKind::ScopeUnderflow))?;
        self.scope_index -= 1;
        let current = mem::take(&mut self.symbols_table);
        self.symbols_table = current
            .take_outer()
            .ok_or(CompileError::new(CompileErrorKind::ScopeUnderflow))?;
        Ok((scope.instructions, scope.source_map))
    }

    pub(crate) fn resolve_or_error(&mut self, name: &str, span: Span) -> Result<Symbol> {
        self.symbols_table.resolve_symbol(name).ok_or_else(|| {
            CompileErrorKind::UndefinedVariable {
                name: name.to_string(),
            }
            .at(span)
            .into()
        })
    }

    pub(crate) fn attach_span(&self, err: Error, span: Span) -> Error {
        match err {
            Error::Compile(ce) if ce.span.is_none() => CompileError {
                kind: ce.kind,
                span: Some(span),
            }
            .into(),
            other => other,
        }
    }
}
