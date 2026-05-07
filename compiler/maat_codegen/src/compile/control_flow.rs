use maat_ast::*;
use maat_bytecode::Opcode;
use maat_errors::{CompileErrorKind, Result};
use maat_runtime::{Integer, Value};
use maat_span::Span;

use super::{Compiler, LoopContext};
use crate::symbol::Symbol;

impl Compiler {
    pub(crate) fn compile_loop(&mut self, loop_stmt: &LoopStmt) -> Result<()> {
        let span = loop_stmt.span;
        let bound = loop_stmt.bound;

        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let counter_name = format!("__bound_{id}");
        let bound_name = format!("__blimit_{id}");

        let bound_idx = self.add_constant(Value::Integer(Integer::I64(bound as i64)))?;
        self.emit(Opcode::Constant, &[bound_idx], span);
        let bound_sym = self.define_and_set(&bound_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let counter_sym = self.define_and_set(&counter_name, false, span)?;

        let loop_start = self.current_instructions().len();
        self.loop_contexts.push(LoopContext {
            label: loop_stmt.label.clone(),
            continue_target: None,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        });
        self.load_symbol(&counter_sym, span);
        self.load_symbol(&bound_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.compile_block_statement(&loop_stmt.body)?;

        let continue_target = self.current_instructions().len();
        self.load_symbol(&counter_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&counter_sym, span);

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

    pub(crate) fn compile_while(&mut self, while_stmt: &WhileStmt) -> Result<()> {
        let span = while_stmt.span;
        let bound = while_stmt.bound;

        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let counter_name = format!("__bound_{id}");
        let bound_name = format!("__blimit_{id}");

        let bound_idx = self.add_constant(Value::Integer(Integer::I64(bound as i64)))?;
        self.emit(Opcode::Constant, &[bound_idx], span);
        let bound_sym = self.define_and_set(&bound_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let counter_sym = self.define_and_set(&counter_name, false, span)?;

        let loop_start = self.current_instructions().len();
        self.loop_contexts.push(LoopContext {
            label: while_stmt.label.clone(),
            continue_target: None,
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        });

        self.compile_expression(&while_stmt.condition)?;
        let cond_exit = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&counter_sym, span);
        self.load_symbol(&bound_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let bound_exit = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.compile_block_statement(&while_stmt.body)?;

        let continue_target = self.current_instructions().len();
        self.load_symbol(&counter_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&counter_sym, span);

        self.emit(Opcode::Jump, &[loop_start], span);
        let loop_exit = self.current_instructions().len();
        self.replace_operand(cond_exit, loop_exit)?;
        self.replace_operand(bound_exit, loop_exit)?;
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

    pub(crate) fn compile_for_range(&mut self, for_stmt: &ForStmt) -> Result<()> {
        let span = for_stmt.span;
        let Expr::Range(ref range) = *for_stmt.iterable else {
            unreachable!("compile_for_range called with non-range iterable");
        };
        let inclusive = range.inclusive;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let end_name = format!("__end_{id}");
        let i_name = format!("__i_{id}");

        self.compile_expression(&range.end)?;
        let end_sym = self.define_and_set(&end_name, false, span)?;

        self.compile_expression(&range.start)?;
        let i_sym = self.define_and_set(&i_name, false, span)?;

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
            self.emit(Opcode::GreaterThan, &[], span);
            self.emit(Opcode::Bang, &[], span);
        } else {
            self.emit(Opcode::LessThan, &[], span);
        }
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&i_sym, span);
        let _elem_sym = self.define_and_set(&for_stmt.ident, false, span)?;

        self.compile_block_statement(&for_stmt.body)?;

        let elem_kind = range.kind.as_ref();
        self.finalize_counting_loop(&i_sym, loop_start, exit_jump, elem_kind, span)
    }

    pub(crate) fn compile_for_array(&mut self, for_stmt: &ForStmt) -> Result<()> {
        let span = for_stmt.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let iter_name = format!("__iter_{id}");
        let len_name = format!("__len_{id}");
        let i_name = format!("__i_{id}");

        self.compile_expression(&for_stmt.iterable)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, false, span)?;

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

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let _elem_sym = self.define_and_set(&for_stmt.ident, false, span)?;

        self.compile_block_statement(&for_stmt.body)?;

        self.finalize_counting_loop(&i_sym, loop_start, exit_jump, None, span)
    }

    pub(crate) fn compile_for_map(&mut self, for_stmt: &ForStmt) -> Result<()> {
        let span = for_stmt.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let iter_name = format!("__miter_{id}");
        let keys_name = format!("__mkeys_{id}");
        let len_name = format!("__mlen_{id}");
        let i_name = format!("__mi_{id}");

        self.compile_expression(&for_stmt.iterable)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let keys_builtin = self.resolve_or_error("Map::keys", span)?;
        self.load_symbol(&keys_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let keys_sym = self.define_and_set(&keys_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&keys_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, false, span)?;

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

        self.load_symbol(&keys_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);

        let key_name = format!("__mkey_{id}");
        let key_sym = self.define_and_set(&key_name, false, span)?;

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&key_sym, span);
        self.emit(Opcode::Index, &[], span);

        let val_name = format!("__mval_{id}");
        let val_sym = self.define_and_set(&val_name, false, span)?;
        self.load_symbol(&key_sym, span);
        self.load_symbol(&val_sym, span);
        self.emit(Opcode::Tuple, &[2], span);

        let pattern = for_stmt
            .pattern
            .as_ref()
            .expect("compile_for_map requires a pattern");
        self.compile_let_destructure(pattern, span)?;
        self.compile_block_statement(&for_stmt.body)?;
        self.finalize_counting_loop(&i_sym, loop_start, exit_jump, None, span)
    }

    pub(crate) fn finalize_counting_loop(
        &mut self,
        i_sym: &Symbol,
        loop_start: usize,
        exit_jump: usize,
        elem_kind: Option<&NumKind>,
        span: Span,
    ) -> Result<()> {
        let continue_target = self.current_instructions().len();
        self.load_symbol(i_sym, span);
        let one = elem_kind
            .map(Integer::one_of_kind)
            .unwrap_or(Integer::I64(1));
        let one_idx = self.add_constant(Value::Integer(one))?;
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

    pub(crate) fn compile_break(&mut self, expr: &BreakExpr) -> Result<()> {
        if self.loop_contexts.is_empty() {
            return Err(CompileErrorKind::BreakOutsideLoop.at(expr.span).into());
        }
        let ctx_index = self.resolve_loop_label(&expr.label, expr.span)?;
        match &expr.value {
            Some(val) => self.compile_expression(val)?,
            None => {
                self.emit(Opcode::Unit, &[], expr.span);
            }
        }
        let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], expr.span);
        self.loop_contexts[ctx_index].break_jumps.push(jump_pos);
        Ok(())
    }

    pub(crate) fn compile_continue(&mut self, expr: &ContinueExpr) -> Result<()> {
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

    pub(crate) fn resolve_loop_label(&self, label: &Option<String>, span: Span) -> Result<usize> {
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

    pub(crate) fn compile_conditional(&mut self, cond: &CondExpr) -> Result<()> {
        self.compile_expression(&cond.condition)?;

        let cond_jump_pos = self.emit(Opcode::CondJump, &[Self::JUMP], cond.span);

        self.compile_block_statement(&cond.consequence)?;
        if self.last_instruction_is(Opcode::Pop) {
            self.remove_last_pop();
        } else {
            self.emit(Opcode::Unit, &[], cond.span);
        }
        let jump_pos = self.emit(Opcode::Jump, &[Self::JUMP], cond.span);
        let cons_pos = self.current_instructions().len();
        self.replace_operand(cond_jump_pos, cons_pos)?;
        match &cond.alternative {
            None => {
                self.emit(Opcode::Unit, &[], cond.span);
            }
            Some(alt_block) => {
                self.compile_block_statement(alt_block)?;
                if self.last_instruction_is(Opcode::Pop) {
                    self.remove_last_pop();
                } else {
                    self.emit(Opcode::Unit, &[], cond.span);
                }
            }
        }
        let alt_pos = self.current_instructions().len();
        self.replace_operand(jump_pos, alt_pos)?;
        Ok(())
    }

    pub(crate) fn compile_match(&mut self, match_expr: &MatchExpr) -> Result<()> {
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
                    self.emit(Opcode::Unit, &[], span);
                    end_jumps.push(self.emit(Opcode::Jump, &[Self::JUMP], span));
                }
            }
        }

        self.emit(Opcode::Unit, &[], span);
        let exit = self.current_instructions().len();
        for jump_pos in end_jumps {
            self.replace_operand(jump_pos, exit)?;
        }
        Ok(())
    }

    pub(crate) fn compile_match_arm_body(&mut self, body: &Expr, span: Span) -> Result<usize> {
        self.compile_expression(body)?;
        if self.last_instruction_is(Opcode::Pop) {
            self.remove_last_pop();
        }
        Ok(self.emit(Opcode::Jump, &[Self::JUMP], span))
    }

    pub(crate) fn find_variant_in_registry(&self, variant: &str) -> Option<(usize, usize, usize)> {
        self.variant_index
            .get(variant)
            .map(|entry| (entry.registry_index, entry.tag, entry.field_count))
    }

    pub(crate) fn bind_variant_fields(
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

    pub(crate) fn compile_try(&mut self, try_expr: &TryExpr) -> Result<()> {
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
}
