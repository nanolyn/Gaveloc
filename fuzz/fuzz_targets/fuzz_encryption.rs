#![no_main]

use gaveloc_core::launch_args::EncryptedSessionId;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // The encryption function should never panic, even on arbitrary input.
    // It may return an error, but should not crash.
    let _ = EncryptedSessionId::new(data);
});
