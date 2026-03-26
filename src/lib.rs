mod types;
mod url_utils;
mod scoring;
mod processing;

use std::ffi::{CStr, CString, c_char};
use types::{ProcessInput, ProcessOutput};
use processing::process;

#[no_mangle]
pub extern "C" fn ddg_sp_process_json(input: *const c_char) -> *mut c_char {
    let input_str = unsafe {
        if input.is_null() {
            return std::ptr::null_mut();
        }
        match CStr::from_ptr(input).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let parsed: ProcessInput = match serde_json::from_str(input_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("suggestions_processor: input JSON parse error: {e}");
            return std::ptr::null_mut();
        }
    };

    let output: ProcessOutput = process(&parsed);

    let json = match serde_json::to_string(&output) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("suggestions_processor: output JSON serialize error: {e}");
            return std::ptr::null_mut();
        }
    };

    match CString::new(json) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn ddg_sp_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}
