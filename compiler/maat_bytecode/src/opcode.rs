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

    /// Conditional jump: pop value and jump to target address if not truthy.
    /// Operands: [u16] - jump target address
    CondJump = 14,

    /// Unconditional jump to target address.
    /// Operands: [u16] - jump target address
    Jump = 15,

    /// Push the null value onto the stack.
    /// Operands: none
    Null = 16,

    /// Store a value in a global binding.
    /// Operands: [u16] - global variable index
    SetGlobal = 17,

    /// Load a value from a global binding.
    /// Operands: [u16] - global variable index
    GetGlobal = 18,

    /// Build a vector from the top N stack elements.
    /// Operands: [u16] - number of elements
    Vector = 19,

    /// Build a map from the top N stack elements (key-value pairs).
    /// Operands: [u16] - total number of elements (keys + values)
    Map = 20,

    /// Index into a vector or map. Pops index and container, pushes result.
    /// Operands: none
    Index = 21,

    /// Call a function. Pops function and arguments from the stack.
    /// Operands: [u8] - number of arguments
    Call = 22,

    /// Return from a function with a value on top of the stack.
    /// Operands: none
    ReturnValue = 23,

    /// Return from a function with no explicit return value (implicit null).
    /// Operands: none
    Return = 24,

    /// Load a local binding onto the stack.
    /// Operands: [u8] - local variable index
    GetLocal = 25,

    /// Store a value in a local binding.
    /// Operands: [u8] - local variable index
    SetLocal = 26,

    /// Load a built-in function onto the stack.
    /// Operands: [u8] - builtin function index
    GetBuiltin = 27,

    /// Create a closure from a compiled function and captured free variables.
    /// Operands: [u16, u8] - constant pool index of the function, number of free variables
    Closure = 28,

    /// Load a free variable from the current closure's captured environment.
    /// Operands: [u8] - free variable index
    GetFree = 29,

    /// Push the current closure onto the stack for recursive self-reference.
    /// Operands: none
    CurrentClosure = 30,

    /// Convert a numeric value to a different numeric type.
    /// Operands: [u8] - target type tag (see [`TypeTag`])
    Convert = 31,

    /// Construct a struct or enum variant from the top N stack elements.
    /// Operands: [u16] - type registry index, [u8] - number of fields
    Construct = 32,

    /// Read a field from a struct object on top of the stack.
    /// Operands: [u16] - field index
    GetField = 33,

    /// Read the variant tag from an enum object on top of the stack.
    /// If the tag matches, jump over the operand's target address;
    /// otherwise jump to the target.
    /// Operands: [u16] - expected variant tag, [u16] - jump target on mismatch
    MatchTag = 34,

    /// Compute the remainder of two values from the stack.
    /// Operands: none
    Mod = 35,

    /// Bitwise AND of two values from the stack.
    /// Operands: none
    BitAnd = 36,

    /// Bitwise OR of two values from the stack.
    /// Operands: none
    BitOr = 37,

    /// Bitwise XOR of two values from the stack.
    /// Operands: none
    BitXor = 38,

    /// Left shift of two values from the stack.
    /// Operands: none
    Shl = 39,

    /// Right shift of two values from the stack.
    /// Operands: none
    Shr = 40,

    /// Construct a half-open `Range` from two i64 values on the stack.
    /// Pops `end` then `start`, pushes `Object::Range(start, end)`.
    /// Operands: none
    MakeRange = 41,

    /// Construct an inclusive `RangeInclusive` from two i64 values on the stack.
    /// Pops `end` then `start`, pushes `Object::RangeInclusive(start, end)`.
    /// Operands: none
    MakeRangeInclusive = 42,
}

