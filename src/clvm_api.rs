use crate::classic::clvm_tools::clvmc;
use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::stages::stage_0::{
    DefaultProgramRunner, RunProgramOption, TRunProgram,
};
use crate::compiler::clvm::parse_and_run;
use crate::compiler::clvm::run;
use crate::compiler::compiler::{compile_file, DefaultCompilerOpts};
use crate::compiler::comptypes::CompileErr;
use crate::compiler::runtypes::RunFailure;
use crate::compiler::sexp::parse_sexp;
use crate::compiler::srcloc::Srcloc;
use clvm_rs::allocator::Allocator;
use clvm_rs::allocator::{NodePtr, SExp};
use clvm_rs::serialize::node_from_bytes;
use std::collections::HashMap;
use std::rc::Rc;

use crate::classic::clvm::__type_compatibility__::Stream;
use crate::classic::clvm::serialize::sexp_to_stream;
use crate::clvm_serialize::prepare_response_for_flutter;
use anyhow::Result;
use clvm_rs::cost::Cost;
use clvm_rs::reduction::Response;
use clvm_rs::run_program::{run_program, STRICT_MODE};

#[derive(Debug, Clone)]
pub struct ClvmResponse {
    pub value_type: String,
    pub value: Vec<u8>,
    pub encoded: String,
    pub value_len: i32,
}

pub enum ArgBytesType {
    Hex(),

    String(),

    Bytes(),

    Number(),
    G1Affine(),
    ListOf(),
    TupleOf()
}

#[derive(Debug, Clone)]
pub struct ClvmArg {
    pub value_type: ArgBytesType,
    pub value: Vec<u8>,
    pub children:Vec<ClvmArg>,
}

#[derive(Debug, Clone)]
pub struct ProgramResponse {
    pub cost: u64,
    pub value: Vec<ClvmResponse>,

    pub sha_256_tree: Vec<u8>,
}
pub fn compiler_clvm(to_run: String, args: String, file_path: String) -> Result<Vec<ClvmResponse>> {
    let mut allocator = Allocator::new();
    let runner = Rc::new(DefaultProgramRunner::new());
    let response = parse_and_run(&mut allocator, runner, &file_path, &to_run, &args);

    let r2 = response.unwrap();
    let r2_ref = r2.as_ref();
    let mut convert_allocator = Allocator::new();

    let r_node = node_from_bytes(&mut convert_allocator, r2_ref.encode().as_ref()).unwrap();
    let values_response = prepare_response_for_flutter(r_node).unwrap();
    return Ok(values_response);
}

pub fn run_serialized_program(
    program_data: Vec<u8>,
    program_args: Vec<ClvmArg>,
    calc_256_tree: bool,
) -> Result<ProgramResponse> {
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
    let values_response = prepare_response_for_flutter(run_result.1).unwrap();

    let sha_256_encoded;
    if calc_256_tree {
        sha_256_encoded = sha256tree(&mut allocator, run_result.1).data().to_vec();
    } else {
        sha_256_encoded = vec![];
    }
    Ok(ProgramResponse {
        cost: run_result.0,

        value: values_response,
        sha_256_tree: sha_256_encoded,
    })
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

pub fn run_string(content: String, args: String) -> Result<Vec<ClvmResponse>> {
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
    let r2_ref = r2.as_ref();
    let mut convert_allocator = Allocator::new();

    let r_node = node_from_bytes(&mut convert_allocator, r2_ref.encode().as_ref()).unwrap();
    let values_response = prepare_response_for_flutter(r_node).unwrap();
    return Ok(values_response);
}

// Allow compile clvm file and return the file path
pub fn compile_clvm_file(
    real_input_path: String,
    output_path: String,
    search_paths: Vec<String>,
) -> Result<String> {
    let mut path_string = real_input_path;

    let _ = if !std::path::Path::new(&path_string).exists() && !path_string.ends_with(".clvm") {
        path_string = path_string + ".clvm";
    };

    let _ = print!("input   {}\n", path_string);

    return Ok(clvmc::compile_clvm(&path_string, &output_path, &search_paths).unwrap());
}
