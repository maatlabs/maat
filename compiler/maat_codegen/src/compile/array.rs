use maat_ast::{ArrayLit, IndexExpr, InfixExpr, MethodCallExpr};
use maat_bytecode::{Opcode, TypeTag};
use maat_errors::Result;
use maat_runtime::{Integer, Value};
use maat_span::Span;

use super::Compiler;
use crate::symbol::Symbol;

impl Compiler {
    pub(crate) fn compile_array_literal(&mut self, arr: &ArrayLit, span: Span) -> Result<()> {
        if arr.elements.is_empty() {
            let zero_idx = self.add_constant(Value::Integer(Integer::I64(0)))?;
            self.emit(Opcode::Constant, &[zero_idx], span);
            self.emit(Opcode::HeapAlloc, &[], span);
            return Ok(());
        }
        for element in &arr.elements {
            self.compile_expression(element)?;
            self.emit(Opcode::HeapAlloc, &[], span);
        }
        for _ in 1..arr.elements.len() {
            self.emit(Opcode::Pop, &[], span);
        }
        Ok(())
    }

    pub(crate) fn compile_index_expression(
        &mut self,
        index_expr: &IndexExpr,
        span: Span,
    ) -> Result<()> {
        if index_expr.array_len.is_some() {
            self.compile_expression(&index_expr.expr)?;
            self.compile_expression(&index_expr.index)?;

            self.emit(Opcode::Convert, &[TypeTag::U64.to_byte() as usize], span);
            self.emit(Opcode::Add, &[], span);
            self.emit(Opcode::HeapRead, &[], span);
            return Ok(());
        }
        self.compile_expression(&index_expr.expr)?;
        self.compile_expression(&index_expr.index)?;
        self.emit(Opcode::Index, &[], span);
        Ok(())
    }

    pub(crate) fn compile_array_equality(
        &mut self,
        infix_expr: &InfixExpr,
        n: usize,
        span: Span,
    ) -> Result<()> {
        let id = self.for_loop_counter;
        self.for_loop_counter += 1;
        let lhs_name = format!("__arr_eq_lhs_{id}");
        let rhs_name = format!("__arr_eq_rhs_{id}");

        self.compile_expression(&infix_expr.lhs)?;
        let lhs_sym = self.define_and_set(&lhs_name, false, span)?;
        self.compile_expression(&infix_expr.rhs)?;
        let rhs_sym = self.define_and_set(&rhs_name, false, span)?;

        if n == 0 {
            self.emit(Opcode::True, &[], span);
        } else {
            self.emit_array_element_equal(&lhs_sym, &rhs_sym, 0, span)?;
            let mut end_jumps: Vec<usize> = Vec::with_capacity(n.saturating_sub(1));
            for i in 1..n {
                let cond_jump = self.emit(Opcode::CondJump, &[Self::JUMP], span);
                self.emit_array_element_equal(&lhs_sym, &rhs_sym, i, span)?;
                let jump_to_end = self.emit(Opcode::Jump, &[Self::JUMP], span);
                let false_branch = self.current_instructions().len();
                self.replace_operand(cond_jump, false_branch)?;
                self.emit(Opcode::False, &[], span);
                end_jumps.push(jump_to_end);
            }
            let end_pos = self.current_instructions().len();
            for jump in end_jumps {
                self.replace_operand(jump, end_pos)?;
            }
        }

        if infix_expr.operator == "!=" {
            self.emit(Opcode::Bang, &[], span);
        }
        Ok(())
    }

    fn emit_array_element_equal(
        &mut self,
        lhs_sym: &Symbol,
        rhs_sym: &Symbol,
        i: usize,
        span: Span,
    ) -> Result<()> {
        let i_const = self.add_constant(Value::Integer(Integer::U64(i as u64)))?;
        self.load_symbol(lhs_sym, span);
        self.emit(Opcode::Constant, &[i_const], span);
        self.emit(Opcode::Add, &[], span);
        self.emit(Opcode::HeapRead, &[], span);
        self.load_symbol(rhs_sym, span);
        self.emit(Opcode::Constant, &[i_const], span);
        self.emit(Opcode::Add, &[], span);
        self.emit(Opcode::HeapRead, &[], span);
        self.emit(Opcode::Equal, &[], span);
        Ok(())
    }

    pub(crate) fn compile_array_len_constant(
        &mut self,
        mc: &MethodCallExpr,
        n: usize,
        span: Span,
    ) -> Result<()> {
        self.compile_expression(&mc.object)?;
        self.emit(Opcode::Pop, &[], span);
        let idx = self.add_constant(Value::Integer(Integer::Usize(n)))?;
        self.emit(Opcode::Constant, &[idx], span);
        Ok(())
    }
}
