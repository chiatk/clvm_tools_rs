use std::collections::HashMap;
use std::rc::Rc;
use std::fs;

use clvm_rs::allocator::{
    Allocator,
    NodePtr
};

use crate::classic::clvm::KEYWORD_FROM_ATOM;
use crate::classic::clvm::__type_compatibility__::{
    BytesFromType,
    Bytes,
    Stream,
    Tuple,
    t
};
use crate::classic::clvm::serialize::{
    SimpleCreateCLVMObject,
    sexp_from_stream
};
use crate::classic::clvm::SExp::{
    sexp_as_bin
};
use crate::classic::clvm_tools::binutils::{
    disassemble,
    assemble_from_ir
};
use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::ir::reader::read_ir;

use crate::classic::platform::PathJoin;

use crate::classic::platform::argparse::{
    Argument,
    ArgumentParser,
    ArgumentValue,
    ArgumentValueConv,
    IntConversion,
    NArgsSpec,
    TArgOptionAction,
    TArgumentParserProps
};

// import {
//   KEYWORD_FROM_ATOM,
//   SExp,
//   EvalError,
//   sexp_from_stream,
//   sexp_to_stream,
//   str,
//   Tuple,
//   None,
//   t,
//   Bytes,
//   int,
//   h,
//   TPreEvalF,
//   Optional,
//   CLVMObject,
// } from "clvm";

// import * as reader from "../ir/reader";
// import * as binutils from "./binutils";
// import {make_trace_pre_eval, trace_to_text, trace_to_table} from "./debug";
// import {sha256tree} from "./sha256tree";
// import {fs_exists, fs_isFile, fs_read, path_join} from "../platform/io";
// import {Stream} from "clvm/dist/__type_compatibility__";
// import * as argparse from "../platform/argparse";
// import * as stage_0 from "../stages/stage_0";
// import * as stage_1 from "../stages/stage_1";
// import * as stage_2 from "../stages/stage_2/index";
// import {TRunProgram} from "../stages/stage_0";
// import {now} from "../platform/performance";
// import {print} from "../platform/print";

pub struct PathOrCodeConv { }

impl ArgumentValueConv for PathOrCodeConv {
    fn convert(&self, arg: &String) -> Result<ArgumentValue,String> {
        match fs::read_to_string(arg) {
            Ok(s) => { return Ok(ArgumentValue::ArgString(s)); },
            Err(_) => { return Ok(ArgumentValue::ArgString(arg.to_string())); }
        }
    }
}

// export function stream_to_bin(write_f: (f: Stream) => void){
//   const f = new Stream();
//   write_f(f);
//   return f.getValue();
// }

pub trait TConversion {
    fn invoke<'a>(&self, allocator: &'a mut Allocator, text: &String) -> Result<Tuple<NodePtr, String>, String>;
}
pub fn call_tool<'a>(allocator: &'a mut Allocator, tool_name: String, desc: String, conversion: Box<dyn TConversion>, input_args: &Vec<String>) {
    let props = TArgumentParserProps {
        description: desc,
        prog: tool_name.to_string()
    };

    let mut parser = ArgumentParser::new(Some(props));
    parser.add_argument(
        vec!(
            "-H".to_string(),
            "--script-hash".to_string()
        ),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Show only sha256 tree hash of program".to_string())
    );
    parser.add_argument(
        vec!("path_or_code".to_string()),
        Argument::new().
            set_n_args(NArgsSpec::KleeneStar).
            set_type(Rc::new(PathOrCodeConv {})).
            set_help("path to clvm script, or literal script".to_string())
    );

    let mut rest_args = Vec::new();
    for a in input_args[1..].into_iter() {
        rest_args.push(a.to_string());
    }
    let args_res = parser.parse_args(&rest_args);
    let args : HashMap<String, ArgumentValue>;

    match args_res {
        Ok(a) => { args = a; },
        Err(e) => {
            print!("{:?}\n", e);
            return;
        }
    }

    let args_path_or_code_val =
        match args.get(&"path_or_code".to_string()) {
            None => ArgumentValue::ArgArray(vec!()),
            Some(v) => v.clone()
        };

    let args_path_or_code =
        match args_path_or_code_val {
            ArgumentValue::ArgArray(v) => v,
            _ => vec!()
        };

    for program in args_path_or_code {
        match program {
            ArgumentValue::ArgString(s) => {
                if s == "-" {
                    panic!("Read stdin is not supported at this time");
                }

                let conv_result = conversion.invoke(allocator, &s);
                match conv_result {
                    Ok(conv_result) => {
                        let sexp = conv_result.first().clone();
                        let text = conv_result.rest();
                        if args.contains_key(&"script_hash".to_string()) {
                            print!("{}\n", sha256tree(allocator, sexp).hex());
                        } else if text.len() > 0 {
                            print!("{}\n", text);
                        }
                    },
                    Err(e) => {
                        panic!("Conversion returned error: {:?}", e);
                    }
                }
            },
            _ => {
                panic!("inappropriate argument conversion");
            }
        }

    }
}