impl Opcode {
    /// Returns the name of this opcode for disassembly.
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
            Self::CondJump => "OpCondJump",
            Self::Jump => "OpJump",
            Self::Null => "OpNull",
            Self::SetGlobal => "OpSetGlobal",
            Self::GetGlobal => "OpGetGlobal",
            Self::Vector => "OpVector",
            Self::Map => "OpMap",
            Self::Index => "OpIndex",
            Self::Call => "OpCall",
            Self::ReturnValue => "OpReturnValue",
            Self::Return => "OpReturn",
            Self::GetLocal => "OpGetLocal",
            Self::SetLocal => "OpSetLocal",
            Self::GetBuiltin => "OpGetBuiltin",
            Self::Closure => "OpClosure",
            Self::GetFree => "OpGetFree",
            Self::CurrentClosure => "OpCurrentClosure",
            Self::Convert => "OpConvert",
            Self::Construct => "OpConstruct",
            Self::GetField => "OpGetField",
            Self::MatchTag => "OpMatchTag",
            Self::Mod => "OpMod",
            Self::BitAnd => "OpBitAnd",
            Self::BitOr => "OpBitOr",
            Self::BitXor => "OpBitXor",
            Self::Shl => "OpShl",
            Self::Shr => "OpShr",
            Self::MakeRange => "OpMakeRange",
            Self::MakeRangeInclusive => "OpMakeRangeInclusive",
        }
    }

    /// Returns the operand widths for this opcode.
    ///
    /// Each element in the returned slice represents the byte width
    /// of an operand. For example, `&[2]` means one 2-byte operand.
    #[inline]
    pub const fn operand_widths(self) -> &'static [usize] {
        match self {
            Self::Constant
            | Self::CondJump
            | Self::Jump
            | Self::SetGlobal
            | Self::GetGlobal
            | Self::Vector
            | Self::Map
            | Self::GetField => &[2],
            Self::Closure | Self::Construct => &[2, 1],
            Self::MatchTag => &[2, 2],
            Self::Call
            | Self::GetLocal
            | Self::SetLocal
            | Self::GetBuiltin
            | Self::GetFree
            | Self::Convert => &[1],
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
            | Self::Bang
            | Self::Null
            | Self::Index
            | Self::ReturnValue
            | Self::Return
            | Self::CurrentClosure
            | Self::Mod
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor
            | Self::Shl
            | Self::Shr
            | Self::MakeRange
            | Self::MakeRangeInclusive => &[],
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
            14 => Some(Self::CondJump),
            15 => Some(Self::Jump),
            16 => Some(Self::Null),
            17 => Some(Self::SetGlobal),
            18 => Some(Self::GetGlobal),
            19 => Some(Self::Vector),
            20 => Some(Self::Map),
            21 => Some(Self::Index),
            22 => Some(Self::Call),
            23 => Some(Self::ReturnValue),
            24 => Some(Self::Return),
            25 => Some(Self::GetLocal),
            26 => Some(Self::SetLocal),
            27 => Some(Self::GetBuiltin),
            28 => Some(Self::Closure),
            29 => Some(Self::GetFree),
            30 => Some(Self::CurrentClosure),
            31 => Some(Self::Convert),
            32 => Some(Self::Construct),
            33 => Some(Self::GetField),
            34 => Some(Self::MatchTag),
            35 => Some(Self::Mod),
            36 => Some(Self::BitAnd),
            37 => Some(Self::BitOr),
            38 => Some(Self::BitXor),
            39 => Some(Self::Shl),
            40 => Some(Self::Shr),
            41 => Some(Self::MakeRange),
            42 => Some(Self::MakeRangeInclusive),
            _ => None,
        }
    }

    /// Converts this opcode to its byte representation.
    #[inline]
    pub const fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Numeric type tags for the `OpConvert` instruction operand.
///
/// Each variant corresponds to a runtime numeric type and is encoded
/// as a single byte in the instruction stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TypeTag {
    I8 = 0,
    I16 = 1,
    I32 = 2,
    I64 = 3,
    I128 = 4,
    Isize = 5,
    U8 = 6,
    U16 = 7,
    U32 = 8,
    U64 = 9,
    U128 = 10,
    Usize = 11,
}

impl TypeTag {
    /// Converts a byte to a type tag.
    #[inline]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::I8),
            1 => Some(Self::I16),
            2 => Some(Self::I32),
            3 => Some(Self::I64),
            4 => Some(Self::I128),
            5 => Some(Self::Isize),
            6 => Some(Self::U8),
            7 => Some(Self::U16),
            8 => Some(Self::U32),
            9 => Some(Self::U64),
            10 => Some(Self::U128),
            11 => Some(Self::Usize),
            _ => None,
        }
    }

    /// Converts this type tag to its byte representation.
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
        for byte in 0..=42 {
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
