//! Utilities for STARK proof generation and verification.

pub mod proof_serializer {
    //! Proof serialization and deserialization.
    //!
    //! Wire format (version 6):
    //!
    //! ```text
    //! PROOF_MAGIC:        b"MATP"       (4 bytes)
    //! PROOF_VERSION:      u16 BE        (2 bytes, currently 6)
    //! OUTPUT:             u64 LE        (8 bytes, claimed program output)
    //! INPUT_COUNT:        u16 BE        (2 bytes, number of public inputs)
    //! INPUTS:             [u64; N] LE   (8 * N bytes, public input values)
    //! OUTPUT_BASE:        u32 BE        (4 bytes, flat base of the output segment)
    //! OUTPUT_SEG_LEN:     u32 BE        (4 bytes, number of public-output cells)
    //! OUTPUT_SEG:         [u64; L] LE   (8 * L bytes, public-output cell values)
    //! PROGRAM_BASE:       u32 BE        (4 bytes, flat base of the program segment)
    //! PROGRAM_SEG_LEN:    u32 BE        (4 bytes, number of program-memory cells)
    //! PROGRAM_SEG:        [u64; P] LE   (8 * P bytes, program-memory cell values)
    //! PAYLOAD:            Winterfell    (variable, Winterfell's native Proof encoding)
    //! ```
    //!
    //! Minimum header (zero inputs, zero output cells, zero program cells): 32 bytes.

    use maat_air::Proof;
    use maat_errors::SerializationError;
    use maat_field::BaseElement;
    use maat_trace::{PublicMemory, PublicSegment};

    const PROOF_MAGIC: [u8; 4] = *b"MATP";
    const PROOF_VERSION: u16 = 6;
    // Minimum header size with zero inputs / output cells / program cells:
    // 4 (magic) + 2 (version) + 8 (output) + 2 (input count) + 4 (input_base)
    //   + 4 (output_base) + 4 (output_seg_len) + 4 (program_base) + 4 (program_seg_len).
    const MIN_HEADER_SIZE: usize = 36;
    const MAX_INPUT_COUNT: usize = 1024;
    const MAX_PUBLIC_SEGMENT_CELLS: usize = 1 << 20;

    #[derive(Debug, Clone)]
    pub struct ProofPublicInputs {
        pub output: BaseElement,
        pub memory: PublicMemory,
    }

    pub fn serialize_proof(proof: &Proof, output: BaseElement, memory: &PublicMemory) -> Vec<u8> {
        let inputs = &memory.input.cells;
        let output_segment = &memory.output.cells;
        let program_segment = &memory.program.cells;
        let payload = proof.to_bytes();
        let input_count = inputs.len() as u16;
        let total_size = MIN_HEADER_SIZE
            + inputs.len() * 8
            + output_segment.len() * 8
            + program_segment.len() * 8
            + payload.len();

        let mut buf = Vec::with_capacity(total_size);
        buf.extend_from_slice(&PROOF_MAGIC);
        buf.extend_from_slice(&PROOF_VERSION.to_be_bytes());
        buf.extend_from_slice(&output.as_int().to_le_bytes());
        buf.extend_from_slice(&input_count.to_be_bytes());
        for input in inputs {
            buf.extend_from_slice(&input.as_int().to_le_bytes());
        }
        buf.extend_from_slice(&memory.input.base.to_be_bytes());
        buf.extend_from_slice(&memory.output.base.to_be_bytes());
        buf.extend_from_slice(&(output_segment.len() as u32).to_be_bytes());
        for cell in output_segment {
            buf.extend_from_slice(&cell.as_int().to_le_bytes());
        }
        buf.extend_from_slice(&memory.program.base.to_be_bytes());
        buf.extend_from_slice(&(program_segment.len() as u32).to_be_bytes());
        for cell in program_segment {
            buf.extend_from_slice(&cell.as_int().to_le_bytes());
        }
        buf.extend_from_slice(&payload);
        buf
    }

