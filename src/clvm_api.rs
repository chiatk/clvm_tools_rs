use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::stages::stage_0::{
    DefaultProgramRunner, RunProgramOption, TRunProgram,
};
use crate::compiler::clvm::run;
use crate::compiler::compiler::{compile_file, DefaultCompilerOpts};
use crate::compiler::comptypes::CompileErr;
use crate::compiler::runtypes::RunFailure;
use crate::compiler::sexp::parse_sexp;
use crate::compiler::srcloc::Srcloc;
use clvm_rs::allocator::{NodePtr, SExp};
use clvm_rs::node::Node;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::io::{Bytes, Cursor, Write};
use std::rc::Rc;
//use crate::clvm_serialize::{node_from_bytes, node_to_bytes};
use clvm_rs::allocator::Allocator;
use clvm_rs::serialize::{node_from_bytes, node_to_bytes, node_to_stream};

use crate::classic::clvm::__type_compatibility__::Stream;
use crate::classic::clvm::serialize::sexp_to_stream;
use crate::clvm_serialize::node_ptr_to_stream;
use anyhow::Result;

pub fn run_clvm_program_sha_256_tree(
    program_data: Vec<u8>,
    program_args: Vec<u8>,
) -> Result<Vec<u8>> {
    let mut allocator = Allocator::new();
    let mut allocator_args = Allocator::new();
    let program = node_from_bytes(&mut allocator, program_data.as_ref()).unwrap();

    let args;
    if program_args.len() == 0 {
        args = allocator_args.null();
    } else {
        args = node_from_bytes(&mut allocator_args, program_args.as_ref()).unwrap();
    }
    let max_cost = 12000000000 as u64;
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

    let sha_256_encoded = sha256tree(&mut allocator, run_result.1).data().to_vec();
    Ok(sha_256_encoded)
}

pub fn run_clvm_program_atom(program_data: Vec<u8>, program_args: Vec<u8>) -> Result<Vec<u8>> {
    let mut allocator = Allocator::new();
    let mut allocator_args = Allocator::new();
    let program = node_from_bytes(&mut allocator, program_data.as_ref()).unwrap();
    let args;
    if program_args.len() == 0 {
        args = allocator_args.null();
    } else {
        args = node_from_bytes(&mut allocator_args, program_args.as_ref()).unwrap();
    }
    let max_cost = 12000000000 as u64;
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
    let f_result = node_to_bytes(&Node::new(&allocator, run_result.1)).unwrap();
    Ok(f_result)
}

pub fn compile_string(content: String) -> Result<String> {
    let mut allocator = Allocator::new();
    let runner = Rc::new(DefaultProgramRunner::new());
    let opts = Rc::new(DefaultCompilerOpts::new(&"*test*".to_string()));

    let r = compile_file(&mut allocator, runner, opts, &content)
        .map(|x| x.to_string())
        .unwrap();
    Ok(r)
}

pub fn run_string(content: String, args: String) -> Result<Vec<u8>> {
    let mut allocator = Allocator::new();
    let runner = Rc::new(DefaultProgramRunner::new());
    let srcloc = Srcloc::start(&"*test*".to_string());
    let opts = Rc::new(DefaultCompilerOpts::new(&"*test*".to_string()));
    let sexp_args = parse_sexp(srcloc.clone(), &args).unwrap()[0].clone();

    let r = compile_file(&mut allocator, runner.clone(), opts, &content).and_then(|x| {
        run(
            &mut allocator,
            runner,
            Rc::new(HashMap::new()),
            Rc::new(x),
            sexp_args,
        )
        .map_err(|e| match e {
            RunFailure::RunErr(l, s) => CompileErr(l, s),
            RunFailure::RunExn(l, s) => CompileErr(l, s.to_string()),
        })
    });
    let r2 = r.unwrap();
    // let mut r_allocator = Allocator::new();
    let r2_ref = r2.as_ref();
    //let r_rc = r2_ref.borrow();
    let mut convert_allocator = Allocator::new();

    let r_node = node_from_bytes(&mut convert_allocator, r2_ref.encode().as_ref()).unwrap();

    let mut buffer = Cursor::new(Vec::new());
    let _ = node_ptr_to_stream(r_node, &mut buffer);
    let vec = buffer.into_inner();
    return Ok(vec);
}
