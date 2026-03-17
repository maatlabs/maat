#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_bytecode::Bytecode;

fuzz_target!(|data: &[u8]| {
    let _ = Bytecode::deserialize(data);
});
