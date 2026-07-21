#![no_main]
//! libFuzzer target for the JFOXLink frame parser.
//!
//! The parser's core security invariant is that `from_bytes` must NEVER panic
//! on any input — it is the first thing a hostile RF environment reaches. This
//! target drives it with arbitrary bytes; any panic (out-of-bounds, unwrap,
//! arithmetic overflow) is a crash the fuzzer will surface.
//!
//! Run: `cargo +nightly fuzz run frame_fuzz -- -max_len=512`

use libfuzzer_sys::fuzz_target;

use jfl_core::frame::JflFrame;

fuzz_target!(|data: &[u8]| {
    // Must return Ok/Err, never panic.
    if let Ok(frame) = JflFrame::from_bytes(data) {
        // On a successful parse, the borrowed slices must be internally
        // consistent with the declared length — exercise the accessors.
        let _ = frame.encrypted_payload.len();
        let _ = frame.gcm_tag.len();
        let _ = frame.hmac.len();
        assert_eq!(frame.encrypted_payload.len(), frame.len as usize);
    }
});
