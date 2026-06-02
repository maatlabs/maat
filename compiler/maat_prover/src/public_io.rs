//! Verifier-side public-I/O bundle.

use std::fmt::Write;

use maat_errors::BundleError;
use maat_field::BaseElement;
use serde::{Deserialize, Serialize};

use crate::gadgets::proof_serializer::deserialize_proof;

type Result<T> = std::result::Result<T, BundleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicIo {
    #[serde(default)]
    pub inputs: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub private_inputs: Vec<String>,

    pub output: String,

    #[serde(default)]
    pub program_hash: String,
}

impl PublicIo {
    pub fn new(inputs: Vec<BaseElement>, output: BaseElement, program_hash: [u8; 32]) -> Self {
        Self {
            inputs: inputs.iter().map(|f| f.as_int().to_string()).collect(),
            private_inputs: Vec::new(),
            output: output.as_int().to_string(),
            program_hash: format_program_hash(&program_hash),
        }
    }

    pub fn inputs_felt(&self) -> Result<Vec<BaseElement>> {
        self.inputs.iter().map(|s| parse_felt(s)).collect()
    }

    pub fn private_inputs_felt(&self) -> Result<Vec<BaseElement>> {
        self.private_inputs.iter().map(|s| parse_felt(s)).collect()
    }

    pub fn output_felt(&self) -> Result<BaseElement> {
        parse_felt(&self.output)
    }

    pub fn program_hash_bytes(&self) -> Result<[u8; 32]> {
        parse_program_hash(&self.program_hash)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("PublicIo serialization is infallible")
    }

    pub fn from_json(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| BundleError::Parse(e.to_string()))
    }
}

pub fn extract_public_io(proof_bytes: &[u8]) -> Result<PublicIo> {
    let (_, embedded) = deserialize_proof(proof_bytes)?;
    Ok(PublicIo::new(
        embedded.memory.input.cells,
        embedded.output,
        maat_air::program_hash(&embedded.memory.program),
    ))
}

/// Format a 32-byte program hash as `"0x"` + 64 lowercase hex characters.
pub fn format_program_hash(hash: &[u8; 32]) -> String {
    let mut s = String::with_capacity(66);
    s.push_str("0x");
    for b in hash {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Parse a `"0x"`-prefixed 64-hex-character program hash.
pub fn parse_program_hash(value: &str) -> Result<[u8; 32]> {
    let invalid = |reason: String| BundleError::InvalidProgramHash {
        value: value.to_string(),
        reason,
    };
    let stripped = value
        .strip_prefix("0x")
        .ok_or_else(|| invalid("missing '0x' prefix".to_string()))?;
    if stripped.len() != 64 {
        return Err(invalid(format!(
            "expected 64 hex digits, got {}",
            stripped.len()
        )));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&stripped[i * 2..i * 2 + 2], 16)
            .map_err(|e| invalid(e.to_string()))?;
    }
    Ok(out)
}

/// Parse a decimal-string field element.
pub fn parse_felt(value: &str) -> Result<BaseElement> {
    value
        .trim()
        .parse::<u64>()
        .map(BaseElement::new)
        .map_err(|e| BundleError::InvalidFelt {
            value: value.to_string(),
            reason: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_round_trips_through_json() {
        let original = PublicIo::new(
            vec![BaseElement::new(3), BaseElement::new(7)],
            BaseElement::new(42),
            [0xab; 32],
        );
        let parsed = PublicIo::from_json(&original.to_json()).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn written_bundle_omits_empty_private_inputs() {
        let bundle = PublicIo::new(vec![BaseElement::new(1)], BaseElement::new(2), [0u8; 32]);
        assert!(
            !bundle.to_json().contains("private_inputs"),
            "--write-public-io must not emit private_inputs"
        );
    }

    #[test]
    fn prover_bundle_round_trips_private_inputs() {
        let mut bundle = PublicIo::new(vec![BaseElement::new(7)], BaseElement::new(11), [0u8; 32]);
        bundle.private_inputs = vec!["3".to_string(), "5".to_string()];
        let parsed = PublicIo::from_json(&bundle.to_json()).unwrap();
        assert_eq!(bundle, parsed);
        assert_eq!(
            parsed.private_inputs_felt().unwrap(),
            vec![BaseElement::new(3), BaseElement::new(5)]
        );
    }

    #[test]
    fn bundle_serializes_cells_as_decimal_strings() {
        let big = (1u64 << 50) + 7;
        let bundle = PublicIo::new(
            vec![BaseElement::new(big)],
            BaseElement::new(big + 4),
            [0u8; 32],
        );
        let json = bundle.to_json();
        assert!(json.contains(&format!("\"{big}\"")));
        assert!(json.contains(&format!("\"{}\"", big + 4)));
    }

    #[test]
    fn program_hash_round_trips() {
        let mut hash = [0u8; 32];
        for (i, b) in hash.iter_mut().enumerate() {
            *b = i as u8;
        }
        let formatted = format_program_hash(&hash);
        assert_eq!(formatted.len(), 66);
        assert!(formatted.starts_with("0x"));
        assert_eq!(parse_program_hash(&formatted).unwrap(), hash);
    }

    #[test]
    fn missing_prefix_rejected() {
        let err = parse_program_hash(&"ab".repeat(32)).unwrap_err();
        assert!(matches!(err, BundleError::InvalidProgramHash { .. }));
    }

    #[test]
    fn short_hash_rejected() {
        let err = parse_program_hash("0xabcd").unwrap_err();
        assert!(matches!(err, BundleError::InvalidProgramHash { .. }));
    }

    #[test]
    fn non_hex_character_rejected() {
        let err = parse_program_hash(&format!("0x{}", "zz".repeat(32))).unwrap_err();
        assert!(matches!(err, BundleError::InvalidProgramHash { .. }));
    }

    #[test]
    fn unknown_field_rejected_at_parse_time() {
        let json = r#"{
            "inputs": [],
            "output": "0",
            "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "stray": true
        }"#;
        let err = PublicIo::from_json(json).unwrap_err();
        assert!(matches!(err, BundleError::Parse(_)));
    }

    #[test]
    fn inputs_field_defaults_to_empty() {
        let json = r#"{
            "output": "1",
            "program_hash": "0x0000000000000000000000000000000000000000000000000000000000000000"
        }"#;
        let bundle = PublicIo::from_json(json).unwrap();
        assert!(bundle.inputs.is_empty());
    }

    #[test]
    fn minimal_output_only_bundle_parses() {
        let bundle = PublicIo::from_json(r#"{"output":"42"}"#).unwrap();
        assert_eq!(bundle.output_felt().unwrap(), BaseElement::new(42));
        assert!(bundle.inputs.is_empty());
        assert!(bundle.private_inputs.is_empty());
        assert!(bundle.program_hash.is_empty());
    }
}
