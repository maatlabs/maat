use std::fmt;

use maat_errors::DecodeError;

use crate::Opcode;

/// A sequence of bytecode instructions.
///
/// Instructions are represented as a byte vector where each instruction
/// consists of an opcode byte followed by zero or more operand bytes.
/// All multi-byte operands are encoded in big-endian format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Instructions(Vec<u8>);

impl Instructions {
    /// Creates an empty instruction sequence.
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates an instruction sequence from a byte vector.
    #[inline]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Returns a reference to the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Returns the number of bytes in this instruction sequence.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if this instruction sequence is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Appends another instruction sequence to this one.
    pub fn extend(&mut self, other: &Self) {
        self.0.extend_from_slice(&other.0);
    }
}

impl From<Vec<u8>> for Instructions {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<Instructions> for Vec<u8> {
    fn from(ins: Instructions) -> Self {
        ins.0
    }
}

impl AsRef<[u8]> for Instructions {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Instructions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ip = 0;

        while ip < self.0.len() {
            let opcode = match Opcode::from_byte(self.0[ip]) {
                Some(op) => op,
                None => {
                    writeln!(f, "ERROR: unknown opcode {}", self.0[ip])?;
                    ip += 1;
                    continue;
                }
            };

            let widths = opcode.operand_widths();
            let (operands, bytes_read) = match decode_operands(widths, &self.0[ip + 1..]) {
                Ok(result) => result,
                Err(e) => {
                    writeln!(f, "ERROR: failed to decode operands at offset {ip}: {e}")?;
                    ip += 1;
                    continue;
                }
            };

            write!(f, "{:04} {}", ip, opcode.name())?;
            for operand in operands {
                write!(f, " {operand}")?;
            }
            writeln!(f)?;

            ip += 1 + bytes_read;
        }

        Ok(())
    }
}

/// Encodes a bytecode instruction from an opcode and operands.
///
/// All multi-byte operands are encoded in big-endian format.
///
/// # Parameters
///
/// * `opcode` - The operation code for the instruction
/// * `operands` - Slice of operand values (e.g., constant pool indices)
///
/// # Returns
///
/// A byte vector containing the opcode byte followed by encoded operand bytes.
///
/// # Examples
///
/// ```
/// use maat_bytecode::{Opcode, encode};
///
/// // Encode OpConstant instruction referencing constant pool index 65534
/// let instruction = encode(Opcode::Constant, &[65534]);
/// assert_eq!(instruction, vec![0, 255, 254]);
///
/// // Encode OpAdd instruction with no operands
/// let instruction = encode(Opcode::Add, &[]);
/// assert_eq!(instruction, vec![1]);
/// ```
pub fn encode(opcode: Opcode, operands: &[usize]) -> Vec<u8> {
    let widths = opcode.operand_widths();

    let inst_len = 1 + widths.iter().sum::<usize>();
    let mut instruction = vec![opcode.to_byte()];
    instruction.reserve(inst_len - 1);

    for (&operand, &width) in operands.iter().zip(widths.iter()) {
        encode_operand_bytes(&mut instruction, operand, width);
    }

    instruction
}

/// Decodes operands from an instruction byte slice.
///
/// All multi-byte operands are decoded from big-endian format.
///
/// # Parameters
///
/// * `widths` - Slice of operand widths in bytes
/// * `bytes` - The instruction bytes starting after the opcode byte
///
/// # Returns
///
/// A tuple of (decoded operands, total bytes read).
///
/// # Errors
///
/// Returns `DecodeError` if the bytecode is malformed (truncated operands or
/// unsupported operand widths).
///
/// # Examples
///
/// ```
/// use maat_bytecode::{Opcode, encode, decode_operands};
///
/// let instruction = encode(Opcode::Constant, &[65535]);
/// let widths = Opcode::Constant.operand_widths();
/// let (operands, bytes_read) = decode_operands(widths, &instruction[1..]).unwrap();
///
/// assert_eq!(operands, vec![65535]);
/// assert_eq!(bytes_read, 2);
/// ```
pub fn decode_operands(widths: &[usize], bytes: &[u8]) -> Result<(Vec<usize>, usize), DecodeError> {
    let mut operands = Vec::with_capacity(widths.len());
    let mut offset = 0;

    for &width in widths {
        let operand = decode_operand_bytes(bytes, offset, width)?;
        operands.push(operand);
        offset += width;
    }

    Ok((operands, offset))
}

