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

    /// Returns the last value popped from the stack.
    ///
    /// After execution completes, the top-level expression result sits
    /// at the stack position just below `sp` (the slot vacated by the
    /// final `OpPop`). This method retrieves that value without
    /// consuming it.
    pub fn last_popped_stack_elem(&self) -> Option<&Object> {
        if self.sp < self.stack.len() {
            Some(&self.stack[self.sp])
        } else {
            None
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
                    let array = self.build_array(num_elements)?;
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
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack contains fewer elements than requested.
    fn build_array(&mut self, num_elements: usize) -> Result<Object> {
        if num_elements > self.sp {
            return Err(VmError::new(format!(
                "stack underflow in array construction: need {num_elements} elements, stack has {}",
                self.sp
            ))
            .into());
        }
        let start = self.sp - num_elements;
        let elements = self.stack[start..self.sp].to_vec();
        self.sp = start;
        Ok(Object::Array(elements))
    }

    /// Builds a hash object from the top `num_elements` stack values.
    ///
    /// Elements are expected in alternating key-value order on the stack.
    ///
    /// # Errors
    ///
    /// Returns `VmError` if the stack contains fewer elements than requested,
    /// or if a key is not a hashable type.
    fn build_hash(&mut self, num_elements: usize) -> Result<Object> {
        if num_elements > self.sp {
            return Err(VmError::new(format!(
                "stack underflow in hash construction: need {num_elements} elements, stack has {}",
                self.sp
            ))
            .into());
        }
        let start = self.sp - num_elements;
        let mut pairs = HashMap::with_capacity(num_elements / 2);

        for i in (start..self.sp).step_by(2) {
            let key = self.stack[i].clone();
            let value = self.stack[i + 1].clone();
            let key = Hashable::try_from(key).map_err(|e| VmError::new(e.to_string()))?;
            pairs.insert(key, value);
        }

        self.sp = start;
        Ok(Object::Hash(HashObject { pairs }))
    }

    /// Dispatches index operations to the appropriate handler.
    fn execute_index_expression(&mut self, container: Object, index: Object) -> Result<()> {
        match (&container, &index) {
            (Object::Array(elements), _) => self.execute_array_index(elements, &index),
            (Object::Hash(hash), _) => self.execute_hash_index(hash, index),
            _ => Err(VmError::new(format!(
                "index operator not supported: {}",
                container.type_name()
            ))
            .into()),
        }
    }

    /// Indexes into an array with bounds checking.
    ///
    /// Converts the index to `usize` via `TryFrom`. Negative values and
    /// out-of-bounds indices produce `Null`. Non-integer index types return an error.
    fn execute_array_index(&mut self, elements: &[Object], index: &Object) -> Result<()> {
        let idx = match index {
            Object::I64(v) => match usize::try_from(*v) {
                Ok(i) => i,
                Err(_) => return self.push_stack(NULL),
            },
            _ => {
                return Err(VmError::new(format!(
                    "array index must be an integer, got {}",
                    index.type_name()
                ))
                .into());
            }
        };

        if idx >= elements.len() {
            return self.push_stack(NULL);
        }
        self.push_stack(elements[idx].clone())
    }

    /// Indexes into a hash by key.
    fn execute_hash_index(&mut self, hash: &HashObject, index: Object) -> Result<()> {
        let key = Hashable::try_from(index).map_err(|e| VmError::new(e.to_string()))?;

        match hash.pairs.get(&key) {
            Some(value) => self.push_stack(value.clone()),
            None => self.push_stack(NULL),
        }
    }
}
