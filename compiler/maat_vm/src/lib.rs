//! Virtual machine for executing Maat bytecode.
//!
//! This crate implements a stack-based virtual machine that executes
//! compiled bytecode instructions. The VM maintains a value stack and
//! instruction pointer, executing operations sequentially.

use maat_bytecode::{Bytecode, Opcode};
use maat_errors::{Result, VmError};
use maat_eval::Object;

/// Maximum number of elements that can be pushed onto the stack.
const STACK_SIZE: usize = 2048;

/// Virtual machine for executing bytecode instructions.
///
/// The VM uses a stack-based architecture where operands are pushed onto
/// a stack, operations pop operands and push results, maintaining the
/// instruction pointer to track execution progress.
#[derive(Debug)]
pub struct VM {
    constants: Vec<Object>,
    instructions: Vec<u8>,
    stack: Vec<Object>,
    sp: usize,
}

impl VM {
    /// Creates a new virtual machine from compiled bytecode.
    ///
    /// Initializes the VM with the provided constants and instructions,
    /// allocating a stack of size `STACK_SIZE`.
    pub fn new(bytecode: Bytecode) -> Self {
        Self {
            constants: bytecode.constants,
            instructions: bytecode.instructions.into(),
            stack: Vec::with_capacity(STACK_SIZE),
            sp: 0,
        }
    }

    /// Executes the bytecode instructions.
    ///
    /// Iterates through instructions, dispatching to appropriate handlers
    /// for each opcode. Returns an error if execution fails.
    pub fn run(&mut self) -> Result<()> {
        let mut ip = 0;

        while ip < self.instructions.len() {
            let op = Opcode::from_byte(self.instructions[ip]).ok_or_else(|| {
                VmError::new(format!("unknown opcode: {}", self.instructions[ip]))
            })?;

            match op {
                Opcode::Constant => {
                    let const_index = self.read_u16(ip + 1);
                    ip += 2;
                    let constant = self.constants[const_index as usize].clone();
                    self.push(constant)?;
                }

                Opcode::Pop => {
                    self.pop()?;
                }

                Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => {
                    self.execute_binary_operation(op)?;
                }

                Opcode::True => {
                    self.push(Object::Boolean(true))?;
                }

                Opcode::False => {
                    self.push(Object::Boolean(false))?;
                }

                Opcode::Equal | Opcode::NotEqual | Opcode::GreaterThan | Opcode::LessThan => {
                    self.execute_comparison(op)?;
                }

                Opcode::Bang => {
                    self.execute_bang_operator()?;
                }

                Opcode::Minus => {
                    self.execute_minus_operator()?;
                }
            }

            ip += 1;
        }

        Ok(())
    }

    /// Reads a 16-bit unsigned integer from the instruction stream.
    #[inline]
    fn read_u16(&self, offset: usize) -> u16 {
        u16::from_be_bytes([self.instructions[offset], self.instructions[offset + 1]])
    }

    /// Pushes a value onto the stack.
    fn push(&mut self, obj: Object) -> Result<()> {
        if self.sp >= STACK_SIZE {
            return Err(VmError::new("stack overflow").into());
        }

        if self.sp >= self.stack.len() {
            self.stack.push(obj);
        } else {
            self.stack[self.sp] = obj;
        }
        self.sp += 1;

        Ok(())
    }

    /// Pops a value from the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack is empty (stack underflow).
    fn pop(&mut self) -> Result<Object> {
        if self.sp == 0 {
            return Err(VmError::new("stack underflow").into());
        }

        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    /// Executes a binary arithmetic operation.
    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop()?;
        let left = self.pop()?;

        match (&left, &right) {
            (Object::I64(l), Object::I64(r)) => self.execute_binary_integer_operation(op, *l, *r),
            _ => Err(VmError::new(format!(
                "unsupported types for binary operation: {} {}",
                left.type_name(),
                right.type_name()
            ))
            .into()),
        }
    }

    /// Executes a binary integer arithmetic operation.
    fn execute_binary_integer_operation(
        &mut self,
        op: Opcode,
        left: i64,
        right: i64,
    ) -> Result<()> {
        let result = match op {
            Opcode::Add => left.checked_add(right),
            Opcode::Sub => left.checked_sub(right),
            Opcode::Mul => left.checked_mul(right),
            Opcode::Div => left.checked_div(right),
            _ => return Err(VmError::new(format!("unknown integer operator: {:?}", op)).into()),
        };

        let result = result.ok_or_else(|| VmError::new("integer arithmetic overflow"))?;
        self.push(Object::I64(result))
    }

    /// Executes a comparison operation.
    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop()?;
        let left = self.pop()?;

