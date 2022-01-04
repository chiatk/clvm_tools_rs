#![allow(
    non_camel_case_types,
    unused,
    clippy::redundant_closure,
    clippy::useless_conversion,
    non_snake_case
)]
// AUTO GENERATED FILE, DO NOT EDIT.
// Generated by `flutter_rust_bridge`.

use crate::clvm_api::*;
use flutter_rust_bridge::*;

// Section: wire functions

#[no_mangle]
pub extern "C" fn wire_compiler_clvm(
    port: i64,
    to_run: *mut wire_uint_8_list,
    args: *mut wire_uint_8_list,
    file_path: *mut wire_uint_8_list,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "compiler_clvm",
            port: Some(port),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_to_run = to_run.wire2api();
            let api_args = args.wire2api();
            let api_file_path = file_path.wire2api();
            move |task_callback| compiler_clvm(api_to_run, api_args, api_file_path)
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_run_serialized_program(
    port: i64,
    program_data: *mut wire_uint_8_list,
    program_args: *mut wire_list_clvm_arg,
    calc_256_tree: bool,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "run_serialized_program",
            port: Some(port),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_program_data = program_data.wire2api();
            let api_program_args = program_args.wire2api();
            let api_calc_256_tree = calc_256_tree.wire2api();
            move |task_callback| {
                run_serialized_program(api_program_data, api_program_args, api_calc_256_tree)
            }
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_compile_string(port: i64, content: *mut wire_uint_8_list) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "compile_string",
            port: Some(port),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_content = content.wire2api();
            move |task_callback| compile_string(api_content)
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_run_string(
    port: i64,
    content: *mut wire_uint_8_list,
    args: *mut wire_uint_8_list,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "run_string",
            port: Some(port),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_content = content.wire2api();
            let api_args = args.wire2api();
            move |task_callback| run_string(api_content, api_args)
        },
    )
}

#[no_mangle]
pub extern "C" fn wire_compile_clvm_file(
    port: i64,
    real_input_path: *mut wire_uint_8_list,
    output_path: *mut wire_uint_8_list,
    search_paths: *mut wire_StringList,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap(
        WrapInfo {
            debug_name: "compile_clvm_file",
            port: Some(port),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_real_input_path = real_input_path.wire2api();
            let api_output_path = output_path.wire2api();
            let api_search_paths = search_paths.wire2api();
            move |task_callback| {
                compile_clvm_file(api_real_input_path, api_output_path, api_search_paths)
            }
        },
    )
}

// Section: wire structs

#[repr(C)]
#[derive(Clone)]
pub struct wire_StringList {
    ptr: *mut *mut wire_uint_8_list,
    len: i32,
}

#[repr(C)]
#[derive(Clone)]
pub struct wire_ClvmArg {
    value_type: i32,
    value: *mut wire_uint_8_list,
    children: *mut wire_list_clvm_arg,
}

#[repr(C)]
#[derive(Clone)]
pub struct wire_list_clvm_arg {
    ptr: *mut wire_ClvmArg,
    len: i32,
}

#[repr(C)]
#[derive(Clone)]
pub struct wire_uint_8_list {
    ptr: *mut u8,
    len: i32,
}

// Section: wire enums

// Section: allocate functions

#[no_mangle]
pub extern "C" fn new_StringList(len: i32) -> *mut wire_StringList {
    let wrap = wire_StringList {
        ptr: support::new_leak_vec_ptr(<*mut wire_uint_8_list>::new_with_null_ptr(), len),
        len,
    };
    support::new_leak_box_ptr(wrap)
}

#[no_mangle]
pub extern "C" fn new_list_clvm_arg(len: i32) -> *mut wire_list_clvm_arg {
    let wrap = wire_list_clvm_arg {
        ptr: support::new_leak_vec_ptr(<wire_ClvmArg>::new_with_null_ptr(), len),
        len,
    };
    support::new_leak_box_ptr(wrap)
}

