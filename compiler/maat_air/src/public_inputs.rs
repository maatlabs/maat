//! Public inputs for the STARK constraint system.

use winter_math::fields::f64::BaseElement;
use winter_math::{FieldElement, ToElements};

/// Number of field elements in the program hash.
const PROGRAM_HASH_LEN: usize = 4;

/// Public inputs shared between prover and verifier.
#[derive(Debug, Clone)]
pub struct MaatPublicInputs {
    pub program_hash: [BaseElement; PROGRAM_HASH_LEN],
    pub inputs: Vec<BaseElement>,
    /// The claimed program output (last value on the stack at termination).
    pub output: BaseElement,
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
        }
    }

    pub fn with_output(output: BaseElement) -> Self {
        Self {
            program_hash: [BaseElement::ZERO; PROGRAM_HASH_LEN],
            inputs: vec![],
            output,
        }
    }
}

impl ToElements<BaseElement> for MaatPublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut elements = Vec::with_capacity(PROGRAM_HASH_LEN + self.inputs.len() + 1);
        elements.extend_from_slice(&self.program_hash);
        elements.extend_from_slice(&self.inputs);
        elements.push(self.output);
        elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_elements_includes_all_fields() {
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
        assert_eq!(elements.len(), 7); // 4 hash + 2 inputs + 1 output
        assert_eq!(elements[0], BaseElement::new(1));
        assert_eq!(elements[4], BaseElement::new(10));
        assert_eq!(elements[6], BaseElement::new(42));
    }

    #[test]
    fn with_output_has_zero_hash_and_empty_inputs() {
        let pi = MaatPublicInputs::with_output(BaseElement::new(99));
        let elements = pi.to_elements();
        assert_eq!(elements.len(), 5); // 4 zeros + 1 output
        assert_eq!(elements[0], BaseElement::ZERO);
        assert_eq!(elements[4], BaseElement::new(99));
    }
}
