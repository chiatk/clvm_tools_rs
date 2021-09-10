use std::borrow::Borrow;
use std::rc::Rc;

use encoding8::ascii::is_printable;
use unicode_segmentation::UnicodeSegmentation;

use clvm_rs::allocator::{
    Allocator,
    NodePtr,
    SExp
};
use clvm_rs::reduction::EvalErr;

use crate::classic::clvm::__type_compatibility__::{
    Bytes,
    BytesFromType,
    Record,
    Stream
};
use crate::classic::clvm::{
    KEYWORD_TO_ATOM,
    KEYWORD_FROM_ATOM
};
use crate::classic::clvm_tools::ir::Type::IRRepr;
use crate::classic::clvm_tools::ir::reader::IRReader;
use crate::classic::clvm_tools::ir::writer::write_ir;

pub fn is_printable_string(s: &String) -> bool {
    for ch in s.graphemes(true) {
        if ch.chars().nth(0).unwrap() > 0xff as char || !is_printable(ch.chars().nth(0).unwrap() as u8) {
            return false;
        }
    }
    return true;
}

pub fn assemble_from_ir<'a>(
    allocator: &'a mut Allocator,
    ir_sexp: Rc<IRRepr>
) -> Result<NodePtr, EvalErr> {
    match ir_sexp.borrow() {
        IRRepr::Null => { return Ok(allocator.null()); },
        IRRepr::Quotes(b) => { return allocator.new_atom(b.data()); },
        IRRepr::Int(b,_signed) => { return allocator.new_atom(b.data()); },
        IRRepr::Hex(b) => { return allocator.new_atom(b.data()); },
        IRRepr::Symbol(s) => {
            let mut s_real_name = s.clone();
            if s.starts_with("#") {
                s_real_name = s[1..].to_string();
            } else {
                match KEYWORD_TO_ATOM().get(&s_real_name) {
                    Some(v) => { return allocator.new_atom(v); },
                    None => { }
                }
            }
            let v: Vec<u8> = s_real_name.as_bytes().to_vec();
            return allocator.new_atom(&v);
        },
        IRRepr::Cons(l,r) => {
            return assemble_from_ir(allocator, l.clone()).and_then(|l| {
                return assemble_from_ir(allocator, r.clone()).and_then(|r| {
                    return allocator.new_pair(l,r);
                });
            });
        }
    }
}

pub fn ir_for_atom(atom: &Bytes, allow_keyword: bool) -> IRRepr {
    if atom.length() == 0 {
        return IRRepr::Null;
    }
    if atom.length() > 2 {
        match String::from_utf8(atom.data().to_vec()) {
            Ok(v) => {
                if is_printable_string(&v) {
                    return IRRepr::Quotes(atom.clone());
                }
            },
            _ => { }
        }
        // Determine whether the bytes identity an integer in canonical form.
    } else {
        if allow_keyword {
            match KEYWORD_FROM_ATOM().get(atom.data()) {
                Some(kw) => { return IRRepr::Symbol(kw.to_string()); },
                _ => { }
            }
        }

        if atom.length() == 1 || (atom.length() > 1 && atom.data()[0] != 0) {
            return IRRepr::Int(atom.clone(), true);
        }
    }
    return IRRepr::Hex(atom.clone());
}

/*
 * (2 2 (2) (2 3 4)) => (a 2 (a) (a 3 4))
 *
 * d(P(2,P(2,P(P(2,()),P(P(2,P(3,P(4))))))), head=true)
 * a(2,true); d(P(2,P(P(2,()),P(P(2,P(3,P(4)))))), head=false)
 * a(2,false); d(P(P(2,()),P(P(2,P(3,P(4))))), head=false)
 * d(P(2,()), head=true); d(P(P(2,P(3,P(4)))), head=false)
 * a(2,true); d((), head=false); d(P(P(2,P(3,P(4)))), head=false)
 * a((),false); d(P(P(2,P(3,P(4)))), head=false)
 */
pub fn disassemble_to_ir_with_kw<'a>(
    allocator: &'a mut Allocator,
    sexp: NodePtr,
    keyword_from_atom: &Record<String, Vec<u8>>,
    head: bool,
    allow_keyword: bool
) -> IRRepr {
    match allocator.sexp(sexp) {
        SExp::Pair(l,r) => {
            let new_head =
                match allocator.sexp(l) {
                    SExp::Pair(_,_) => true,
                    _ => head
                };

            let v0 =
                disassemble_to_ir_with_kw(
                    allocator, l.clone(), keyword_from_atom, new_head, allow_keyword
                );
            let v1 =
                disassemble_to_ir_with_kw(
                    allocator, r.clone(), keyword_from_atom, false, allow_keyword
                );
            return IRRepr::Cons(Rc::new(v0), Rc::new(v1));
        },

        SExp::Atom(a) => {
            let bytes =
                Bytes::new(Some(BytesFromType::Raw(allocator.buf(&a).to_vec())));
            return ir_for_atom(&bytes, head && allow_keyword);
        }
    }
}

pub fn disassemble_with_kw<'a>(
    allocator: &'a mut Allocator,
    sexp: NodePtr,
    keyword_from_atom: &Record<String, Vec<u8>>
) -> String {
    let symbols = disassemble_to_ir_with_kw(
        allocator,
        sexp,
        &keyword_from_atom,
        true,
        true
    );
    return write_ir(Rc::new(symbols));
}

pub fn disassemble<'a>(
    allocator: &'a mut Allocator,
    sexp: NodePtr
) -> String {
    return disassemble_with_kw(
        allocator,
        sexp,
        KEYWORD_TO_ATOM()
    );
}

pub fn assemble<'a>(
    allocator: &'a mut Allocator,
    s: &String
) -> Result<NodePtr, EvalErr> {
    let v = s.as_bytes().to_vec();
    let stream = Stream::new(Some(Bytes::new(Some(BytesFromType::Raw(v)))));
    let mut reader = IRReader::new(stream);
    return reader.read_expr().
        map_err(|e| EvalErr(allocator.null(), e)).
        and_then(
        |ir| assemble_from_ir(allocator, Rc::new(ir))
        );
}