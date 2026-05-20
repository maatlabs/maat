use std::rc::Rc;

use maat_ast::*;
use maat_bytecode::{Constant, Opcode};
use maat_errors::{CompileErrorKind, Result};
use maat_runtime::{CompiledFn, TypeDef};
use maat_span::Span;

use super::Compiler;

impl Compiler {
    pub(crate) fn compile_struct_literal(&mut self, lit: &StructLitExpr) -> Result<()> {
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

    pub(crate) fn compile_path_expression(&mut self, path: &PathExpr) -> Result<()> {
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

    pub(crate) fn emit_variant_constructor(
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
            let compiled_fn = Constant::CompiledFn(CompiledFn {
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

    pub(crate) fn resolve_enum_variant(
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

    pub(crate) fn compile_field_access(&mut self, fa: &FieldAccessExpr) -> Result<()> {
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

    pub(crate) fn compile_let_destructure(&mut self, pattern: &Pattern, span: Span) -> Result<()> {
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
}
