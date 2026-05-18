use maat_ast::{Expr, FmtSegment, Ident, MacroCallExpr, parse_format_string, unescape_string};
use maat_bytecode::{Constant, Opcode};
use maat_errors::{CompileErrorKind, Result};
use maat_span::Span;

use super::Compiler;
use crate::registry;

impl Compiler {
    pub(crate) fn compile_macro_call(&mut self, mc: &MacroCallExpr) -> Result<()> {
        let span = mc.span;
        match mc.name.as_str() {
            "format" => self.compile_format_macro(&mc.arguments, span),
            "print" => self.compile_print_macro(&mc.arguments, false, span),
            "println" => self.compile_print_macro(&mc.arguments, true, span),
            "assert" => self.compile_assert_macro(&mc.arguments, span),
            "assert_eq" => self.compile_assert_eq_macro(&mc.arguments, span),
            "panic" => self.compile_panic_macro(&mc.arguments, span),
            "todo" => self.emit_builtin_call(
                "__panic",
                &[Constant::Str("not yet implemented".to_string())],
                span,
            ),
            "unimplemented" => self.emit_builtin_call(
                "__panic",
                &[Constant::Str("not implemented".to_string())],
                span,
            ),
            _ => Err(CompileErrorKind::UnknownMacro {
                name: mc.name.clone(),
            }
            .at(span)
            .into()),
        }
    }

    fn compile_format_macro(&mut self, args: &[Expr], span: Span) -> Result<()> {
        if args.is_empty() {
            let idx = self.add_constant(Constant::Str(String::new()))?;
            self.emit(Opcode::Constant, &[idx], span);
            return Ok(());
        }

        let fmt = match &args[0] {
            Expr::Str(s) => unescape_string(&s.value),
            _ => {
                return Err(CompileErrorKind::MacroExpectsFormatString {
                    macro_name: "format".to_string(),
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

        let mut arg_idx = 0;
        let mut pieces = 0usize;

        for segment in &segments {
            match segment {
                FmtSegment::Literal(text) => {
                    let idx = self.add_constant(Constant::Str(text.clone()))?;
                    self.emit(Opcode::Constant, &[idx], span);
                    pieces += 1;
                }
                FmtSegment::Arg => {
                    self.emit_builtin_call_expr("__to_string", &value_args[arg_idx], span)?;
                    arg_idx += 1;
                    pieces += 1;
                }
                FmtSegment::Capture(name) => {
                    let ident_expr = Expr::Ident(Ident {
                        value: name.clone(),
                        span,
                    });
                    self.emit_builtin_call_expr("__to_string", &ident_expr, span)?;
                    pieces += 1;
                }
            }
            if pieces > 1 {
                self.emit(Opcode::Add, &[], span);
            }
        }

        if pieces == 0 {
            let idx = self.add_constant(Constant::Str(String::new()))?;
            self.emit(Opcode::Constant, &[idx], span);
        }

        Ok(())
    }

    fn compile_print_macro(&mut self, args: &[Expr], newline: bool, span: Span) -> Result<()> {
        let macro_name = if newline { "println" } else { "print" };

        if args.is_empty() {
            if newline {
                return self.emit_builtin_call(
                    "__print_str_ln",
                    &[Constant::Str(String::new())],
                    span,
                );
            }
            return self.emit_builtin_call("__print_str", &[Constant::Str(String::new())], span);
        }
        let fmt = match &args[0] {
            Expr::Str(s) => unescape_string(&s.value),
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

        // Each builtin call pushes a unit result. We pop all intermediate
        // results and keep only the final call's result as the expression value.
        let mut arg_idx = 0;
        let mut emitted_calls = 0usize;

        for segment in &segments {
            if emitted_calls > 0 {
                self.emit(Opcode::Pop, &[], span);
            }
            match segment {
                FmtSegment::Literal(text) => {
                    self.emit_builtin_call("__print_str", &[Constant::Str(text.clone())], span)?;
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
            self.emit_builtin_call("__print_str_ln", &[Constant::Str(String::new())], span)?;
        }
        Ok(())
    }

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
                &[Constant::Str("assertion failed".to_string())],
                span,
            )?;
        }
        self.emit(Opcode::Pop, &[], span);

        let after = self.current_instructions().len();
        self.replace_operand(skip_pos, after)?;
        self.emit(Opcode::Unit, &[], span);
        Ok(())
    }

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
            &[Constant::Str(
                "assertion `left == right` failed".to_string(),
            )],
            span,
        )?;
        self.emit(Opcode::Pop, &[], span);

        let after = self.current_instructions().len();
        self.replace_operand(skip_pos, after)?;
        self.emit(Opcode::Unit, &[], span);
        Ok(())
    }

    fn compile_panic_macro(&mut self, args: &[Expr], span: Span) -> Result<()> {
        if args.is_empty() {
            return self.emit_builtin_call(
                "__panic",
                &[Constant::Str("explicit panic".to_string())],
                span,
            );
        }

        let fmt = match &args[0] {
            Expr::Str(s) => unescape_string(&s.value),
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
            return self.emit_builtin_call("__panic", &[Constant::Str(fmt)], span);
        }

        let mut arg_idx = 0;
        let mut first = true;

        for segment in &segments {
            let segment_str = match segment {
                FmtSegment::Literal(text) => {
                    let idx = self.add_constant(Constant::Str(text.clone()))?;
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
                let concat_idx = registry::resolve_builtin_index("__str_concat");
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
}
