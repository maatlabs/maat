//! Binary serialization and deserialization for compiled bytecode.
//!
//! Uses [`postcard`] for the payload, wrapped in a custom header for
//! format identification and version control.
//!
//! # Wire Format
//!
//! ```text
//! Header:  BYTECODE_MAGIC "MATC" (4B) + FORMAT_VERSION u32 BE (4B)
//! Payload: postcard-encoded Bytecode (variable length)
//! ```

use maat_errors::SerializationError;

use crate::{Bytecode, MAX_CONSTANT_POOL_SIZE};

const BYTECODE_MAGIC: [u8; 4] = *b"MATC";
const BYTECODE_VERSION: u32 = 1;
const HEADER_SIZE: usize = 4 + 4;
const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;
const MAX_INSTRUCTION_COUNT: usize = 1_000_000;

type Result<T> = std::result::Result<T, SerializationError>;

impl Bytecode {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let payload = postcard::to_allocvec(self)
            .map_err(|e| SerializationError::PostcardEncode(e.to_string()))?;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(&BYTECODE_MAGIC);
        out.extend_from_slice(&BYTECODE_VERSION.to_be_bytes());
        out.extend_from_slice(&payload);
        Ok(out)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(SerializationError::UnexpectedEof {
                offset: 0,
                needed: HEADER_SIZE,
            });
        }
        if bytes[..4] != BYTECODE_MAGIC {
            return Err(SerializationError::InvalidMagic { expected: "MATC" });
        }
        let mut version_bytes = [0u8; 4];
        version_bytes.copy_from_slice(&bytes[4..8]);
        let version = u32::from_be_bytes(version_bytes);
        if version != BYTECODE_VERSION {
            return Err(SerializationError::UnsupportedVersion(version as u64));
        }
        let payload = &bytes[HEADER_SIZE..];
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(SerializationError::PayloadTooLarge {
                size: payload.len(),
                limit: MAX_PAYLOAD_SIZE,
            });
        }
        let bc: Self = postcard::from_bytes(payload)
            .map_err(|e| SerializationError::PostcardDecode(e.to_string()))?;
        if bc.constants.len() > MAX_CONSTANT_POOL_SIZE {
            return Err(SerializationError::ResourceLimitExceeded {
                field: "constant pool",
                size: bc.constants.len(),
                limit: MAX_CONSTANT_POOL_SIZE,
            });
        }
        if bc.instructions.len() > MAX_INSTRUCTION_COUNT {
            return Err(SerializationError::ResourceLimitExceeded {
                field: "instruction stream",
                size: bc.instructions.len(),
                limit: MAX_INSTRUCTION_COUNT,
            });
        }
        Ok(bc)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use maat_runtime::{Closure, CompiledFn, Hashable, Integer, Map, Value};
    use maat_span::{SourceMap, Span};

    use super::*;
    use crate::Instructions;

    fn roundtrip(bytecode: &Bytecode) -> Bytecode {
        let bytes = bytecode.serialize().expect("serialize failed");
        Bytecode::deserialize(&bytes).expect("deserialize failed")
    }

    #[test]
    fn empty_bytecode() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn numeric_constants() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![
                Value::Unit,
                Value::Integer(Integer::I8(-42)),
                Value::Integer(Integer::I16(-1000)),
                Value::Integer(Integer::I32(100_000)),
                Value::Integer(Integer::I64(i64::MAX)),
                Value::Integer(Integer::I128(i128::MIN)),
                Value::Integer(Integer::Isize(-1)),
                Value::Integer(Integer::U8(255)),
                Value::Integer(Integer::U16(65535)),
                Value::Integer(Integer::U32(4_000_000_000)),
                Value::Integer(Integer::U64(u64::MAX)),
                Value::Integer(Integer::U128(u128::MAX)),
                Value::Integer(Integer::Usize(42)),
                Value::Bool(true),
                Value::Bool(false),
            ],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn string_constant() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![
                Value::Str(String::new()),
                Value::Str("hello, world!".to_owned()),
                Value::Str("\u{1F600}".to_owned()),
            ],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn array_constant() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Value::Vector(vec![
                Value::Integer(Integer::I64(1)),
                Value::Str("two".to_owned()),
                Value::Bool(true),
                Value::Vector(vec![
                    Value::Integer(Integer::I64(3)),
                    Value::Integer(Integer::I64(4)),
                ]),
            ])],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn map_constant() {
        let mut pairs = indexmap::IndexMap::new();
        pairs.insert(
            Hashable::Integer(Integer::I64(1)),
            Value::Str("one".to_owned()),
        );
        pairs.insert(Hashable::Str("key".to_owned()), Value::Bool(true));

        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Value::Map(Map { pairs })],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn compiled_function_constant() {
        let cf = CompiledFn {
            instructions: Rc::from(vec![1u8, 2, 3, 4].as_slice()),
            num_locals: 2,
            num_parameters: 1,
            source_map: SourceMap::new(),
        };
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Value::CompiledFn(cf)],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn closure_constant() {
        let cf = CompiledFn {
            instructions: Rc::from(vec![10u8, 20].as_slice()),
            num_locals: 3,
            num_parameters: 2,
            source_map: SourceMap::new(),
        };
        let closure = Closure {
            func: cf,
            free_vars: vec![
                Value::Integer(Integer::I64(42)),
                Value::Str("captured".to_owned()),
            ],
        };
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Value::Closure(closure)],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn nested_compiled_function() {
        let inner_cf = CompiledFn {
            instructions: Rc::from(vec![5u8, 6].as_slice()),
            num_locals: 0,
            num_parameters: 0,
            source_map: SourceMap::new(),
        };
        let bc = Bytecode {
            instructions: Instructions::from(vec![0, 0, 1, 2]),
            constants: vec![
                Value::Integer(Integer::I64(99)),
                Value::CompiledFn(inner_cf),
            ],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn source_map_roundtrip() {
        let mut sm = SourceMap::new();
        sm.add(0, Span::new(0, 5));
        sm.add(3, Span::new(10, 20));
        sm.add(10, Span::new(30, 50));

        let bc = Bytecode {
            instructions: Instructions::from(vec![1, 2, 3]),
            constants: vec![],
            source_map: sm,
            type_registry: vec![],
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn invalid_magic() {
        let result = Bytecode::deserialize(b"NOPE\x00\x00\x00\x01");
        assert!(matches!(
            result,
            Err(SerializationError::InvalidMagic { expected: "MATC" })
        ));
    }

    #[test]
    fn unsupported_version() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MATC");
        data.extend_from_slice(&99u32.to_be_bytes());
        let result = Bytecode::deserialize(&data);
        assert!(matches!(
            result,
            Err(SerializationError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn truncated_header() {
        let result = Bytecode::deserialize(b"MAT");
        assert!(matches!(
            result,
            Err(SerializationError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn truncated_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MATC");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.push(0xFF);
        let result = Bytecode::deserialize(&data);
        assert!(result.is_err());
    }

    #[test]
    fn non_serializable_object() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Value::Builtin(|_| Ok(Value::Unit))],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        let result = bc.serialize();
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_identity() {
        let bc = Bytecode {
            instructions: Instructions::from(vec![0, 0, 1, 1, 2]),
            constants: vec![
                Value::Integer(Integer::I64(42)),
                Value::Str("test".to_owned()),
            ],
            source_map: SourceMap::new(),
            type_registry: vec![],
        };
        let bytes = bc.serialize().expect("serialize failed");
        let restored = Bytecode::deserialize(&bytes).expect("deserialize failed");
        let bytes2 = restored.serialize().expect("re-serialize failed");
        assert_eq!(
            bytes, bytes2,
            "serialize -> deserialize -> serialize must be identical"
        );
    }
}
