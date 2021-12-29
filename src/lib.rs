use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use std::collections::HashMap;
use std::rc::Rc;

use clvm_rs::allocator::{Allocator, NodePtr, SExp};
use clvm_rs::cost::Cost;
use clvm_rs::f_table::{f_lookup_for_hashmap, FLookup};
use clvm_rs::more_ops::op_unknown;
use clvm_rs::operator_handler::OperatorHandler;
use clvm_rs::reduction::{EvalErr, Reduction, Response};

use clvm_rs::run_program::{run_program, PreEval};

use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::stages::stage_0::{
    DefaultProgramRunner, OpRouter, RunProgramOption, TRunProgram,
};
use clvm_rs::chia_dialect::chia_dialect;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate derivative;

#[macro_use]
extern crate indoc;

#[macro_use]
extern crate do_notation;

#[macro_use]
#[cfg(not(any(test, target_family = "wasm")))]
extern crate pyo3;

mod util;

pub mod classic;
pub mod compiler;

// Python impl
#[cfg(not(any(test, target_family = "wasm")))]
mod py;

#[cfg(test)]
mod tests;

#[cfg(target_family = "wasm")]
pub mod wasm;

#[no_mangle]
pub extern "C" fn rust_greeting(to: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(to) };
    let recipient = match c_str.to_str() {
        Err(_) => "there",
        Ok(string) => string,
    };

    CString::new("Hello ".to_owned() + recipient)
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "C" fn rust_cstr_free(s: *mut c_char) {
    unsafe {
        if s.is_null() {
            return;
        }
        CString::from_raw(s)
    };
}

#[no_mangle]
pub extern "C" fn rust_run_clvm_program(data: &[u8]) -> *mut std::string::String {
    let apply_kw_vec = vec![2 as u8];
    let quote_kw_vec = vec![1 as u8];
    let mut allocator = Allocator::new();
    let program = match node_from_bytes(&mut allocator, data) {
        Err(_) => {
            let mut fail_result = std::string::String::from("");
            return &mut fail_result;
        }
        Ok(r) => r,
    };
    let args = allocator.null();
    let dialect = chia_dialect(false);
    let max_cost = 12000000000;
    let program_response = DefaultProgramRunner::new().run_program(
        &mut allocator,
        program,
        args,
        Some(RunProgramOption {
            operator_lookup: None,
            max_cost: if max_cost == 0 {
                None
            } else {
                Some(max_cost as u64)
            },
            pre_eval_f: None,
            strict: false,
        }),
    );
    let run_result = program_response.unwrap();

    let mut sha256 = sha256tree(&mut allocator, run_result.1).hex();
    return &mut sha256;
}
