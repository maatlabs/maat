//! Virtual machine for executing Maat bytecode.
//!
//! This crate implements a stack-based virtual machine that executes
//! compiled bytecode instructions. The VM maintains a value stack and
//! instruction pointer, executing operations sequentially.

use std::collections::HashMap;

use maat_bytecode::{Bytecode, MAX_GLOBALS, MAX_STACK_SIZE, Opcode};
use maat_errors::{Result, VmError};
use maat_eval::{FALSE, HashObject, Hashable, NULL, Object, TRUE};

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
    globals: Vec<Object>,
}

impl VM {
    /// Creates a new virtual machine from compiled bytecode.
    ///
    /// Initializes the VM with the provided constants and instructions,
    /// allocating a stack of size `MAX_STACK_SIZE`.
    pub fn new(bytecode: Bytecode) -> Self {
        Self {
            constants: bytecode.constants,
            instructions: bytecode.instructions.into(),
            stack: Vec::with_capacity(MAX_STACK_SIZE),
            sp: 0,
            globals: Vec::with_capacity(MAX_GLOBALS),
        }
    }

    /// Creates a VM with an existing globals store.
    ///
    /// This enables REPL sessions where global variable values persist
    /// across multiple bytecode executions.
    pub fn with_globals(bytecode: Bytecode, globals: Vec<Object>) -> Self {
        Self {
            constants: bytecode.constants,
            instructions: bytecode.instructions.into(),
            stack: Vec::with_capacity(MAX_STACK_SIZE),
            sp: 0,
            globals,
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
                    let index = self.read_operand(ip + 1);
                    ip += 2;
                    let constant = self.constants[index].clone();
                    self.push_stack(constant)?;
                }
                Opcode::Pop => {
                    self.pop_stack()?;
                }
                Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => {
                    self.execute_binary_operation(op)?;
                }
                Opcode::True => self.push_stack(TRUE)?,
                Opcode::False => self.push_stack(FALSE)?,

                Opcode::Equal | Opcode::NotEqual | Opcode::GreaterThan | Opcode::LessThan => {
                    self.execute_comparison(op)?
                }
                Opcode::Bang => self.execute_bang_operator()?,
                Opcode::Minus => self.execute_minus_operator()?,
                Opcode::Jump => {
                    let target = self.read_operand(ip + 1);
                    ip = target;
                    continue;
                }
                Opcode::CondJump => {
                    let target = self.read_operand(ip + 1);
                    ip += 2;
                    let condition = self.pop_stack()?;
                    if !condition.is_truthy() {
                        ip = target;
                        continue;
                    }
                }
                Opcode::Null => {
                    self.push_stack(NULL)?;
                }
                Opcode::SetGlobal => {
                    let index = self.read_operand(ip + 1);
                    ip += 2;
                    let value = self.pop_stack()?;
                    if index >= self.globals.len() {
                        self.globals.resize(index + 1, Object::Null);
                    }
                    self.globals[index] = value;
                }
                Opcode::GetGlobal => {
                    let index = self.read_operand(ip + 1);
                    ip += 2;
                    let value = self.globals[index].clone();
                    self.push_stack(value)?;
                }
                Opcode::Array => {
                    let num_elements = self.read_operand(ip + 1);
                    ip += 2;
                    let array = self.build_array(num_elements);
                    self.push_stack(array)?;
                }
                Opcode::Hash => {
                    let num_elements = self.read_operand(ip + 1);
                    ip += 2;
                    let hash = self.build_hash(num_elements)?;
                    self.push_stack(hash)?;
                }
                Opcode::Index => {
                    let index = self.pop_stack()?;
                    let container = self.pop_stack()?;
                    self.execute_index_expression(container, index)?;
                }
            }