        match (&left, &right) {
            (Object::I64(l), Object::I64(r)) => self.execute_integer_comparison(op, *l, *r),
            _ => {
                let result = match op {
                    Opcode::Equal => left == right,
                    Opcode::NotEqual => left != right,
                    _ => {
                        return Err(VmError::new(format!(
                            "unknown operator: {:?} ({} {})",
                            op,
                            left.type_name(),
                            right.type_name()
                        ))
                        .into());
                    }
                };
                self.push(Object::Boolean(result))
            }
        }
    }

    /// Executes an integer comparison operation.
    fn execute_integer_comparison(&mut self, op: Opcode, left: i64, right: i64) -> Result<()> {
        let result = match op {
            Opcode::Equal => left == right,
            Opcode::NotEqual => left != right,
            Opcode::GreaterThan => left > right,
            Opcode::LessThan => left < right,
            _ => return Err(VmError::new(format!("unknown operator: {:?}", op)).into()),
        };

        self.push(Object::Boolean(result))
    }

    /// Executes the logical NOT (bang) operator.
    fn execute_bang_operator(&mut self) -> Result<()> {
        let operand = self.pop()?;

        let result = match operand {
            Object::Boolean(true) => Object::Boolean(false),
            Object::Boolean(false) => Object::Boolean(true),
            _ => Object::Boolean(false),
        };

        self.push(result)
    }

    /// Executes the unary minus operator.
    fn execute_minus_operator(&mut self) -> Result<()> {
        let operand = self.pop()?;

        match operand {
            Object::I64(value) => {
                let result = value
                    .checked_neg()
                    .ok_or_else(|| VmError::new("integer negation overflow"))?;
                self.push(Object::I64(result))
            }
            _ => Err(VmError::new(format!(
                "unsupported type for negation: {}",
                operand.type_name()
            ))
            .into()),
        }
    }

    /// Returns the last value popped from the stack.
    ///
    /// This is primarily used for testing to retrieve the result of
    /// an expression after execution completes.
    #[cfg(test)]
    pub fn last_popped_stack_elem(&self) -> Option<&Object> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use maat_ast::{Node, Program};
    use maat_codegen::Compiler;
    use maat_lexer::Lexer;
    use maat_parse::Parser;

    use super::*;

    #[derive(Debug)]
    enum TestValue {
        Int(i64),
        Bool(bool),
    }

    fn parse(input: &str) -> Program {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        parser.parse_program()
    }

    fn run_vm_test(input: &str, expected: TestValue) {
        let program = parse(input);
        let mut compiler = Compiler::new();
        compiler
            .compile(&Node::Program(program))
            .expect("compilation failed");

        let bytecode = compiler.bytecode();
        let mut vm = VM::new(bytecode);
        vm.run().expect("vm error");

        let stack_elem = vm
            .last_popped_stack_elem()
            .expect("no value on stack")
            .clone();

        match expected {
            TestValue::Int(expected_val) => match stack_elem {
                Object::I64(val) => {
                    assert_eq!(val, expected_val, "wrong integer value for input: {input}")
                }
                _ => panic!("expected integer object, got: {:?}", stack_elem),
            },
            TestValue::Bool(expected_val) => match stack_elem {
                Object::Boolean(val) => {
                    assert_eq!(val, expected_val, "wrong boolean value for input: {input}")
                }
                _ => panic!("expected boolean object, got: {:?}", stack_elem),
            },
        }
    }

    #[test]
    fn test_integer_arithmetic() {
        let cases = vec![
            ("1", TestValue::Int(1)),
            ("2", TestValue::Int(2)),
            ("1 + 2", TestValue::Int(3)),
            ("1 - 2", TestValue::Int(-1)),
            ("1 * 2", TestValue::Int(2)),
            ("4 / 2", TestValue::Int(2)),
            ("50 / 2 * 2 + 10 - 5", TestValue::Int(55)),
            ("5 * (2 + 10)", TestValue::Int(60)),
            ("5 + 5 + 5 + 5 - 10", TestValue::Int(10)),
            ("2 * 2 * 2 * 2 * 2", TestValue::Int(32)),
            ("5 * 2 + 10", TestValue::Int(20)),
            ("5 + 2 * 10", TestValue::Int(25)),
            ("5 * (2 + 10)", TestValue::Int(60)),
            ("-5", TestValue::Int(-5)),
            ("-10", TestValue::Int(-10)),
            ("-50 + 100 + -50", TestValue::Int(0)),
            ("(5 + 10 * 2 + 15 / 3) * 2 + -10", TestValue::Int(50)),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn test_boolean_expressions() {
        let cases = vec![
            ("true", TestValue::Bool(true)),
            ("false", TestValue::Bool(false)),
            ("1 < 2", TestValue::Bool(true)),
            ("1 > 2", TestValue::Bool(false)),
            ("1 < 1", TestValue::Bool(false)),
            ("1 > 1", TestValue::Bool(false)),
            ("1 == 1", TestValue::Bool(true)),
            ("1 != 1", TestValue::Bool(false)),
            ("1 == 2", TestValue::Bool(false)),
            ("1 != 2", TestValue::Bool(true)),
            ("true == true", TestValue::Bool(true)),
            ("false == false", TestValue::Bool(true)),
            ("true == false", TestValue::Bool(false)),
            ("true != false", TestValue::Bool(true)),
            ("false != true", TestValue::Bool(true)),
            ("(1 < 2) == true", TestValue::Bool(true)),
            ("(1 < 2) == false", TestValue::Bool(false)),
            ("(1 > 2) == true", TestValue::Bool(false)),
            ("(1 > 2) == false", TestValue::Bool(true)),
            ("!true", TestValue::Bool(false)),
            ("!false", TestValue::Bool(true)),
            ("!5", TestValue::Bool(false)),
            ("!!true", TestValue::Bool(true)),
            ("!!false", TestValue::Bool(false)),
            ("!!5", TestValue::Bool(true)),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn test_stack_underflow() {
        use maat_bytecode::{Instructions, Opcode, encode};
        use maat_errors::Error;

        let mut instructions = Instructions::new();
        instructions.extend(&Instructions::from(encode(Opcode::Pop, &[])));

        let bytecode = Bytecode {
            instructions,
            constants: vec![],
        };

        let mut vm = VM::new(bytecode);
        let result = vm.run();

        assert!(result.is_err(), "should fail on stack underflow");

        match result.unwrap_err() {
            Error::Vm(err) => {
                assert!(
                    err.message.contains("stack underflow"),
                    "expected stack underflow error, got: {}",
                    err.message
                );
            }
            other => panic!("expected VmError, got {:?}", other),
        }
    }
}
