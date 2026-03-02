//! Binary serialization and deserialization for compiled bytecode.
//!
//! Uses [`postcard`] (a `serde`-compatible compact binary format) for the
//! payload, wrapped in a custom header for format identification and version
//! control.
//!
//! # Wire Format
//!
//! ```text
//! Header:  MAGIC "MAAT" (4B) + FORMAT_VERSION u32 BE (4B)
//! Payload: postcard-encoded Bytecode (variable length)
//! ```
//!
//! The header uses fixed big-endian encoding for human-readable hex dumps
//! and consistent identification. The payload delegates entirely to postcard's
//! varint-based encoding, which handles all `Bytecode` fields including the
//! constant pool, source map, and instruction stream.

use maat_errors::SerializationError;

use crate::Bytecode;

/// Magic bytes identifying a Maat bytecode file.
const MAAT_MAGIC: [u8; 4] = *b"MAAT";

/// Current format version. Incremented when the binary layout changes
/// in a backward-incompatible way.
const FORMAT_VERSION: u32 = 1;

/// Header size in bytes (4-byte magic + 4-byte version).
const HEADER_SIZE: usize = 4 + 4;

type Result<T> = std::result::Result<T, SerializationError>;

impl Bytecode {
    /// Serializes this bytecode to a binary representation.
    ///
    /// The output can be written to a `.mtc` file and later restored with
    /// [`deserialize`](Self::deserialize). The format consists of an 8-byte
    /// header (magic + version) followed by a postcard-encoded payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the constant pool contains object types that cannot be represented in
    /// the binary format (e.g., `Builtin` function pointers or tree-walking `Function` objects).
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let payload = postcard::to_allocvec(self)
            .map_err(|e| SerializationError::PostcardEncode(e.to_string()))?;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(&MAAT_MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Deserializes bytecode from a binary representation produced by
    /// [`serialize`](Self::serialize).
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed: invalid magic bytes,
    /// unsupported version, or a corrupted/truncated postcard payload.
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(SerializationError::UnexpectedEof {
                offset: 0,
                needed: HEADER_SIZE,
            });
        }

        if bytes[..4] != MAAT_MAGIC {
            return Err(SerializationError::InvalidMagic);
        }

        let mut version_bytes = [0u8; 4];
        version_bytes.copy_from_slice(&bytes[4..8]);
        let version = u32::from_be_bytes(version_bytes);
        if version != FORMAT_VERSION {
            return Err(SerializationError::UnsupportedVersion(version));
        }

        postcard::from_bytes(&bytes[HEADER_SIZE..])
            .map_err(|e| SerializationError::PostcardDecode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use maat_runtime::{Closure, CompiledFunction, HashObject, Hashable, Object};
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
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn numeric_constants() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![
                Object::Null,
                Object::I8(-42),
                Object::I16(-1000),
                Object::I32(100_000),
                Object::I64(i64::MAX),
                Object::I128(i128::MIN),
                Object::Isize(-1),
                Object::U8(255),
                Object::U16(65535),
                Object::U32(4_000_000_000),
                Object::U64(u64::MAX),
                Object::U128(u128::MAX),
                Object::Usize(42),
                Object::Bool(true),
                Object::Bool(false),
            ],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn string_constant() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![
                Object::Str(String::new()),
                Object::Str("hello, world!".to_owned()),
                Object::Str("\u{1F600}".to_owned()),
            ],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn array_constant() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Object::Array(vec![
                Object::I64(1),
                Object::Str("two".to_owned()),
                Object::Bool(true),
                Object::Array(vec![Object::I64(3), Object::I64(4)]),
            ])],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn hash_constant() {
        let mut pairs = indexmap::IndexMap::new();
        pairs.insert(Hashable::I64(1), Object::Str("one".to_owned()));
        pairs.insert(Hashable::Str("key".to_owned()), Object::Bool(true));

        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Object::Hash(HashObject { pairs })],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn compiled_function_constant() {
        let cf = CompiledFunction {
            instructions: Rc::from(vec![1u8, 2, 3, 4].as_slice()),
            num_locals: 2,
            num_parameters: 1,
            source_map: SourceMap::new(),
        };
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Object::CompiledFunction(cf)],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn closure_constant() {
        let cf = CompiledFunction {
            instructions: Rc::from(vec![10u8, 20].as_slice()),
            num_locals: 3,
            num_parameters: 2,
            source_map: SourceMap::new(),
        };
        let closure = Closure {
            func: cf,
            free_vars: vec![Object::I64(42), Object::Str("captured".to_owned())],
        };
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Object::Closure(closure)],
            source_map: SourceMap::new(),
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn nested_compiled_function() {
        let inner_cf = CompiledFunction {
            instructions: Rc::from(vec![5u8, 6].as_slice()),
            num_locals: 0,
            num_parameters: 0,
            source_map: SourceMap::new(),
        };
        let bc = Bytecode {
            instructions: Instructions::from(vec![0, 0, 1, 2]),
            constants: vec![Object::I64(99), Object::CompiledFunction(inner_cf)],
            source_map: SourceMap::new(),
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
        };
        assert_eq!(roundtrip(&bc), bc);
    }

    #[test]
    fn invalid_magic() {
        let result = Bytecode::deserialize(b"NOPE\x00\x00\x00\x01");
        assert!(matches!(result, Err(SerializationError::InvalidMagic)));
    }

    #[test]
    fn unsupported_version() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MAAT");
        data.extend_from_slice(&99u32.to_be_bytes());
        let result = Bytecode::deserialize(&data);
        assert!(matches!(
            result,
            Err(SerializationError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn truncated_header() {
        let result = Bytecode::deserialize(b"MAA");
        assert!(matches!(
            result,
            Err(SerializationError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn truncated_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(b"MAAT");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.push(0xFF);
        let result = Bytecode::deserialize(&data);
        assert!(result.is_err());
    }

    #[test]
    fn non_serializable_object() {
        let bc = Bytecode {
            instructions: Instructions::new(),
            constants: vec![Object::Builtin(|_| Ok(Object::Null))],
            source_map: SourceMap::new(),
        };
        let result = bc.serialize();
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_identity() {
        let bc = Bytecode {
            instructions: Instructions::from(vec![0, 0, 1, 1, 2]),
            constants: vec![Object::I64(42), Object::Str("test".to_owned())],
            source_map: SourceMap::new(),
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