            ip += 1;
        }

        Ok(())
    }

    /// Reads the 16-bit unsigned integer operand from the instruction stream,
    /// returning it as a `usize`.
    #[inline]
    fn read_operand(&self, offset: usize) -> usize {
        u16::from_be_bytes([self.instructions[offset], self.instructions[offset + 1]]) as usize
    }

    /// Pushes a value onto the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack is full (stack overflow).
    fn push_stack(&mut self, obj: Object) -> Result<()> {
        if self.sp >= MAX_STACK_SIZE {
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
    fn pop_stack(&mut self) -> Result<Object> {
        if self.sp == 0 {
            return Err(VmError::new("stack underflow").into());
        }

        self.sp -= 1;
        Ok(self.stack[self.sp].clone())
    }

    /// Executes a binary arithmetic operation.
    fn execute_binary_operation(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

        match (&left, &right) {
            (Object::I64(l), Object::I64(r)) => self.execute_binary_integer_operation(op, *l, *r),
            (Object::String(l), Object::String(r)) => {
                self.execute_binary_string_operation(op, l, r)
            }
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
        self.push_stack(Object::I64(result))
    }

    /// Executes a comparison operation.
    fn execute_comparison(&mut self, op: Opcode) -> Result<()> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;

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
                self.push_stack(Object::Boolean(result))
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

        self.push_stack(Object::Boolean(result))
    }

    /// Executes the logical NOT (bang) operator.
    fn execute_bang_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;
        let result = match operand {
            Object::Boolean(false) | Object::Null => TRUE,
            _ => FALSE,
        };
        self.push_stack(result)
    }

    /// Executes the unary minus operator.
    fn execute_minus_operator(&mut self) -> Result<()> {
        let operand = self.pop_stack()?;
        match operand {
            Object::I64(value) => {
                let result = value
                    .checked_neg()
                    .ok_or_else(|| VmError::new("integer negation overflow"))?;
                self.push_stack(Object::I64(result))
            }
            _ => Err(VmError::new(format!(
                "unsupported type for negation: {}",
                operand.type_name()
            ))
            .into()),
        }
    }

    /// Executes string concatenation via `OpAdd`.
    fn execute_binary_string_operation(
        &mut self,
        op: Opcode,
        left: &str,
        right: &str,
    ) -> Result<()> {
        if op != Opcode::Add {
            return Err(VmError::new(format!("unknown string operator: {}", op.name())).into());
        }
        let mut result = String::with_capacity(left.len() + right.len());
        result.push_str(left);
        result.push_str(right);
        self.push_stack(Object::String(result))
    }

    /// Builds an array object from the top `num_elements` stack values.
    fn build_array(&mut self, num_elements: usize) -> Object {
        let start = self.sp - num_elements;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp -= num_elements;
        Object::Array(elements)
    }

    /// Builds a hash object from the top `num_elements` stack values.
    ///
    /// Elements are expected in alternating key-value order on the stack.
    fn build_hash(&mut self, num_elements: usize) -> Result<Object> {
        let start = self.sp - num_elements;
        let mut pairs = HashMap::with_capacity(num_elements / 2);

        for i in (start..self.sp).step_by(2) {
            let key = self.stack[i].clone();
            let value = self.stack[i + 1].clone();
            let key = Hashable::try_from(key).map_err(|e| VmError::new(e.to_string()))?;
            pairs.insert(key, value);
        }

        self.sp -= num_elements;
        Ok(Object::Hash(HashObject { pairs }))
    }

    /// Dispatches index operations to the appropriate handler.
    fn execute_index_expression(&mut self, container: Object, index: Object) -> Result<()> {
        match (&container, &index) {
            (Object::Array(elements), Object::I64(i)) => self.execute_array_index(elements, *i),
            (Object::Hash(hash), _) => self.execute_hash_index(hash, index),
            _ => Err(VmError::new(format!(
                "index operator not supported: {}",
                container.type_name()
            ))
            .into()),
        }
    }

    /// Indexes into an array with bounds checking.
    fn execute_array_index(&mut self, elements: &[Object], index: i64) -> Result<()> {
        let max = elements.len() as i64 - 1;
        if index < 0 || index > max {
            return self.push_stack(NULL);
        }
        self.push_stack(elements[index as usize].clone())
    }

    /// Indexes into a hash by key.
    fn execute_hash_index(&mut self, hash: &HashObject, index: Object) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| VmError::new(e.to_string()))?;

        match hash.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => self.push_stack(NULL),
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
        Str(String),
        IntArray(Vec<i64>),
        Hash(Vec<(i64, i64)>),
        Null,
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
            TestValue::Str(expected_val) => match stack_elem {
                Object::String(val) => {
                    assert_eq!(val, expected_val, "wrong string value for input: {input}")
                }
                _ => panic!("expected string object, got: {:?}", stack_elem),
            },
            TestValue::IntArray(expected_vals) => match stack_elem {
                Object::Array(elements) => {
                    assert_eq!(
                        elements.len(),
                        expected_vals.len(),
                        "wrong array length for input: {input}"
                    );
                    for (i, expected_elem) in expected_vals.iter().enumerate() {
                        match &elements[i] {
                            Object::I64(val) => assert_eq!(
                                *val, *expected_elem,
                                "wrong array element at index {i} for input: {input}"
                            ),
                            other => {
                                panic!("expected integer in array at index {i}, got: {:?}", other)
                            }
                        }
                    }
                }
                _ => panic!("expected array object, got: {:?}", stack_elem),
            },
            TestValue::Hash(expected_pairs) => match &stack_elem {
                Object::Hash(hash_obj) => {
                    assert_eq!(
                        hash_obj.pairs.len(),
                        expected_pairs.len(),
                        "wrong hash size for input: {input}"
                    );
                    for (key, value) in &expected_pairs {
                        let hash_key = maat_eval::Hashable::I64(*key);
                        let actual = hash_obj.pairs.get(&hash_key).unwrap_or_else(|| {
                            panic!("missing key {key} in hash for input: {input}")
                        });
                        match actual {
                            Object::I64(val) => assert_eq!(
                                *val, *value,
                                "wrong hash value for key {key} in input: {input}"
                            ),
                            other => {
                                panic!("expected integer value for key {key}, got: {:?}", other)
                            }
                        }
                    }
                }
                _ => panic!("expected hash object, got: {:?}", stack_elem),
            },
            TestValue::Null => {
                assert_eq!(
                    stack_elem,
                    Object::Null,
                    "expected null for input: {input}, got: {:?}",
                    stack_elem
                );
            }
        }
    }

    #[test]
    fn integer_arithmetic() {
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
    fn boolean_expressions() {
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
            ("!(if (false) { 5; })", TestValue::Bool(true)),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn conditionals() {
        let cases = vec![
            ("if (true) { 10 }", TestValue::Int(10)),
            ("if (true) { 10 } else { 20 }", TestValue::Int(10)),
            ("if (false) { 10 } else { 20 }", TestValue::Int(20)),
            ("if (1) { 10 }", TestValue::Int(10)),
            ("if (1 < 2) { 10 }", TestValue::Int(10)),
            ("if (1 < 2) { 10 } else { 20 }", TestValue::Int(10)),
            ("if (1 > 2) { 10 } else { 20 }", TestValue::Int(20)),
            ("if (1 > 2) { 10 }", TestValue::Null),
            ("if (false) { 10 }", TestValue::Null),
            (
                "if ((if (false) { 10 })) { 10 } else { 20 }",
                TestValue::Int(20),
            ),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn global_let_statements() {
        let cases = vec![
            ("let one = 1; one", TestValue::Int(1)),
            ("let one = 1; let two = 2; one + two", TestValue::Int(3)),
            (
                "let one = 1; let two = one + one; one + two",
                TestValue::Int(3),
            ),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn string_literals() {
        let cases = vec![
            (
                r#""zero knowledge""#,
                TestValue::Str("zero knowledge".to_string()),
            ),
            (
                r#""zero " + "knowledge""#,
                TestValue::Str("zero knowledge".to_string()),
            ),
            (
                r#""zero " + "knowledge" + " proofs""#,
                TestValue::Str("zero knowledge proofs".to_string()),
            ),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn array_literals() {
        let cases = vec![
            ("[]", TestValue::IntArray(vec![])),
            ("[1, 2, 3]", TestValue::IntArray(vec![1, 2, 3])),
            (
                "[1 + 2, 3 * 4, 5 + 6]",
                TestValue::IntArray(vec![3, 12, 11]),
            ),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn hash_literals() {
        let cases = vec![
            ("{}", TestValue::Hash(vec![])),
            ("{1: 2, 2: 3}", TestValue::Hash(vec![(1, 2), (2, 3)])),
            (
                "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
                TestValue::Hash(vec![(2, 4), (6, 16)]),
            ),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn index_expressions() {
        let cases = vec![
            ("[1, 2, 3][1]", TestValue::Int(2)),
            ("[1, 2, 3][0 + 2]", TestValue::Int(3)),
            ("[[1, 1, 1]][0][0]", TestValue::Int(1)),
            ("[][0]", TestValue::Null),
            ("[1, 2, 3][99]", TestValue::Null),
            ("[1][-1]", TestValue::Null),
            ("{1: 1, 2: 2}[1]", TestValue::Int(1)),
            ("{1: 1, 2: 2}[2]", TestValue::Int(2)),
            ("{1: 1}[0]", TestValue::Null),
            ("{}[0]", TestValue::Null),
        ];

        for (input, expected) in cases {
            run_vm_test(input, expected);
        }
    }

    #[test]
    fn stack_underflow() {
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
