use wasm_bindgen::prelude::*;

// Called when the Wasm module is instantiated
#[wasm_bindgen]
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}
