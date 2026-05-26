//! Diluted-form encoding primitive for the LogUp-backed bitwise builtin.

use maat_field::{BaseElement, FieldElement};

use super::logup::TableId;

/// Number of bits per native chunk.
pub const NATIVE_BITS: usize = 8;

/// Spread stride: each native bit gets `STRIDE` positions in the diluted form.
pub const STRIDE: usize = 4;

/// Width (in bits) of a diluted chunk: `(NATIVE_BITS - 1) * STRIDE + 1 = 13`.
pub const DILUTED_BITS: usize = (NATIVE_BITS - 1) * STRIDE + 1;

/// Number of valid diluted values: `2^NATIVE_BITS = 16`.
pub const POOL_SIZE: usize = 1 << NATIVE_BITS;

/// Number of 4-bit chunks per 64-bit operand.
pub const CHUNKS_PER_OPERAND: usize = 64 / NATIVE_BITS;

/// Mask selecting only the diluted bit positions in a 64-bit word.
pub const SPREAD_MASK: u64 = {
    let mut m = 0u64;
    let mut i = 0usize;
    while i < NATIVE_BITS {
        m |= 1u64 << (i * STRIDE);
        i += 1;
    }
    m
};

pub const POOL_TABLE_ID: TableId = 1;

/// Encodes a 4-bit native value `x` (`0..16`) as a 13-bit diluted value.
pub const fn dilute(x: u64) -> u64 {
    let mut out = 0u64;
    let mut i = 0usize;
    while i < NATIVE_BITS {
        out |= ((x >> i) & 1) << (i * STRIDE);
        i += 1;
    }
    out
}

/// Decodes a diluted value back to its 4-bit native form by harvesting bits
/// at the spread positions.
pub const fn undilute(d: u64) -> u64 {
    let mut out = 0u64;
    let mut i = 0usize;
    while i < NATIVE_BITS {
        out |= ((d >> (i * STRIDE)) & 1) << i;
        i += 1;
    }
    out
}

/// Returns the 16-entry pool of valid diluted values as base-field elements.
pub fn pool_entries() -> [BaseElement; POOL_SIZE] {
    let mut out = [BaseElement::ZERO; POOL_SIZE];
    let mut i = 0usize;
    while i < POOL_SIZE {
        out[i] = BaseElement::new(dilute(i as u64));
        i += 1;
    }
    out
}

/// Returns `true` iff `d` is one of the 16 valid diluted values.
pub fn is_in_pool(d: u64) -> bool {
    d & !SPREAD_MASK == 0
}

/// Returns the weight of chunk position `k` for operand reconstruction.
pub const fn chunk_weight(k: usize) -> u64 {
    1u64 << (k * NATIVE_BITS)
}

/// Decomposes a 64-bit operand into `CHUNKS_PER_OPERAND` 4-bit chunks
/// in little-endian order.
pub fn chunk_decompose(operand: u64) -> [u64; CHUNKS_PER_OPERAND] {
    let mask = (1u64 << NATIVE_BITS) - 1;
    std::array::from_fn(|k| (operand >> (k * NATIVE_BITS)) & mask)
}

/// Reconstructs a 64-bit operand from its 4-bit chunk decomposition.
pub fn chunk_recompose(chunks: &[u64; CHUNKS_PER_OPERAND]) -> u64 {
    chunks
        .iter()
        .enumerate()
        .fold(0u64, |acc, (k, &c)| acc | (c << (k * NATIVE_BITS)))
}

/// Per-chunk diluted witness for one bitwise operation pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkBitwiseWitness {
    pub d_a: BaseElement,
    pub d_b: BaseElement,
    pub d_and: BaseElement,
    pub d_or: BaseElement,
    pub d_xor: BaseElement,
}

/// Builds the diluted witness for a 4-bit native chunk pair `(a, b)`.
pub fn chunk_witness(a: u64, b: u64) -> ChunkBitwiseWitness {
    debug_assert!(a < (1u64 << NATIVE_BITS), "a out of native range");
    debug_assert!(b < (1u64 << NATIVE_BITS), "b out of native range");
    ChunkBitwiseWitness {
        d_a: BaseElement::new(dilute(a)),
        d_b: BaseElement::new(dilute(b)),
        d_and: BaseElement::new(dilute(a & b)),
        d_or: BaseElement::new(dilute(a | b)),
        d_xor: BaseElement::new(dilute(a ^ b)),
    }
}