/// Encodes an operand value into the instruction buffer.
///
/// # Panics
///
/// Panics if an unsupported operand width is encountered (not 1, 2, 4, or 8 bytes).
#[inline]
fn encode_operand_bytes(instruction: &mut Vec<u8>, operand: usize, width: usize) {
    match width {
        1 => instruction.push(operand as u8),
        2 => instruction.extend_from_slice(&(operand as u16).to_be_bytes()),
        4 => instruction.extend_from_slice(&(operand as u32).to_be_bytes()),
        8 => instruction.extend_from_slice(&(operand as u64).to_be_bytes()),
        _ => panic!("unsupported operand width: {width}"),
    }
}

/// Decodes an operand value from instruction bytes.
///
/// # Errors
///
/// Returns `DecodeError::UnexpectedEndOfBytecode` if there are not enough bytes
/// available at the given offset for the specified width.
///
/// Returns `DecodeError::UnsupportedOperandWidth` if the width is not 1, 2, 4, or 8 bytes.
#[inline]
fn decode_operand_bytes(bytes: &[u8], offset: usize, width: usize) -> Result<usize, DecodeError> {
    let end = offset.saturating_add(width);
    if end > bytes.len() {
        return Err(DecodeError::UnexpectedEndOfBytecode {
            offset,
            needed: width,
            available: bytes.len().saturating_sub(offset),
        });
    }

    match width {
        1 => Ok(bytes[offset] as usize),
        2 => Ok(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize),
        4 => Ok(u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize),
        8 => Ok(u64::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize),
        _ => Err(DecodeError::UnsupportedOperandWidth(width)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_instruction() {
        let cases = vec![
            (Opcode::Constant, vec![65534], vec![0, 255, 254]),
            (Opcode::Add, vec![], vec![1]),
        ];

        for (opcode, operands, expected) in cases {
            let instruction = encode(opcode, &operands);
            assert_eq!(instruction, expected);
        }
    }

    #[test]
    fn instructions_display() {
        let instructions = vec![
            encode(Opcode::Add, &[]),
            encode(Opcode::Constant, &[2]),
            encode(Opcode::Constant, &[65535]),
        ];

        let mut bytecode = Instructions::new();
        for ins in instructions {
            bytecode.extend(&Instructions::from(ins));
        }

        let expected = "0000 OpAdd\n0001 OpConstant 2\n0004 OpConstant 65535\n";
        assert_eq!(bytecode.to_string(), expected);
    }

    #[test]
    fn test_decode_operands() {
        let cases = vec![(Opcode::Constant, vec![65535], 2)];

        for (opcode, expected_operands, expected_bytes_read) in cases {
            let instruction = encode(opcode, &expected_operands);
            let widths = opcode.operand_widths();
            let (operands, bytes_read) = decode_operands(widths, &instruction[1..])
                .expect("decode should succeed for valid bytecode");

            assert_eq!(operands, expected_operands);
            assert_eq!(bytes_read, expected_bytes_read);
        }
    }

    #[test]
    fn decode_truncated_bytecode() {
        // OpConstant requires 2 bytes for its operand, but we only provide 1
        let truncated = [Opcode::Constant.to_byte(), 0x00];
        let widths = Opcode::Constant.operand_widths();

        let result = decode_operands(widths, &truncated[1..]);
        assert!(result.is_err(), "should fail on truncated bytecode");

        match result.unwrap_err() {
            DecodeError::UnexpectedEndOfBytecode {
                offset,
                needed,
                available,
            } => {
                assert_eq!(offset, 0);
                assert_eq!(needed, 2);
                assert_eq!(available, 1);
            }
            other => panic!("expected UnexpectedEndOfBytecode, got {other:?}"),
        }
    }

    #[test]
    fn decode_empty_bytecode() {
        // Try to decode operands from empty bytes
        let empty: &[u8] = &[];
        let widths = Opcode::Constant.operand_widths();

        let result = decode_operands(widths, empty);
        assert!(result.is_err(), "should fail on empty bytecode");

        match result.unwrap_err() {
            DecodeError::UnexpectedEndOfBytecode {
                offset,
                needed,
                available,
            } => {
                assert_eq!(offset, 0);
                assert_eq!(needed, 2);
                assert_eq!(available, 0);
            }
            other => panic!("expected UnexpectedEndOfBytecode, got {other:?}"),
        }
    }
}
