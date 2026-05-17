use maat_ast::MethodCallExpr;
use maat_bytecode::Opcode;
use maat_errors::{CompileErrorKind, Result};
use maat_runtime::{Integer, TypeDef, Value};

use super::Compiler;
use crate::registry::BUILTIN_METHOD_PREFIXES;

impl Compiler {
    pub(crate) fn compile_method_call(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        if self.is_desugared_higher_order(mc) {
            return self.compile_desugared_higher_order(mc);
        }
        if let Some(n) = mc.array_len
            && (mc.method == "len" || mc.method == "length")
            && mc.arguments.is_empty()
        {
            return self.compile_array_len_constant(mc, n, span);
        }
        let qualified_name = self.resolve_method_name(mc).ok_or_else(|| {
            CompileErrorKind::UndefinedVariable {
                name: format!("unknown method `{}`", mc.method),
            }
            .at(span)
        })?;
        if qualified_name == "Vector::push" && mc.arguments.len() == 1 {
            self.compile_expression(&mc.object)?;
            self.compile_expression(&mc.arguments[0])?;
            self.emit(Opcode::VectorPush, &[], span);
            self.emit(Opcode::Pop, &[], span);
            return Ok(());
        }
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

    pub(crate) fn resolve_method_name(&mut self, mc: &MethodCallExpr) -> Option<String> {
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

    pub(crate) fn is_desugared_higher_order(&self, mc: &MethodCallExpr) -> bool {
        matches!(
            (mc.receiver.as_deref(), mc.method.as_str()),
            (Some("Option"), "map" | "and_then" | "unwrap_or_else")
                | (
                    Some("Result"),
                    "map" | "and_then" | "map_err" | "unwrap_or_else" | "or_else"
                )
                | (
                    Some("Vector"),
                    "map"
                        | "filter"
                        | "fold"
                        | "any"
                        | "all"
                        | "find"
                        | "position"
                        | "for_each"
                        | "flat_map"
                )
        )
    }

    pub(crate) fn compile_desugared_higher_order(&mut self, mc: &MethodCallExpr) -> Result<()> {
        match mc.receiver.as_deref() {
            Some("Option" | "Result") => self.compile_option_result_higher_order(mc),
            Some("Vector") => self.compile_vector_iterator(mc),
            _ => unreachable!("is_desugared_higher_order already validated receiver"),
        }
    }

    fn compile_option_result_higher_order(&mut self, mc: &MethodCallExpr) -> Result<()> {
        match mc.method.as_str() {
            "map" | "and_then" => self.compile_option_result_map_like(mc),
            "unwrap_or_else" => self.compile_option_result_unwrap_or_else(mc),
            "map_err" => self.compile_result_map_err(mc),
            "or_else" => self.compile_result_or_else(mc),
            _ => unreachable!("unhandled Option/Result HOF: {}", mc.method),
        }
    }

    fn compile_option_result_map_like(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let is_option = mc.receiver.as_deref() == Some("Option");
        let is_map = mc.method == "map";

        let success_tag: usize = 0;
        let type_index: usize = if is_option { 0 } else { 1 };
        let some_or_ok = type_index << 8;
        let none_or_err = (type_index << 8) | 1;

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
            // `Result::Err`: the scrutinee is already the Err variant; keep it.
        }
        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    fn compile_option_result_unwrap_or_else(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let is_option = mc.receiver.as_deref() == Some("Option");
        let success_tag: usize = 0;

        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let fn_name = format!("__hof_fn_{id}");
        let val_name = format!("__hof_val_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;

        let match_tag_pos = self.emit(Opcode::MatchTag, &[success_tag, Self::JUMP], span);
        self.emit(Opcode::GetField, &[0], span);
        let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let fail_arm = self.current_instructions().len();
        self.replace_match_tag_target(match_tag_pos, fail_arm)?;

        if is_option {
            self.emit(Opcode::Pop, &[], span);
            self.load_symbol(&fn_sym, span);
            self.emit(Opcode::Call, &[0], span);
        } else {
            self.emit(Opcode::GetField, &[0], span);
            let val_sym = self.define_and_set(&val_name, false, span)?;
            self.load_symbol(&fn_sym, span);
            self.load_symbol(&val_sym, span);
            self.emit(Opcode::Call, &[1], span);
        }

        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    fn compile_result_map_err(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let ok_tag: usize = 0;
        let err_construct: usize = (1 << 8) | 1; // Result type_index=1, Err tag=1

        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let fn_name = format!("__hof_fn_{id}");
        let val_name = format!("__hof_val_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;

        let match_tag_pos = self.emit(Opcode::MatchTag, &[ok_tag, Self::JUMP], span);
        let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let err_arm = self.current_instructions().len();
        self.replace_match_tag_target(match_tag_pos, err_arm)?;

        self.emit(Opcode::GetField, &[0], span);
        let val_sym = self.define_and_set(&val_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&val_sym, span);
        self.emit(Opcode::Call, &[1], span);
        self.emit(Opcode::Construct, &[err_construct, 1], span);

        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    fn compile_result_or_else(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let ok_tag: usize = 0;

        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let fn_name = format!("__hof_fn_{id}");
        let val_name = format!("__hof_val_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;

        let match_tag_pos = self.emit(Opcode::MatchTag, &[ok_tag, Self::JUMP], span);
        let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let err_arm = self.current_instructions().len();
        self.replace_match_tag_target(match_tag_pos, err_arm)?;

        self.emit(Opcode::GetField, &[0], span);
        let val_sym = self.define_and_set(&val_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&val_sym, span);
        self.emit(Opcode::Call, &[1], span);

        let end = self.current_instructions().len();
        self.replace_operand(jump_to_end, end)?;
        Ok(())
    }

    fn compile_vector_iterator(&mut self, mc: &MethodCallExpr) -> Result<()> {
        match mc.method.as_str() {
            "map" => self.compile_vector_map(mc),
            "filter" => self.compile_vector_filter(mc),
            "fold" => self.compile_vector_fold(mc),
            "any" => self.compile_vector_any_all(mc, true),
            "all" => self.compile_vector_any_all(mc, false),
            "find" => self.compile_vector_find(mc),
            "position" => self.compile_vector_position(mc),
            "for_each" => self.compile_vector_for_each(mc),
            "flat_map" => self.compile_vector_flat_map(mc),
            _ => unreachable!("is_desugared_higher_order already validated method"),
        }
    }

    fn compile_vector_map(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vmap_fn_{id}");
        let iter_name = format!("__vmap_iter_{id}");
        let len_name = format!("__vmap_len_{id}");
        let result_name = format!("__vmap_result_{id}");
        let i_name = format!("__vmap_i_{id}");
        let elem_name = format!("__vmap_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        self.emit(Opcode::Vector, &[0], span);
        let result_sym = self.define_and_set(&result_name, true, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        let push_builtin = self.resolve_or_error("Vector::push", span)?;
        self.load_symbol(&push_builtin, span);
        self.load_symbol(&result_sym, span);
        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);
        self.emit(Opcode::Call, &[2], span);
        self.emit_set_symbol(&result_sym, span);

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);

        self.emit(Opcode::Jump, &[loop_start], span);

        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;
        self.load_symbol(&result_sym, span);
        Ok(())
    }

    fn compile_vector_filter(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vfilt_fn_{id}");
        let iter_name = format!("__vfilt_iter_{id}");
        let len_name = format!("__vfilt_len_{id}");
        let result_name = format!("__vfilt_result_{id}");
        let i_name = format!("__vfilt_i_{id}");
        let elem_name = format!("__vfilt_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        self.emit(Opcode::Vector, &[0], span);
        let result_sym = self.define_and_set(&result_name, true, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let skip_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        let push_builtin = self.resolve_or_error("Vector::push", span)?;
        self.load_symbol(&push_builtin, span);
        self.load_symbol(&result_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[2], span);
        self.emit_set_symbol(&result_sym, span);

        let skip_target = self.current_instructions().len();
        self.replace_operand(skip_jump, skip_target)?;

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);

        self.emit(Opcode::Jump, &[loop_start], span);

        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;
        self.load_symbol(&result_sym, span);
        Ok(())
    }

    fn compile_vector_fold(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vfold_fn_{id}");
        let acc_name = format!("__vfold_acc_{id}");
        let iter_name = format!("__vfold_iter_{id}");
        let len_name = format!("__vfold_len_{id}");
        let i_name = format!("__vfold_i_{id}");
        let elem_name = format!("__vfold_elem_{id}");

        self.compile_expression(&mc.arguments[1])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.arguments[0])?;
        let acc_sym = self.define_and_set(&acc_name, true, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&acc_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[2], span);
        self.emit_set_symbol(&acc_sym, span);

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);

        self.emit(Opcode::Jump, &[loop_start], span);

        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;
        self.load_symbol(&acc_sym, span);
        Ok(())
    }

    fn compile_vector_any_all(&mut self, mc: &MethodCallExpr, is_any: bool) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vaa_fn_{id}");
        let iter_name = format!("__vaa_iter_{id}");
        let len_name = format!("__vaa_len_{id}");
        let i_name = format!("__vaa_i_{id}");
        let elem_name = format!("__vaa_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);

        let (early_value, default_value) = if is_any { (true, false) } else { (false, true) };

        if is_any {
            let continue_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

            let early_idx = self.add_constant(Value::Bool(early_value))?;
            self.emit(Opcode::Constant, &[early_idx], span);
            let early_exit = self.emit(Opcode::Jump, &[Self::JUMP], span);

            let continue_target = self.current_instructions().len();
            self.replace_operand(continue_jump, continue_target)?;

            self.load_symbol(&i_sym, span);
            let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
            self.emit(Opcode::Constant, &[one_idx], span);
            self.emit(Opcode::Add, &[], span);
            self.emit_set_symbol(&i_sym, span);
            self.emit(Opcode::Jump, &[loop_start], span);

            let default_target = self.current_instructions().len();
            self.replace_operand(exit_jump, default_target)?;
            let default_idx = self.add_constant(Value::Bool(default_value))?;
            self.emit(Opcode::Constant, &[default_idx], span);

            let end = self.current_instructions().len();
            self.replace_operand(early_exit, end)?;
        } else {
            let early_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

            self.load_symbol(&i_sym, span);
            let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
            self.emit(Opcode::Constant, &[one_idx], span);
            self.emit(Opcode::Add, &[], span);
            self.emit_set_symbol(&i_sym, span);
            self.emit(Opcode::Jump, &[loop_start], span);

            let early_target = self.current_instructions().len();
            self.replace_operand(early_jump, early_target)?;
            let early_idx = self.add_constant(Value::Bool(early_value))?;
            self.emit(Opcode::Constant, &[early_idx], span);
            let early_exit = self.emit(Opcode::Jump, &[Self::JUMP], span);

            let default_target = self.current_instructions().len();
            self.replace_operand(exit_jump, default_target)?;
            let default_idx = self.add_constant(Value::Bool(default_value))?;
            self.emit(Opcode::Constant, &[default_idx], span);

            let end = self.current_instructions().len();
            self.replace_operand(early_exit, end)?;
        }
        Ok(())
    }

    fn compile_vector_find(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vfind_fn_{id}");
        let iter_name = format!("__vfind_iter_{id}");
        let len_name = format!("__vfind_len_{id}");
        let i_name = format!("__vfind_i_{id}");
        let elem_name = format!("__vfind_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);

        let continue_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&elem_sym, span);
        let some_tag = 0usize; // Option type_index=0, Some tag=0
        self.emit(Opcode::Construct, &[some_tag, 1], span);
        let early_exit = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let continue_target = self.current_instructions().len();
        self.replace_operand(continue_jump, continue_target)?;

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);
        self.emit(Opcode::Jump, &[loop_start], span);

        let default_target = self.current_instructions().len();
        self.replace_operand(exit_jump, default_target)?;
        let none_tag = 1usize; // Option None tag=1
        self.emit(Opcode::Construct, &[none_tag, 0], span);

        let end = self.current_instructions().len();
        self.replace_operand(early_exit, end)?;
        Ok(())
    }

    fn compile_vector_position(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vpos_fn_{id}");
        let iter_name = format!("__vpos_iter_{id}");
        let len_name = format!("__vpos_len_{id}");
        let i_name = format!("__vpos_i_{id}");
        let elem_name = format!("__vpos_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::Usize(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);

        let continue_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&i_sym, span);
        let some_tag = 0usize;
        self.emit(Opcode::Construct, &[some_tag, 1], span);
        let early_exit = self.emit(Opcode::Jump, &[Self::JUMP], span);

        let continue_target = self.current_instructions().len();
        self.replace_operand(continue_jump, continue_target)?;

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::Usize(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);
        self.emit(Opcode::Jump, &[loop_start], span);

        let default_target = self.current_instructions().len();
        self.replace_operand(exit_jump, default_target)?;
        let none_tag = 1usize;
        self.emit(Opcode::Construct, &[none_tag, 0], span);

        let end = self.current_instructions().len();
        self.replace_operand(early_exit, end)?;
        Ok(())
    }

    fn compile_vector_for_each(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vfe_fn_{id}");
        let iter_name = format!("__vfe_iter_{id}");
        let len_name = format!("__vfe_len_{id}");
        let i_name = format!("__vfe_i_{id}");
        let elem_name = format!("__vfe_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);
        self.emit(Opcode::Pop, &[], span);

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);
        self.emit(Opcode::Jump, &[loop_start], span);

        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;

        let unit_idx = self.add_constant(Value::Unit)?;
        self.emit(Opcode::Constant, &[unit_idx], span);
        Ok(())
    }

    fn compile_vector_flat_map(&mut self, mc: &MethodCallExpr) -> Result<()> {
        let span = mc.span;
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;

        let fn_name = format!("__vfm_fn_{id}");
        let iter_name = format!("__vfm_iter_{id}");
        let len_name = format!("__vfm_len_{id}");
        let result_name = format!("__vfm_result_{id}");
        let i_name = format!("__vfm_i_{id}");
        let elem_name = format!("__vfm_elem_{id}");

        self.compile_expression(&mc.arguments[0])?;
        let fn_sym = self.define_and_set(&fn_name, false, span)?;

        self.compile_expression(&mc.object)?;
        let iter_sym = self.define_and_set(&iter_name, false, span)?;

        let len_builtin = self.resolve_or_error("Vector::len", span)?;
        self.load_symbol(&len_builtin, span);
        self.load_symbol(&iter_sym, span);
        self.emit(Opcode::Call, &[1], span);
        let len_sym = self.define_and_set(&len_name, false, span)?;

        self.emit(Opcode::Vector, &[0], span);
        let result_sym = self.define_and_set(&result_name, true, span)?;

        let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
        self.emit(Opcode::Constant, &[zero_idx], span);
        let i_sym = self.define_and_set(&i_name, true, span)?;

        let loop_start = self.current_instructions().len();
        self.load_symbol(&i_sym, span);
        self.load_symbol(&len_sym, span);
        self.emit(Opcode::LessThan, &[], span);
        let exit_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);

        self.load_symbol(&iter_sym, span);
        self.load_symbol(&i_sym, span);
        self.emit(Opcode::Index, &[], span);
        let elem_sym = self.define_and_set(&elem_name, false, span)?;

        let chain_builtin = self.resolve_or_error("Vector::chain", span)?;
        self.load_symbol(&chain_builtin, span);
        self.load_symbol(&result_sym, span);
        self.load_symbol(&fn_sym, span);
        self.load_symbol(&elem_sym, span);
        self.emit(Opcode::Call, &[1], span);
        self.emit(Opcode::Call, &[2], span);
        self.emit_set_symbol(&result_sym, span);

        self.load_symbol(&i_sym, span);
        let one_idx = self.add_constant(Value::Integer(Integer::I64(1)))?;
        self.emit(Opcode::Constant, &[one_idx], span);
        self.emit(Opcode::Add, &[], span);
        self.emit_set_symbol(&i_sym, span);
        self.emit(Opcode::Jump, &[loop_start], span);

        let loop_exit = self.current_instructions().len();
        self.replace_operand(exit_jump, loop_exit)?;
        self.load_symbol(&result_sym, span);
        Ok(())
    }
}
