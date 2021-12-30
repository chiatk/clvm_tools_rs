use clvm_rs::node::Node;

use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::stages::stage_0::{
    DefaultProgramRunner, RunProgramOption, TRunProgram,
};
//use crate::clvm_serialize::{node_from_bytes, node_to_bytes};
use clvm_rs::allocator::Allocator;
use clvm_rs::serialize::{node_from_bytes, node_to_bytes};

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
    return Ok(sha_256_encoded);
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
    return Ok(f_result);
}
