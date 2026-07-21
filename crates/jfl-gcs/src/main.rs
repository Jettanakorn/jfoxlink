// The decoder and key store are part of the GCS binary. They were previously
// present as source files but never declared as modules, so they were never
// compiled, linked, or tested. Declaring them here wires the receive path in.
mod decoder;
mod key_store;

fn main() {
    println!("Starting jfl-gcs...");
    // GCS decoder / Simulator entrypoint
    // Integrates with mavlink crate for telemetry decoding
}