#[no_mangle]
pub extern "C" fn new_uint_8_list(len: i32) -> *mut wire_uint_8_list {
    let ans = wire_uint_8_list {
        ptr: support::new_leak_vec_ptr(Default::default(), len),
        len,
    };
    support::new_leak_box_ptr(ans)
}

// Section: impl Wire2Api

pub trait Wire2Api<T> {
    fn wire2api(self) -> T;
}

impl<T, S> Wire2Api<Option<T>> for *mut S
where
    *mut S: Wire2Api<T>,
{
    fn wire2api(self) -> Option<T> {
        if self.is_null() {
            None
        } else {
            Some(self.wire2api())
        }
    }
}

impl Wire2Api<String> for *mut wire_uint_8_list {
    fn wire2api(self) -> String {
        let vec: Vec<u8> = self.wire2api();
        String::from_utf8_lossy(&vec).into_owned()
    }
}

impl Wire2Api<Vec<String>> for *mut wire_StringList {
    fn wire2api(self) -> Vec<String> {
        let vec = unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        };
        vec.into_iter().map(Wire2Api::wire2api).collect()
    }
}

impl Wire2Api<ArgBytesType> for i32 {
    fn wire2api(self) -> ArgBytesType {
        match self {
            0 => ArgBytesType::Hex,
            1 => ArgBytesType::String,
            2 => ArgBytesType::Bytes,
            3 => ArgBytesType::Number,
            4 => ArgBytesType::G1Affine,
            5 => ArgBytesType::ListOf,
            6 => ArgBytesType::TupleOf,
            _ => unreachable!("Invalid variant for ArgBytesType: {}", self),
        }
    }
}

impl Wire2Api<bool> for bool {
    fn wire2api(self) -> bool {
        self
    }
}

impl Wire2Api<ClvmArg> for wire_ClvmArg {
    fn wire2api(self) -> ClvmArg {
        ClvmArg {
            value_type: self.value_type.wire2api(),
            value: self.value.wire2api(),
            children: self.children.wire2api(),
        }
    }
}

impl Wire2Api<Vec<ClvmArg>> for *mut wire_list_clvm_arg {
    fn wire2api(self) -> Vec<ClvmArg> {
        let vec = unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        };
        vec.into_iter().map(Wire2Api::wire2api).collect()
    }
}

impl Wire2Api<u8> for u8 {
    fn wire2api(self) -> u8 {
        self
    }
}

impl Wire2Api<Vec<u8>> for *mut wire_uint_8_list {
    fn wire2api(self) -> Vec<u8> {
        unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        }
    }
}

// Section: impl NewWithNullPtr

pub trait NewWithNullPtr {
    fn new_with_null_ptr() -> Self;
}

impl<T> NewWithNullPtr for *mut T {
    fn new_with_null_ptr() -> Self {
        std::ptr::null_mut()
    }
}

impl NewWithNullPtr for wire_ClvmArg {
    fn new_with_null_ptr() -> Self {
        Self {
            value_type: Default::default(),
            value: core::ptr::null_mut(),
            children: core::ptr::null_mut(),
        }
    }
}

// Section: impl IntoDart

impl support::IntoDart for ClvmResponse {
    fn into_dart(self) -> support::DartCObject {
        vec![
            self.value_type.into_dart(),
            self.value.into_dart(),
            self.encoded.into_dart(),
            self.value_len.into_dart(),
        ]
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for ClvmResponse {}

impl support::IntoDart for ProgramResponse {
    fn into_dart(self) -> support::DartCObject {
        vec![
            self.cost.into_dart(),
            self.value.into_dart(),
            self.sha_256_tree.into_dart(),
        ]
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for ProgramResponse {}

// Section: executor
support::lazy_static! {
    pub static ref FLUTTER_RUST_BRIDGE_HANDLER: support::DefaultHandler = Default::default();
}

// Section: sync execution mode utility

#[no_mangle]
pub extern "C" fn free_WireSyncReturnStruct(val: support::WireSyncReturnStruct) {
    unsafe {
        let _ = support::vec_from_leak_ptr(val.ptr, val.len);
    }
}
