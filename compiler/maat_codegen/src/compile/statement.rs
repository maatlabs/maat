use std::rc::Rc;

use maat_ast::*;
use maat_bytecode::{Constant, MAX_ENUM_VARIANTS, Opcode};
use maat_errors::{CompileErrorKind, Error, Result};
use maat_runtime::{CompiledFn, TypeDef, VariantInfo};
use maat_span::Span;

use super::Compiler;
use crate::symbol::{Symbol, SymbolScope};

impl Compiler {
    pub(crate) fn compile_statement(&mut self, stmt: &Stmt) -> Result<()> {
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
                } else if for_stmt.pattern.is_some() {
                    self.compile_for_map(for_stmt)?;
                } else {
                    self.compile_for_array(for_stmt)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn compile_reassign(&mut self, assign_stmt: &ReAssignStmt) -> Result<()> {
        let span = assign_stmt.span;
        let symbol: Symbol = self.resolve_or_error(&assign_stmt.ident, span)?;
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

    pub(crate) fn compile_block_statement(&mut self, block: &BlockStmt) -> Result<()> {
        self.symbols_table.push_block_scope();
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        self.symbols_table.pop_block_scope();
        Ok(())
    }

    pub(crate) fn compile_fn_body<'a>(
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
        let compiled_fn = Constant::CompiledFn(CompiledFn {
            instructions: Rc::from(instructions.as_bytes()),
            num_locals,
            num_parameters: num_params,
            source_map: inner_source_map,
        });
        let index = self.add_constant(compiled_fn)?;
        self.emit(Opcode::Closure, &[index, num_free], span);
        Ok(())
    }

    pub(crate) fn compile_impl_block(&mut self, impl_block: &ImplBlock) -> Result<()> {
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
}
