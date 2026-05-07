use maat_ast::{unescape_string, *};
use maat_bytecode::{Opcode, TypeTag};
use maat_errors::{CompileErrorKind, Result};
use maat_runtime::Value;
use maat_span::Span;

use super::Compiler;

impl Compiler {
    pub(crate) fn compile_expression(&mut self, expr: &Expr) -> Result<()> {
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
            Expr::Char(c) => {
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
                let constant = Value::Str(unescape_string(&s.value));
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
            Expr::Array(arr) => self.compile_array_literal(arr, span),
            Expr::Map(map) => {
                for (key, value) in &map.pairs {
                    self.compile_expression(key)?;
                    self.compile_expression(value)?;
                }
                self.emit(Opcode::Map, &[map.pairs.len() * 2], span);
                Ok(())
            }
            Expr::Index(index_expr) => self.compile_index_expression(index_expr, span),
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
                let tag = TypeTag::from_cast_target(cast.target);
                self.emit(Opcode::Convert, &[tag.to_byte() as usize], span);
                Ok(())
            }
            Expr::Break(break_expr) => self.compile_break(break_expr),
            Expr::Continue(cont_expr) => self.compile_continue(cont_expr),
            Expr::MacroCall(mc) => self.compile_macro_call(mc),
            Expr::MacroLit(_) => Err(CompileErrorKind::UnsupportedExpr {
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

    pub(crate) fn compile_infix(&mut self, infix_expr: &InfixExpr, span: Span) -> Result<()> {
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
                if let Some(n) = infix_expr.array_eq_len
                    && matches!(infix_expr.operator.as_str(), "==" | "!=")
                {
                    return self.compile_array_equality(infix_expr, n, span);
                }
                self.compile_expression(&infix_expr.lhs)?;
                self.compile_expression(&infix_expr.rhs)?;
                if matches!(infix_expr.op_class, BinOpClass::Felt) {
                    return self.emit_felt_infix(infix_expr.operator.as_str(), span);
                }
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

    pub(crate) fn emit_felt_infix(&mut self, op: &str, span: Span) -> Result<()> {
        match op {
            "+" => {
                self.emit(Opcode::FeltAdd, &[], span);
            }
            "-" => {
                self.emit(Opcode::FeltSub, &[], span);
            }
            "*" => {
                self.emit(Opcode::FeltMul, &[], span);
            }
            "/" => {
                self.emit(Opcode::FeltInv, &[], span);
                self.emit(Opcode::FeltMul, &[], span);
            }
            "==" => {
                self.emit(Opcode::Equal, &[], span);
            }
            "!=" => {
                self.emit(Opcode::NotEqual, &[], span);
            }
            other => {
                return Err(CompileErrorKind::UnsupportedOperator {
                    operator: other.to_string(),
                    context: "Felt infix expression".to_string(),
                }
                .at(span)
                .into());
            }
        }
        Ok(())
    }
}