/// Returns the two diluted-form bitwise identity residuals as field
/// elements: `[r_or, r_xor]` where a satisfied witness yields zeros.
pub fn bitwise_identity_residuals<E: FieldElement>(
    d_a: E,
    d_b: E,
    d_and: E,
    d_or: E,
    d_xor: E,
) -> [E; 2] {
    let two = E::ONE + E::ONE;
    let r_or = d_or - (d_a + d_b - d_and);
    let r_xor = d_xor - (d_a + d_b - two * d_and);
    [r_or, r_xor]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::LogUpBuiltin;

    type F = BaseElement;

    fn alpha() -> F {
        F::new(7919)
    }

    #[test]
    fn pool_size_and_chunk_constants_consistent() {
        assert_eq!(NATIVE_BITS, 8);
        assert_eq!(STRIDE, 4);
        assert_eq!(POOL_SIZE, 256);
        assert_eq!(CHUNKS_PER_OPERAND, 8);
        assert_eq!(DILUTED_BITS, 29);
        assert_eq!(SPREAD_MASK, 0x1111_1111);
    }

    #[test]
    fn dilute_undilute_roundtrip_all_natives() {
        for x in 0..POOL_SIZE as u64 {
            let d = dilute(x);
            assert!(
                d <= dilute((POOL_SIZE - 1) as u64),
                "dilute({x}) = {d} exceeds pool maximum"
            );
            assert_eq!(undilute(d), x, "undilute(dilute({x})) round-trip failed");
        }
    }

    #[test]
    fn dilute_pinned_values_match_spec() {
        assert_eq!(dilute(0b0000_0000), 0x0000_0000);
        assert_eq!(dilute(0b0000_0001), 0x0000_0001);
        assert_eq!(dilute(0b0000_0010), 0x0000_0010);
        assert_eq!(dilute(0b0000_0100), 0x0000_0100);
        assert_eq!(dilute(0b0000_1000), 0x0000_1000);
        assert_eq!(dilute(0b0001_0000), 0x0001_0000);
        assert_eq!(dilute(0b0010_0000), 0x0010_0000);
        assert_eq!(dilute(0b0100_0000), 0x0100_0000);
        assert_eq!(dilute(0b1000_0000), 0x1000_0000);
        assert_eq!(dilute(0b1111_1111), 0x1111_1111);
    }

    #[test]
    fn pool_entries_match_dilute_for_each_native() {
        let pool = pool_entries();
        for (x, entry) in pool.iter().enumerate() {
            assert_eq!(
                *entry,
                F::new(dilute(x as u64)),
                "pool entry {x} disagrees with dilute({x})"
            );
        }
    }

    #[test]
    fn pool_membership_accepts_every_valid_diluted_value() {
        for x in 0..POOL_SIZE as u64 {
            assert!(is_in_pool(dilute(x)), "dilute({x}) not recognised in pool");
        }
    }

    #[test]
    fn pool_membership_rejects_off_spread_bits() {
        for off_bit in 0..64 {
            if off_bit % STRIDE == 0 && off_bit < NATIVE_BITS * STRIDE {
                continue;
            }
            let invalid = 1u64 << off_bit;
            assert!(
                !is_in_pool(invalid),
                "off-spread bit at position {off_bit} unexpectedly accepted",
            );
        }
    }

    #[test]
    fn chunk_decompose_roundtrip_random_u64() {
        let cases = [
            0u64,
            0xFFFF_FFFF_FFFF_FFFF,
            0xDEAD_BEEF_CAFE_BABE,
            0x1234_5678_9ABC_DEF0,
            0x0FED_CBA9_8765_4321,
            0x1u64 << 63,
        ];
        for operand in cases {
            let chunks = chunk_decompose(operand);
            for c in &chunks {
                assert!(*c < (1u64 << NATIVE_BITS), "chunk out of native range");
            }
            assert_eq!(chunk_recompose(&chunks), operand, "recompose mismatch");
        }
    }

    #[test]
    fn chunk_weights_form_powers_of_native_base() {
        let base = 1u64 << NATIVE_BITS;
        for k in 0..CHUNKS_PER_OPERAND {
            assert_eq!(chunk_weight(k), base.pow(k as u32));
        }
    }

    #[test]
    fn bitwise_identity_residuals_zero_on_correct_witnesses() {
        for a in 0..POOL_SIZE as u64 {
            for b in 0..POOL_SIZE as u64 {
                let w = chunk_witness(a, b);
                let [r_or, r_xor] =
                    bitwise_identity_residuals::<F>(w.d_a, w.d_b, w.d_and, w.d_or, w.d_xor);
                assert_eq!(
                    r_or,
                    F::ZERO,
                    "OR residual non-zero for ({a}, {b}): r_or = {r_or:?}",
                );
                assert_eq!(
                    r_xor,
                    F::ZERO,
                    "XOR residual non-zero for ({a}, {b}): r_xor = {r_xor:?}",
                );
            }
        }
    }

    #[test]
    fn bitwise_identity_residuals_nonzero_on_tampered_and() {
        let w = chunk_witness(0b1010_1010, 0b1100_1100);
        let tampered_and = F::new(dilute(0b1111_1111));
        let [r_or, r_xor] =
            bitwise_identity_residuals::<F>(w.d_a, w.d_b, tampered_and, w.d_or, w.d_xor);
        assert_ne!(
            r_or,
            F::ZERO,
            "OR residual must reject tampered d_and (a&b inflated)",
        );
        assert_ne!(
            r_xor,
            F::ZERO,
            "XOR residual must reject tampered d_and (a&b inflated)",
        );
    }

    #[test]
    fn bitwise_identity_residuals_nonzero_on_tampered_xor() {
        let w = chunk_witness(0b0110_0110, 0b1010_1010);
        let tampered_xor = F::new(dilute(0b0000_0001));
        let [r_or, r_xor] =
            bitwise_identity_residuals::<F>(w.d_a, w.d_b, w.d_and, w.d_or, tampered_xor);
        assert_eq!(
            r_or,
            F::ZERO,
            "OR residual must remain satisfied when only XOR is tampered",
        );
        assert_ne!(r_xor, F::ZERO, "XOR residual must reject tampered d_xor");
    }

    #[test]
    fn bitwise_identity_residuals_nonzero_on_tampered_or() {
        let w = chunk_witness(0b0011_0011, 0b0101_0101);
        let tampered_or = F::new(dilute(0b0000_0000));
        let [r_or, r_xor] =
            bitwise_identity_residuals::<F>(w.d_a, w.d_b, w.d_and, tampered_or, w.d_xor);
        assert_ne!(r_or, F::ZERO, "OR residual must reject tampered d_or");
        assert_eq!(
            r_xor,
            F::ZERO,
            "XOR residual must remain satisfied when only OR is tampered",
        );
    }

    #[test]
    fn logup_round_trip_against_pool_closes_grand_sum() {
        let mut engine = LogUpBuiltin::new();
        let pool = pool_entries().to_vec();
        engine.register_table(POOL_TABLE_ID, pool).unwrap();

        for x in 0..POOL_SIZE as u64 {
            engine
                .register_lookup(POOL_TABLE_ID, F::new(dilute(x)))
                .expect("dilute(x) must be a valid pool entry");
        }

        let trace_len = (POOL_SIZE + 1).next_power_of_two();
        let mut cols = engine.build_columns::<F>(trace_len, alpha()).unwrap();
        assert_eq!(cols.len(), 1, "exactly one pool table registered");
        let cols = cols.pop().unwrap();

        assert_eq!(cols.table_id, POOL_TABLE_ID);
        assert_eq!(cols.grand_sum[0], F::ZERO, "grand sum must start at zero");
        assert_eq!(
            cols.grand_sum[trace_len - 1],
            F::ZERO,
            "grand sum must close to zero -- diluted pool lookups multiset-equal pool",
        );
    }

    #[test]
    fn logup_rejects_off_spread_lookup() {
        let mut engine = LogUpBuiltin::new();
        engine
            .register_table(POOL_TABLE_ID, pool_entries().to_vec())
            .unwrap();
        let off_spread = F::new(0b0010);
        assert!(
            !is_in_pool(0b0010),
            "0b0010 sits on an off-spread bit by construction",
        );
        let err = engine
            .register_lookup(POOL_TABLE_ID, off_spread)
            .expect_err("off-spread value must be rejected at lookup registration");
        let _ = err;
    }
}
