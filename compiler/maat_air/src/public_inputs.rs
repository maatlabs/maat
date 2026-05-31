//! Public inputs for the STARK constraint system.

use maat_field::{BaseElement, ToElements};

/// Public inputs shared between prover and verifier.
#[derive(Debug, Clone)]
pub struct MaatPublicInputs {
    pub inputs: Vec<BaseElement>,
    pub input_base: u32,
    pub output: BaseElement,
    pub output_base: u32,
    pub output_segment: Vec<BaseElement>,
    pub program_base: u32,
    pub program_segment: Vec<BaseElement>,
}

impl MaatPublicInputs {
    pub fn new(inputs: Vec<BaseElement>, output: BaseElement) -> Self {
        Self {
            inputs,
            input_base: 0,
            output,
            output_base: 0,
            output_segment: Vec::new(),
            program_base: 0,
            program_segment: Vec::new(),
        }
    }

    pub fn with_segments(
        inputs: Vec<BaseElement>,
        output: BaseElement,
        output_base: u32,
        output_segment: Vec<BaseElement>,
        program_base: u32,
        program_segment: Vec<BaseElement>,
    ) -> Self {
        Self {
            inputs,
            input_base: 0,
            output,
            output_base,
            output_segment,
            program_base,
            program_segment,
        }
    }

    pub fn with_output(output: BaseElement) -> Self {
        Self::new(Vec::new(), output)
    }

    pub fn with_input_base(mut self, input_base: u32) -> Self {
        self.input_base = input_base;
        self
    }
}

impl ToElements<BaseElement> for MaatPublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut elements = Vec::with_capacity(
            self.inputs.len() + 5 + self.output_segment.len() + self.program_segment.len(),
        );
        elements.extend_from_slice(&self.inputs);
        elements.push(BaseElement::new(u64::from(self.input_base)));
        elements.push(self.output);
        elements.push(BaseElement::new(u64::from(self.output_base)));
        elements.extend_from_slice(&self.output_segment);
        elements.push(BaseElement::new(u64::from(self.program_base)));
        elements.extend_from_slice(&self.program_segment);
        elements
    }
}

#[cfg(test)]
mod tests {
    use maat_field::FieldElement;

    use super::*;

    #[test]
    fn to_elements_includes_inputs_output_and_bases() {
        let inputs = vec![BaseElement::new(10), BaseElement::new(20)];
        let output = BaseElement::new(42);
        let pi = MaatPublicInputs::new(inputs, output);

        let elements = pi.to_elements();
        // 2 inputs + 1 input_base + 1 output + 1 output_base + 0 output cells
        //   + 1 program_base + 0 program cells.
        assert_eq!(elements.len(), 6);
        assert_eq!(elements[0], BaseElement::new(10));
        assert_eq!(elements[1], BaseElement::new(20));
        assert_eq!(elements[2], BaseElement::ZERO); // input_base
        assert_eq!(elements[3], BaseElement::new(42)); // output
        assert_eq!(elements[4], BaseElement::ZERO); // output_base
        assert_eq!(elements[5], BaseElement::ZERO); // program_base
    }

    #[test]
    fn to_elements_includes_output_and_program_segments() {
        let pi = MaatPublicInputs::with_segments(
            vec![],
            BaseElement::new(100),
            17,
            vec![
                BaseElement::new(7),
                BaseElement::new(13),
                BaseElement::new(31),
            ],
            1,
            vec![BaseElement::new(0xAA), BaseElement::new(0xBB)],
        );
        let elements = pi.to_elements();
        // 0 inputs + 1 input_base + 1 output + 1 output_base + 3 output cells
        //   + 1 program_base + 2 program cells.
        assert_eq!(elements.len(), 9);
        assert_eq!(elements[0], BaseElement::ZERO); // input_base
        assert_eq!(elements[1], BaseElement::new(100)); // output
        assert_eq!(elements[2], BaseElement::new(17)); // output_base
        assert_eq!(elements[3], BaseElement::new(7));
        assert_eq!(elements[4], BaseElement::new(13));
        assert_eq!(elements[5], BaseElement::new(31));
        assert_eq!(elements[6], BaseElement::ONE); // program_base
        assert_eq!(elements[7], BaseElement::new(0xAA));
        assert_eq!(elements[8], BaseElement::new(0xBB));
    }

    #[test]
    fn with_output_has_empty_inputs_and_segments() {
        let pi = MaatPublicInputs::with_output(BaseElement::new(99));
        let elements = pi.to_elements();
        // 1 input_base + 1 output + 1 output_base + 1 program_base.
        assert_eq!(elements.len(), 4);
        assert_eq!(elements[0], BaseElement::ZERO); // input_base
        assert_eq!(elements[1], BaseElement::new(99)); // output
        assert!(pi.output_segment.is_empty());
        assert!(pi.program_segment.is_empty());
    }
}
