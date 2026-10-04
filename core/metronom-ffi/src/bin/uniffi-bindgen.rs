//! Generates the Kotlin/Swift bindings: `cargo run --features bindgen --bin uniffi-bindgen -- generate ...`

fn main() {
    uniffi::uniffi_bindgen_main();
}