pub struct OpcConversion { }

impl TConversion for OpcConversion {
    fn invoke<'a>(&self, allocator: &'a mut Allocator, hex_text: &String) -> Result<Tuple<NodePtr, String>, String> {
        return read_ir(hex_text).and_then(|ir_sexp| {
            return assemble_from_ir(
                allocator,
                Rc::new(ir_sexp)
            ).map_err(|e| e.1);
        }).map(|sexp| {
            return t(sexp, sexp_as_bin(allocator, sexp).hex());
        });
    }
}

pub struct OpdConversion { }

impl TConversion for OpdConversion {
    fn invoke<'a>(&self, allocator: &'a mut Allocator, hex_text: &String) -> Result<Tuple<NodePtr, String>, String> {
        let mut stream =
            Stream::new(Some(Bytes::new(Some(BytesFromType::Hex(hex_text.to_string())))));

        return sexp_from_stream(
            allocator,
            &mut stream,
            Box::new(SimpleCreateCLVMObject {})
        ).map_err(|e| e.1).map(|sexp| {
            let disassembled = disassemble(allocator, sexp.1);
            return t(sexp.1, disassembled);
        });
    }
}

pub fn opc(args: &Vec<String>) {
    let mut allocator = Allocator::new();
    call_tool(&mut allocator, "opc".to_string(), "Compile a clvm script.".to_string(), Box::new(OpcConversion {}), args);
}

pub fn opd(args: &Vec<String>) {
    let mut allocator = Allocator::new();
    call_tool(&mut allocator, "opd".to_string(), "Disassemble a compiled clvm script from hex.".to_string(), Box::new(OpdConversion {}), args);
}

struct StageImport {
}

impl ArgumentValueConv for StageImport {
    fn convert(&self, arg: &String) -> Result<ArgumentValue,String> {
        if arg == "0" {
            return Ok(ArgumentValue::ArgInt(0));
        }
        else if arg == "1" {
            return Ok(ArgumentValue::ArgInt(1));
        }
        else if arg == "2" {
            return Ok(ArgumentValue::ArgInt(2));
        }
        return Err(format!("Unknown stage: {}", arg));
    }
}

// export function as_bin(streamer_f: (s: Stream) => unknown){
//   const f = new Stream();
//   streamer_f(f);
//   return f.getValue();
// }

pub fn run(args: &Vec<String>) {
    return launch_tool(args, &"run".to_string(), 2);
}

// export function brun(args: str[]){
//   return launch_tool(args, "brun");
// }

// export function calculate_cost_offset(run_program: TRunProgram, run_script: SExp){
//   /*
//     These commands are used by the test suite, and many of them expect certain costs.
//     If boilerplate invocation code changes by a fixed cost, you can tweak this
//     value so you don't have to change all the tests' expected costs.
//     Eventually you should re-tare this to zero and alter the tests' costs though.
//     This is a hack and need to go away, probably when we do dialects for real,
//     and then the dialect can have a `run_program` API.
//    */
//   const _null = binutils.assemble("0");
//   const result = run_program(run_script, _null.cons(_null));
//   const cost = result[0] as int;
//   return 53 - cost;
// }

