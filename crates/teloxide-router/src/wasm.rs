//! Runs linker-synthesized constructors. Under wasm-bindgen/workers-rs nothing
//! calls `__wasm_call_ctors` for us (see inventory's crate docs); elsewhere
//! the C runtime does it, so this is a no-op.

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
mod imp {
    use std::sync::Once;

    // SAFETY: `__wasm_call_ctors` is synthesized by the wasm linker; calling
    // it runs the crate constructors. CTORS makes that happen once per
    // process, and inventory's constructors are themselves idempotent.
    unsafe extern "C" {
        fn __wasm_call_ctors();
    }

    static CTORS: Once = Once::new();

    pub fn run_ctors() {
        CTORS.call_once(|| unsafe { __wasm_call_ctors() });
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub fn run_ctors() {}
}

pub(crate) use imp::run_ctors;
