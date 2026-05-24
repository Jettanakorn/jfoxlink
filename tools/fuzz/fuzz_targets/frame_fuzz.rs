use arbitrary::Arbitrary;
use jfl_core::frame::JflFrame;
use std::vec::Vec;

#[derive(Arbitrary)]
struct RawFrameData { bytes: Vec<u8> }

fn main() {
    // cargo-fuzz entrypoint placeholder
    // Run: cargo fuzz run frame_fuzz
}