pub fn launch_tool(args: &Vec<String>, tool_name: &String, default_stage: u32) {
    let props = TArgumentParserProps {
        description: "Execute a clvm script.".to_string(),
        prog: format!("clvm_tools {}", tool_name)
    };

    let mut parser = ArgumentParser::new(Some(props));
    parser.add_argument(
        vec!("--strict".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Unknown opcodes are always fatal errors in strict mode".to_string())
    );
    parser.add_argument(
        vec!("-x".to_string(), "--hex".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Read program and environment as hexadecimal bytecode".to_string())
    );
    parser.add_argument(
        vec!("-s".to_string(), "--stage".to_string()),
        Argument::new().
            set_type(Rc::new(StageImport {})).
            set_help("stage number to include".to_string()).
            set_default(ArgumentValue::ArgInt(default_stage as i64))
    );
    parser.add_argument(
        vec!("-v".to_string(), "--verbose".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Display resolve of all reductions, for debugging".to_string())
    );
    parser.add_argument(
        vec!("-t".to_string(), "--table".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Print diagnostic table of reductions, for debugging".to_string())
    );
    parser.add_argument(
        vec!("-c".to_string(), "--cost".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Show cost".to_string())
    );
    parser.add_argument(
        vec!("--time".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Print execution time".to_string())
    );
    parser.add_argument(
        vec!("-d".to_string(), "--dump".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("dump hex version of final output".to_string())
    );
    parser.add_argument(
        vec!("--quiet".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Suppress printing the program result".to_string())
    );
    parser.add_argument(
        vec!("-y".to_string(), "--symbol-table".to_string()),
        Argument::new().
            set_type(Rc::new(PathJoin {})).
            set_help(".SYM file generated by compiler".to_string())
    );
    parser.add_argument(
        vec!("-n".to_string(), "--no-keywords".to_string()),
        Argument::new().
            set_action(TArgOptionAction::StoreTrue).
            set_help("Output result as data, not as a program".to_string())
    );
    parser.add_argument(
        vec!("-i".to_string(), "--include".to_string()),
        Argument::new().
            set_type(Rc::new(PathJoin {})).
            set_help("add a search path for included files".to_string()).
            set_action(TArgOptionAction::Append).
            set_default(ArgumentValue::ArgArray(vec!()))
    );
    parser.add_argument(
        vec!("path_or_code".to_string()),
        Argument::new().
            set_type(Rc::new(PathOrCodeConv {})).
            set_help("filepath to clvm script, or a literal script".to_string())
    );
    parser.add_argument(
        vec!("env".to_string()),
        Argument::new().
            set_n_args(NArgsSpec::Optional).
            set_type(Rc::new(PathOrCodeConv {})).
            set_help("clvm script environment, as clvm src, or hex".to_string())
    );
    parser.add_argument(
        vec!("-m".to_string(), "--max-cost".to_string()),
        Argument::new().
            set_type(Rc::new(IntConversion::new(Rc::new(|| "help".to_string())))).
            set_default(ArgumentValue::ArgInt(11000000000)).
            set_help("Maximum cost".to_string())
    );

    let arg_vec = args[1..].to_vec();
    let mut parsedArgs: HashMap<String, ArgumentValue> =
        HashMap::new();

    match parser.parse_args(&arg_vec) {
        Err(e) => {
            print!("FAIL: {}\n", e);
            return;
        },
        Ok(pa) => { parsedArgs = pa; }
    }

    let empty_map = HashMap::new();
    let keywords =
        match parsedArgs.get("no_keywords") {
            None => KEYWORD_FROM_ATOM(),
            Some(ArgumentValue::ArgBool(b)) => &empty_map,
            _ => KEYWORD_FROM_ATOM()
        };

    // let mut run_program: Rc<TRunProgram>;
    // match parsedArgs.get("include") {
    //     Some(ArgArray(v)) => {
    //         run_program = Rc::new(stage_2::run_program_for_search_paths(
                
    //         );
    // );
    // match parsedArgs.get("stage") {
    //     Some(s) => {
    // if(typeof (parsedArgs["stage"] as typeof stage_2).run_program_for_search_paths == "function"){
    //     run_program = (parsedArgs["stage"] as typeof stage_2).run_program_for_search_paths(parsedArgs["include"] as str[]);
    // }
    // else{
    //     run_program = (parsedArgs["stage"] as typeof stage_0).run_program;
    // }

    // let input_serialized: Bytes|None = None;
    // let input_sexp: SExp|None = None;

    // const time_start = now();
    // let time_read_hex = -1;
    // let time_assemble = -1;
    // let time_parse_input = -1;
    // let time_done = -1;
    // if(parsedArgs["hex"]){
    //     const assembled_serialized = Bytes.from(parsedArgs["path_or_code"] as str, "hex");
    //     if(!parsedArgs["env"]){
    //         parsedArgs["env"] = "80";
    //     }
    //     const env_serialized = Bytes.from(parsedArgs["env"] as str, "hex");
    //     time_read_hex = now();

    //     input_serialized = h("0xff").concat(assembled_serialized).concat(env_serialized);
    // }
    // else {
    //     const src_text = parsedArgs["path_or_code"] as str;
    //     let mut src_sexp;
    //     match reader.read_ir(src_text) {
    //         Ok(s) => { src_sexp = s; },
    //         Err(e) => {
    //             print!("FAIL: {}\n", e);
    //             return;
    //         }
    //     }

    //     let assembled_sexp = binutils.assemble_from_ir(src_sexp);
    //     if(!parsedArgs["env"]){
    //         parsedArgs["env"] = "()";
    //     }
    //     const env_ir = reader.read_ir(parsedArgs["env"] as str);
    //     const env = binutils.assemble_from_ir(env_ir);
    //     time_assemble = now();

    //     input_sexp = to_sexp_f(t(assembled_sexp, env));
    // }

    // let pre_eval_f: TPreEvalF|None = None;
    // let symbol_table: Record<str, str>|None = None;
    // const log_entries: Array<[SExp, SExp, Optional<SExp>]> = [];

    // if(parsedArgs["symbol_table"]){
    //     symbol_table = JSON.parse(fs_read(parsedArgs["symbol_table"] as str));
    //     pre_eval_f = make_trace_pre_eval(log_entries, symbol_table);
    // }
    // else if(parsedArgs["verbose"] || parsedArgs["table"]){
    //     pre_eval_f = make_trace_pre_eval(log_entries);
    // }

    // const run_script = (parsedArgs["stage"] as Record<str, SExp>)[tool_name];

    // let cost = 0;
    // let result: SExp;
    // let output = "(didn't finish)";
    // const cost_offset = calculate_cost_offset(run_program, run_script);

    // try{
    //     const arg_max_cost = parsedArgs["max_cost"] as int;
    //     const max_cost = Math.max(0, (arg_max_cost !== 0 ? arg_max_cost - cost_offset : 0));
    //     // if use_rust: ...
    //     // else
    //     if(input_sexp === None){
    //         input_sexp = sexp_from_stream(new Stream(input_serialized as Bytes), to_sexp_f);
    //     }
    //     time_parse_input = now();
    //     const run_program_result = run_program(
    //         run_script, input_sexp, {max_cost, pre_eval_f, strict: parsedArgs["strict"] as boolean}
    //     );
    //     cost = run_program_result[0] as int;
    //     result = run_program_result[1] as SExp;
    //     time_done = now();

    //     if(parsedArgs["cost"]){
    //         cost += cost > 0 ? cost_offset : 0;
    //         print!("cost = {}\n", cost);
    //     }

    //     if(parsedArgs["time"]){
    //         if(parsedArgs["hex"]){
    //             print!("read_hex: {}\n", time_read_hex - time_start);
    //         }
    //         else{
    //             print!("assemble_from_ir: {}\n", time_assemble - time_start);
    //             print!("to_sexp_f: {}\n", time_parse_input - time_assemble);
    //         }
    //         print!("run_program: {}\n", time_done - time_parse_input);
    //     }

    //     if(parsedArgs["dump"]){
    //         const blob = as_bin(f => sexp_to_stream(result, f));
    //         output = blob.hex();
    //     }
    //     else if(parsedArgs["quiet"]){
    //         output = "";
    //     }
    //     else{
    //         output = binutils.disassemble(result, keywords);
    //     }
    // }
    // catch (ex) {
    //     if(ex instanceof EvalError){
    //         result = to_sexp_f(ex._sexp as CLVMObject);
    //         output = format!("FAIL: {} {}", ex.message, binutils.disassemble(result, keywords));
    //         return -1;
    //     }
    //     output = ex instanceof Error ? ex.message : typeof ex === "string" ? ex : JSON.stringify(ex);
    //     throw new Error(ex.message);
    // }
    // finally {
    //     print(output);
    //     if(parsedArgs["verbose"] || symbol_table){
    //         print("");
    //         trace_to_text(log_entries, binutils.disassemble, symbol_table || {});
    //     }
    //     if(parsedArgs["table"]){
    //         trace_to_table(log_entries, binutils.disassemble, symbol_table);
    //     }
    // }
}

// export function read_ir(args: str[]){
//   const parser = new argparse.ArgumentParser({description: "Read script and tokenize to IR."});
//   parser.add_argument(
//     ["script"], {help: "script in hex or uncompiled text"});
  
//   const parsedArgs = parser.parse_args(args.slice(1));
  
//   const sexp = reader.read_ir(parsedArgs["script"] as string);
//   const blob = stream_to_bin(f => sexp_to_stream(sexp, f));
//   print(blob.hex());
// }

/*
Copyright 2018 Chia Network Inc
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
   http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
 */