    pub fn deserialize_proof(
        bytes: &[u8],
    ) -> Result<(Proof, ProofPublicInputs), SerializationError> {
        let mut cursor = 0usize;
        let magic = read_slice(bytes, &mut cursor, 4)?;
        if magic != PROOF_MAGIC {
            return Err(SerializationError::InvalidMagic { expected: "MATP" });
        }
        let version_bytes = read_slice(bytes, &mut cursor, 2)?;
        let version = u16::from_be_bytes([version_bytes[0], version_bytes[1]]);
        if version != PROOF_VERSION {
            return Err(SerializationError::UnsupportedVersion(version as u64));
        }

        let output_bytes = read_slice(bytes, &mut cursor, 8)?;
        let output = BaseElement::new(u64::from_le_bytes(output_bytes.try_into().unwrap()));

        let input_count_bytes = read_slice(bytes, &mut cursor, 2)?;
        let input_count = u16::from_be_bytes([input_count_bytes[0], input_count_bytes[1]]) as usize;
        if input_count > MAX_INPUT_COUNT {
            return Err(SerializationError::ResourceLimitExceeded {
                field: "input_count",
                size: input_count,
                limit: MAX_INPUT_COUNT,
            });
        }
        let inputs = read_felt_vec(bytes, &mut cursor, input_count)?;

        let input_base = read_u32_be(bytes, &mut cursor)?;
        let output_base = read_u32_be(bytes, &mut cursor)?;
        let output_seg_len = read_u32_be(bytes, &mut cursor)? as usize;
        check_segment_limit("output_segment_cells", output_seg_len)?;
        let output_segment = read_felt_vec(bytes, &mut cursor, output_seg_len)?;

        let program_base = read_u32_be(bytes, &mut cursor)?;
        let program_seg_len = read_u32_be(bytes, &mut cursor)? as usize;
        check_segment_limit("program_segment_cells", program_seg_len)?;
        let program_segment = read_felt_vec(bytes, &mut cursor, program_seg_len)?;

        let payload = &bytes[cursor..];
        let proof = std::panic::catch_unwind(|| Proof::from_bytes(payload))
            .map_err(|_| {
                SerializationError::WinterfellDecode(
                    "proof payload triggered a panic during deserialization \
                     (likely malformed arithmetic parameters)"
                        .into(),
                )
            })?
            .map_err(|e| SerializationError::WinterfellDecode(e.to_string()))?;

        let public_inputs = ProofPublicInputs {
            output,
            memory: PublicMemory {
                input: PublicSegment::new(input_base, inputs),
                output: PublicSegment::new(output_base, output_segment),
                program: PublicSegment::new(program_base, program_segment),
            },
        };

        Ok((proof, public_inputs))
    }

    fn read_slice<'a>(
        bytes: &'a [u8],
        cursor: &mut usize,
        len: usize,
    ) -> Result<&'a [u8], SerializationError> {
        if bytes.len() < cursor.saturating_add(len) {
            return Err(SerializationError::UnexpectedEof {
                offset: *cursor,
                needed: len,
            });
        }
        let slice = &bytes[*cursor..*cursor + len];
        *cursor += len;
        Ok(slice)
    }

    fn read_u32_be(bytes: &[u8], cursor: &mut usize) -> Result<u32, SerializationError> {
        let slice = read_slice(bytes, cursor, 4)?;
        Ok(u32::from_be_bytes(slice.try_into().unwrap()))
    }

    fn read_felt_vec(
        bytes: &[u8],
        cursor: &mut usize,
        count: usize,
    ) -> Result<Vec<BaseElement>, SerializationError> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let slice = read_slice(bytes, cursor, 8)?;
            out.push(BaseElement::new(u64::from_le_bytes(
                slice.try_into().unwrap(),
            )));
        }
        Ok(out)
    }

    fn check_segment_limit(field: &'static str, size: usize) -> Result<(), SerializationError> {
        if size > MAX_PUBLIC_SEGMENT_CELLS {
            return Err(SerializationError::ResourceLimitExceeded {
                field,
                size,
                limit: MAX_PUBLIC_SEGMENT_CELLS,
            });
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn invalid_magic_rejected() {
            let bytes = b"NOTZ_rest_of_data_here_padding__extra_more_padding_needed";
            let err = deserialize_proof(bytes).unwrap_err();
            assert!(matches!(
                err,
                SerializationError::InvalidMagic { expected: "MATP" }
            ));
        }

        #[test]
        fn unsupported_version_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&99u16.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 26]); // pad past min header
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(err, SerializationError::UnsupportedVersion(99)));
        }

        #[test]
        fn truncated_header_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&PROOF_VERSION.to_be_bytes());
            bytes.extend_from_slice(&[0u8; 8]); // partial header
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(err, SerializationError::UnexpectedEof { .. }));
        }

        #[test]
        fn excessive_input_count_rejected() {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&PROOF_MAGIC);
            bytes.extend_from_slice(&PROOF_VERSION.to_be_bytes());
            bytes.extend_from_slice(&0u64.to_le_bytes()); // output
            bytes.extend_from_slice(&10_000u16.to_be_bytes()); // input count
            bytes.extend_from_slice(&[0u8; 16]); // rest of min header
            let err = deserialize_proof(&bytes).unwrap_err();
            assert!(matches!(
                err,
                SerializationError::ResourceLimitExceeded {
                    field: "input_count",
                    ..
                }
            ));
        }
    }
}
