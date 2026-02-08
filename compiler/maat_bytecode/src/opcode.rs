/// Bytecode operation codes.
///
/// Each opcode represents a single VM instruction. Opcodes are encoded
/// as single bytes, with optional operands following in big-endian format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    /// Push a constant onto the stack.
    /// Operands: [u16] - index into the constant pool
    Constant = 0,

    /// Add two values from the stack.
    /// Operands: none
    Add = 1,

    /// Pop value from the stack.
    /// Operands: none
    Pop = 2,

    /// Subtract two values from the stack.
    /// Operands: none
    Sub = 3,

    /// Multiply two values from the stack.
    /// Operands: none
    Mul = 4,

    /// Divide two values from the stack.
    /// Operands: none
    Div = 5,

    /// Push `true` onto the stack.
    /// Operands: none
    True = 6,

    /// Push `false` onto the stack.
    /// Operands: none
    False = 7,

    /// Test equality of two stack values.
    /// Operands: none
    Equal = 8,

    /// Test inequality of two stack values.
    /// Operands: none
    NotEqual = 9,

    /// Test if first value is greater than second.
    /// Operands: none
    GreaterThan = 10,

    /// Test if first value is less than second.
    /// Operands: none
    LessThan = 11,

    /// Negate a value (unary minus).
    /// Operands: none
    Minus = 12,

    /// Logical NOT operation.
    /// Operands: none
    Bang = 13,
}

impl Opcode {
    /// Returns the name of this opcode for disassembly.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Constant => "OpConstant",
            Self::Add => "OpAdd",
            Self::Pop => "OpPop",
            Self::Sub => "OpSub",
            Self::Mul => "OpMul",
            Self::Div => "OpDiv",
            Self::True => "OpTrue",
            Self::False => "OpFalse",
            Self::Equal => "OpEqual",
            Self::NotEqual => "OpNotEqual",
            Self::GreaterThan => "OpGreaterThan",
            Self::LessThan => "OpLessThan",
            Self::Minus => "OpMinus",
            Self::Bang => "OpBang",
        }
    }

    /// Returns the operand widths for this opcode.
    ///
    /// Each element in the returned slice represents the byte width
    /// of an operand. For example, `&[2]` means one 2-byte operand.
    #[inline]
    pub const fn operand_widths(self) -> &'static [usize] {
        match self {
            Self::Constant => &[2],
            Self::Add
            | Self::Pop
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::True
            | Self::False
            | Self::Equal
            | Self::NotEqual
            | Self::GreaterThan
            | Self::LessThan
            | Self::Minus
            | Self::Bang => &[],
        }
    }

    /// Attempts to convert a byte to an opcode.
    #[inline]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Constant),
            1 => Some(Self::Add),
            2 => Some(Self::Pop),
            3 => Some(Self::Sub),
            4 => Some(Self::Mul),
            5 => Some(Self::Div),
            6 => Some(Self::True),
            7 => Some(Self::False),
            8 => Some(Self::Equal),
            9 => Some(Self::NotEqual),
            10 => Some(Self::GreaterThan),
            11 => Some(Self::LessThan),
            12 => Some(Self::Minus),
            13 => Some(Self::Bang),
            _ => None,
        }
    }

    /// Converts this opcode to its byte representation.
    #[inline]
    pub const fn to_byte(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_roundtrip() {
        for byte in 0..=13 {
            let opcode = Opcode::from_byte(byte).unwrap();
            assert_eq!(opcode.to_byte(), byte);
        }
    }

    #[test]
    fn opcode_metadata() {
        assert_eq!(Opcode::Constant.name(), "OpConstant");
        assert_eq!(Opcode::Constant.operand_widths(), &[2]);

        assert_eq!(Opcode::Add.name(), "OpAdd");
        assert_eq!(Opcode::Add.operand_widths(), &[]);

        assert_eq!(Opcode::Pop.name(), "OpPop");
        assert_eq!(Opcode::Pop.operand_widths(), &[]);

        assert_eq!(Opcode::Sub.name(), "OpSub");
        assert_eq!(Opcode::Mul.name(), "OpMul");
        assert_eq!(Opcode::Div.name(), "OpDiv");

        assert_eq!(Opcode::True.name(), "OpTrue");
        assert_eq!(Opcode::False.name(), "OpFalse");

        assert_eq!(Opcode::Equal.name(), "OpEqual");
        assert_eq!(Opcode::NotEqual.name(), "OpNotEqual");
        assert_eq!(Opcode::GreaterThan.name(), "OpGreaterThan");

        assert_eq!(Opcode::Minus.name(), "OpMinus");
        assert_eq!(Opcode::Bang.name(), "OpBang");
    }

    #[test]
    fn invalid_opcode() {
        assert_eq!(Opcode::from_byte(255), None);
    }
}
