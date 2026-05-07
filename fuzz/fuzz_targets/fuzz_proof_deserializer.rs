#![no_main]

use libfuzzer_sys::fuzz_target;
use maat_prover::deserialize_proof;

fuzz_target!(|data: &[u8]| {
    let libfuzzer_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| deserialize_proof(data));
    std::panic::set_hook(libfuzzer_hook);
});
