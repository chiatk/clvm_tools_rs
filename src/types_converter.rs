use bls12_381::G1Affine;
use std::alloc::alloc;
use std::borrow::Borrow;
use std::convert::TryInto;
use std::rc::Rc;

use crate::classic::clvm::__type_compatibility__::{t, Bytes, BytesFromType, Stream, Tuple};
use crate::classic::clvm::serialize::{sexp_from_stream, SimpleCreateCLVMObject};
use crate::classic::clvm::sexp::{to_sexp_type, CastableType};
use crate::classic::clvm_tools::binutils::disassemble;
use crate::clvm_api::{ArgBytesType, ClvmArg};
use crate::util::{number_from_u8, Number};
use clvm_rs::allocator::{Allocator, NodePtr};
use clvm_rs::reduction::EvalErr;
use yamlette::reader::BlockType::Byte;

// Convert Flutter arguments to to_sexp_type
pub fn toCLVMObject(allocator: &mut Allocator, argument: &ClvmArg) -> Result<NodePtr, EvalErr> {
    return match argument.value_type {
        ArgBytesType::Hex() => toHexType(allocator, argument),
        ArgBytesType::Bytes() => toBytesType(allocator, argument),
        ArgBytesType::String() => toStringType(allocator, argument),

        ArgBytesType::Number() => toNumberType(allocator, argument),
        ArgBytesType::G1Affine() => toG1AffineType(allocator, argument),
        ArgBytesType::ListOf() => toListOfType(allocator, argument),
        ArgBytesType::TupleOf() => toTupleOfType(allocator, argument),
    };
}

pub fn toHexType(allocator: &mut Allocator, hex_text: &ClvmArg) -> Result<NodePtr, EvalErr> {
    return toBytesType(allocator, hex_text);
}

pub fn toStringType(allocator: &mut Allocator, str: &ClvmArg) -> Result<NodePtr, EvalErr> {
    return to_sexp_type(
        allocator,
        CastableType::String(String::from_utf8(str.clone().value).unwrap()),
    );
}

pub fn toNumberType(allocator: &mut Allocator, number: &ClvmArg) -> Result<NodePtr, EvalErr> {
    return to_sexp_type(
        allocator,
        CastableType::Number(number_from_u8(number.value.borrow())),
    );
}

pub fn toG1AffineType(allocator: &mut Allocator, number: &ClvmArg) -> Result<NodePtr, EvalErr> {
    let bytes_array: [u8; 48] = vectorToFixedArray(number.clone().value);
    let g1Affline = G1Affine::from_compressed(&bytes_array).unwrap();
    return to_sexp_type(allocator, CastableType::G1Affine(g1Affline));
}

fn vectorToFixedArray<T>(v: Vec<T>) -> [T; 48]
where
    T: Copy,
{
    let slice = v.as_slice();
    let array: [T; 48] = match slice.try_into() {
        Ok(ba) => ba,
        Err(_) => panic!("Expected a Vec of length {} but it was {}", 48, v.len()),
    };
    array
}
pub fn toListOfType(allocator: &mut Allocator, list: &ClvmArg) -> Result<NodePtr, EvalErr> {
    let mut stack: Vec<Rc<CastableType>> = Vec::new();

    stack.push(Rc::new(CastableType::CLVMObject(allocator.null())));
    for i in 0..list.children.len() - 1 {
        let mut item = list.children[i].borrow();
        let mut node_ptr = toCLVMObject(allocator, item).unwrap();
        let clvmObject = _toCLVMObjectFromNodePtr(node_ptr);
        stack.push(Rc::new(clvmObject));
    }
    return to_sexp_type(allocator, CastableType::ListOf(stack.len(), stack));
}

pub fn toTupleOfType(allocator: &mut Allocator, list: &ClvmArg) -> Result<NodePtr, EvalErr> {
    let mut rigth: CastableType = CastableType::CLVMObject(allocator.null());
    let mut left: CastableType = CastableType::CLVMObject(allocator.null());

    for i in 0..1 {
        let mut item = list.children[i].borrow();
        let mut node_ptr = toCLVMObject(allocator, item).unwrap();
        let clvmObject = _toCLVMObjectFromNodePtr(node_ptr);

        if i == 0 {
            rigth = clvmObject;
        } else {
            left = clvmObject;
        }
    }

    return to_sexp_type(
        allocator,
        CastableType::TupleOf(Rc::new(rigth), Rc::new(left)),
    );
}

pub fn toBytesType(allocator: &mut Allocator, raw: &ClvmArg) -> Result<NodePtr, EvalErr> {
    return to_sexp_type(
        allocator,
        CastableType::Bytes(Bytes::new(Some(BytesFromType::Raw(raw.value.clone())))),
    );
}

pub fn _toCLVMObjectFromNodePtr(node: NodePtr) -> CastableType {
    return CastableType::CLVMObject(node.clone());
}
