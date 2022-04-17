use clvm_rs::allocator::{
    Allocator,
    NodePtr
};
use clvm_rs::reduction::EvalErr;

use crate::classic::clvm::sexp::enlist;
use crate::classic::clvm_tools::NodePath::NodePath;

lazy_static! {
    pub static ref QUOTE_ATOM : Vec<u8> = vec!(1);
    pub static ref APPLY_ATOM : Vec<u8> = vec!(2);
    pub static ref com_atom : Vec<u8> = vec!('c' as u8, 'o' as u8, 'm' as u8);
}

pub fn quote<'a>(allocator: &'a mut Allocator, sexp: NodePtr) -> Result<NodePtr,EvalErr> {
    return allocator.new_atom(&QUOTE_ATOM).
        and_then(|q| allocator.new_pair(q,sexp));
}

// In original python code, the name of this function is `eval`,
// but since the name `eval` cannot be used in typescript context, change the name to `evaluate`.
pub fn evaluate<'a>(allocator: &'a mut Allocator, prog: NodePtr, args: NodePtr) -> Result<NodePtr, EvalErr> {
    return m! {
        a <- allocator.new_atom(&APPLY_ATOM);
        enlist(allocator, &vec!(a, prog, args))
    };
}

pub fn run<'a>(allocator: &'a mut Allocator, prog: NodePtr, macro_lookup: NodePtr) -> Result<NodePtr, EvalErr> {
    /*
     * PROG => (e (com (q . PROG) (mac)) ARGS)
     *
     * The result can be evaluated with the stage_com eval
     * function.
     */
    let args = NodePath::new(None).as_path();
    return m! {
        mac <- quote(allocator, macro_lookup);
        com_sexp <- allocator.new_atom(&com_atom);
        arg_sexp <- allocator.new_atom(&args.data());
        to_eval <- enlist(allocator, &vec!(com_sexp, prog, mac));
        evaluate(allocator, to_eval, arg_sexp)
    };
}

pub fn brun<'a>(allocator: &'a mut Allocator, prog: NodePtr, args: NodePtr) -> Result<NodePtr, EvalErr> {
    return m! {
        quoted_prog <- quote(allocator, prog);
        quoted_args <- quote(allocator, args);
        evaluate(allocator, quoted_prog, quoted_args)
    };
}
