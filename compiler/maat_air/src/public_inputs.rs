//! Public inputs for the STARK constraint system.

use maat_field::{BaseElement, FieldElement, ToElements};

/// Number of field elements in the program hash.
const PROGRAM_HASH_LEN: usize = 4;

/// Public inputs shared between prover and verifier.
#[derive(Debug, Clone)]
pub struct MaatPublicInputs {
    pub program_hash: [BaseElement; PROGRAM_HASH_LEN],
    pub inputs: Vec<BaseElement>,
    pub output: BaseElement,
    pub output_base: u32,
    pub output_segment: Vec<BaseElement>,
}

impl MaatPublicInputs {
    pub fn new(
        program_hash: [BaseElement; PROGRAM_HASH_LEN],
        inputs: Vec<BaseElement>,
        output: BaseElement,
    ) -> Self {
        Self {
            program_hash,
            inputs,
            output,
            output_base: 0,
            output_segment: Vec::new(),
        }
    }

    pub fn with_output_segment(
        program_hash: [BaseElement; PROGRAM_HASH_LEN],
        inputs: Vec<BaseElement>,
        output: BaseElement,
        output_base: u32,
        output_segment: Vec<BaseElement>,
    ) -> Self {
        Self {
            program_hash,
            inputs,
            output,
            output_base,
            output_segment,
        }
    }

    pub fn with_output(output: BaseElement) -> Self {
        Self {
            program_hash: [BaseElement::ZERO; PROGRAM_HASH_LEN],
            inputs: vec![],
            output,
            output_base: 0,
            output_segment: Vec::new(),
        }
    }
}

impl ToElements<BaseElement> for MaatPublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut elements = Vec::with_capacity(
            PROGRAM_HASH_LEN + self.inputs.len() + 2 + self.output_segment.len(),
        );
        elements.extend_from_slice(&self.program_hash);
        elements.extend_from_slice(&self.inputs);
        elements.push(self.output);
        elements.push(BaseElement::new(u64::from(self.output_base)));
        elements.extend_from_slice(&self.output_segment);
        elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_elements_includes_legacy_fields() {
        let hash = [
            BaseElement::new(1),
            BaseElement::new(2),
            BaseElement::new(3),
            BaseElement::new(4),
        ];
        let inputs = vec![BaseElement::new(10), BaseElement::new(20)];
        let output = BaseElement::new(42);
        let pi = MaatPublicInputs::new(hash, inputs, output);

        let elements = pi.to_elements();
        // 4 hash + 2 inputs + 1 output + 1 output_base + 0 segment cells.
        assert_eq!(elements.len(), 8);
        assert_eq!(elements[0], BaseElement::new(1));
        assert_eq!(elements[4], BaseElement::new(10));
        assert_eq!(elements[6], BaseElement::new(42));
        assert_eq!(elements[7], BaseElement::ZERO);
    }

    #[test]
    fn to_elements_includes_output_segment() {
        let hash = [BaseElement::ZERO; PROGRAM_HASH_LEN];
        let pi = MaatPublicInputs::with_output_segment(
            hash,
            vec![],
            BaseElement::new(100),
            17,
            vec![
                BaseElement::new(7),
                BaseElement::new(13),
                BaseElement::new(31),
            ],
        );
        let elements = pi.to_elements();
        // 4 hash + 0 inputs + 1 output + 1 output_base + 3 segment cells.
        assert_eq!(elements.len(), 9);
        assert_eq!(elements[4], BaseElement::new(100));
        assert_eq!(elements[5], BaseElement::new(17));
        assert_eq!(elements[6], BaseElement::new(7));
        assert_eq!(elements[7], BaseElement::new(13));
        assert_eq!(elements[8], BaseElement::new(31));
    }

    #[test]
    fn with_output_has_zero_hash_and_empty_inputs() {
        let pi = MaatPublicInputs::with_output(BaseElement::new(99));
        let elements = pi.to_elements();
        // 4 zeros + 1 output + 1 output_base.
        assert_eq!(elements.len(), 6);
        assert_eq!(elements[0], BaseElement::ZERO);
        assert_eq!(elements[4], BaseElement::new(99));
        assert_eq!(elements[5], BaseElement::ZERO);
        assert!(pi.output_segment.is_empty());
    }
}
