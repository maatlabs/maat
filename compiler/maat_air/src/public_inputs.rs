//! Public inputs for the STARK constraint system.

use maat_field::{BaseElement, ToElements};
use maat_trace::PublicMemory;

/// Public inputs shared between prover and verifier.
#[derive(Debug, Clone)]
pub struct MaatPublicInputs {
    /// Scalar program output bound at `COL_OUT` of the last trace row.
    pub output: BaseElement,
    /// The three address-bound public-memory segments (input, output, program).
    pub memory: PublicMemory,
}

impl MaatPublicInputs {
    pub fn new(output: BaseElement, memory: PublicMemory) -> Self {
        Self { output, memory }
    }

    pub fn with_output(output: BaseElement) -> Self {
        Self {
            output,
            memory: PublicMemory::default(),
        }
    }
}

impl ToElements<BaseElement> for MaatPublicInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let m = &self.memory;
        let mut elements = Vec::with_capacity(
            m.input.cells.len() + 5 + m.output.cells.len() + m.program.cells.len(),
        );
        elements.extend_from_slice(&m.input.cells);
        elements.push(BaseElement::new(u64::from(m.input.base)));
        elements.push(self.output);
        elements.push(BaseElement::new(u64::from(m.output.base)));
        elements.extend_from_slice(&m.output.cells);
        elements.push(BaseElement::new(u64::from(m.program.base)));
        elements.extend_from_slice(&m.program.cells);
        elements
    }
}

#[cfg(test)]
mod tests {
    use maat_field::FieldElement;
    use maat_trace::PublicSegment;

    use super::*;

    #[test]
    fn to_elements_includes_inputs_output_and_bases() {
        let inputs = vec![BaseElement::new(10), BaseElement::new(20)];
        let output = BaseElement::new(42);
        let pi = MaatPublicInputs::new(
            output,
            PublicMemory {
                input: PublicSegment::new(0, inputs),
                ..PublicMemory::default()
            },
        );

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
        let pi = MaatPublicInputs::new(
            BaseElement::new(100),
            PublicMemory {
                input: PublicSegment::default(),
                output: PublicSegment::new(
                    17,
                    vec![
                        BaseElement::new(7),
                        BaseElement::new(13),
                        BaseElement::new(31),
                    ],
                ),
                program: PublicSegment::new(
                    1,
                    vec![BaseElement::new(0xAA), BaseElement::new(0xBB)],
                ),
            },
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
        assert!(pi.memory.output.cells.is_empty());
        assert!(pi.memory.program.cells.is_empty());
    }
}
