use core::cell::RefCell;

use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::io::Write;
use std::mem::swap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

use core::cmp::max;

use clvm_rs::allocator::{Allocator, NodePtr, SExp};
use clvm_rs::reduction::EvalErr;
use clvm_rs::run_program::PreEval;

use num_bigint::ToBigInt;
#[macro_use]
use yamlette::yamlette;
use yamlette::model::yaml::str::FORCE_QUOTES;

use crate::classic::clvm::__type_compatibility__::{t, Bytes, BytesFromType, Stream, Tuple};
use crate::classic::clvm::serialize::{sexp_from_stream, sexp_to_stream, SimpleCreateCLVMObject};
use crate::classic::clvm::sexp::{enlist, proper_list, sexp_as_bin};
use crate::classic::clvm::KEYWORD_FROM_ATOM;
use crate::classic::clvm_tools::binutils::{assemble_from_ir, disassemble, disassemble_with_kw};
use crate::classic::clvm_tools::clvmc::detect_modern;
use crate::classic::clvm_tools::debug::trace_pre_eval;
use crate::classic::clvm_tools::debug::{trace_to_table, trace_to_text};
use crate::classic::clvm_tools::ir::reader::read_ir;
use crate::classic::clvm_tools::sha256tree::sha256tree;
use crate::classic::clvm_tools::stages;
use crate::classic::clvm_tools::stages::stage_0::{
    DefaultProgramRunner, RunProgramOption, TRunProgram,
};
use crate::classic::clvm_tools::stages::stage_2::operators::run_program_for_search_paths;
use crate::classic::clvm_tools::stages::stage_2::optimize::optimize_sexp;

use crate::classic::platform::PathJoin;

use crate::classic::platform::argparse::{
    Argument, ArgumentParser, ArgumentValue, ArgumentValueConv, IntConversion, NArgsSpec,
    TArgOptionAction, TArgumentParserProps,
};
use crate::compiler::clvm::{convert_from_clvm_rs, get_history_len, run_step, start_step, RunStep};
use crate::compiler::compiler::{compile_file, run_optimizer, DefaultCompilerOpts};
use crate::compiler::comptypes::{CompileErr, CompilerOpts};
use crate::compiler::debug::build_symbol_table_mut;
use crate::compiler::prims;
use crate::compiler::runtypes::RunFailure;
use crate::compiler::sexp;
use crate::compiler::sexp::parse_sexp;
use crate::compiler::srcloc::Srcloc;
use crate::util::{collapse, Number};

pub struct PathOrCodeConv {}

impl ArgumentValueConv for PathOrCodeConv {
    fn convert(&self, arg: &String) -> Result<ArgumentValue, String> {
        match fs::read_to_string(arg) {
            Ok(s) => {
                return Ok(ArgumentValue::ArgString(Some(arg.to_string()), s));
            }
            Err(_) => {
                return Ok(ArgumentValue::ArgString(None, arg.to_string()));
            }
        }
    }
}

// export function stream_to_bin(write_f: (f: Stream) => void){
//   const f = new Stream();
//   write_f(f);
//   return f.getValue();
// }

pub trait TConversion {
    fn invoke<'a>(
        &self,
        allocator: &'a mut Allocator,
        text: &String,
    ) -> Result<Tuple<NodePtr, String>, String>;
}
pub fn call_tool<'a>(
    allocator: &'a mut Allocator,
    tool_name: String,
    desc: String,
    conversion: Box<dyn TConversion>,
    input_args: &Vec<String>,
) {
    let props = TArgumentParserProps {
        description: desc,
        prog: tool_name.to_string(),
    };

    let mut parser = ArgumentParser::new(Some(props));
    parser.add_argument(
        vec!["-H".to_string(), "--script-hash".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Show only sha256 tree hash of program".to_string()),
    );
    parser.add_argument(
        vec!["path_or_code".to_string()],
        Argument::new()
            .set_n_args(NArgsSpec::KleeneStar)
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("path to clvm script, or literal script".to_string()),
    );

    let mut rest_args = Vec::new();
    for a in input_args[1..].into_iter() {
        rest_args.push(a.to_string());
    }
    let args_res = parser.parse_args(&rest_args);
    let args: HashMap<String, ArgumentValue>;

    match args_res {
        Ok(a) => {
            args = a;
        }
        Err(e) => {
            print!("{:?}\n", e);
            return;
        }
    }

    let args_path_or_code_val = match args.get(&"path_or_code".to_string()) {
        None => ArgumentValue::ArgArray(vec![]),
        Some(v) => v.clone(),
    };

    let args_path_or_code = match args_path_or_code_val {
        ArgumentValue::ArgArray(v) => v,
        _ => vec![],
    };

    for program in args_path_or_code {
        match program {
            ArgumentValue::ArgString(_, s) => {
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
                    }
                    Err(e) => {
                        panic!("Conversion returned error: {:?}", e);
                    }
                }
            }
            _ => {
                panic!("inappropriate argument conversion");
            }
        }
    }
}

pub struct OpcConversion {}

impl TConversion for OpcConversion {
    fn invoke<'a>(
        &self,
        allocator: &'a mut Allocator,
        hex_text: &String,
    ) -> Result<Tuple<NodePtr, String>, String> {
        return read_ir(hex_text)
            .and_then(|ir_sexp| {
                return assemble_from_ir(allocator, Rc::new(ir_sexp)).map_err(|e| e.1);
            })
            .map(|sexp| {
                return t(sexp, sexp_as_bin(allocator, sexp).hex());
            });
    }
}

pub struct OpdConversion {}

impl TConversion for OpdConversion {
    fn invoke<'a>(
        &self,
        allocator: &'a mut Allocator,
        hex_text: &String,
    ) -> Result<Tuple<NodePtr, String>, String> {
        let mut stream = Stream::new(Some(Bytes::new(Some(BytesFromType::Hex(
            hex_text.to_string(),
        )))));

        return sexp_from_stream(allocator, &mut stream, Box::new(SimpleCreateCLVMObject {}))
            .map_err(|e| e.1)
            .map(|sexp| {
                let disassembled = disassemble(allocator, sexp.1);
                return t(sexp.1, disassembled);
            });
    }
}

pub fn opc(args: &Vec<String>) {
    let mut allocator = Allocator::new();
    call_tool(
        &mut allocator,
        "opc".to_string(),
        "Compile a clvm script.".to_string(),
        Box::new(OpcConversion {}),
        args,
    );
}

pub fn opd(args: &Vec<String>) {
    let mut allocator = Allocator::new();
    call_tool(
        &mut allocator,
        "opd".to_string(),
        "Disassemble a compiled clvm script from hex.".to_string(),
        Box::new(OpdConversion {}),
        args,
    );
}

struct StageImport {}

impl ArgumentValueConv for StageImport {
    fn convert(&self, arg: &String) -> Result<ArgumentValue, String> {
        if arg == "0" {
            return Ok(ArgumentValue::ArgInt(0));
        } else if arg == "1" {
            return Ok(ArgumentValue::ArgInt(1));
        } else if arg == "2" {
            return Ok(ArgumentValue::ArgInt(2));
        }
        return Err(format!("Unknown stage: {}", arg));
    }
}

pub fn run(args: &Vec<String>) {
    let mut s = Stream::new(None);
    launch_tool(&mut s, args, &"run".to_string(), 2);
    io::stdout().write_all(s.get_value().data());
}

pub fn brun(args: &Vec<String>) {
    let mut s = Stream::new(None);
    launch_tool(&mut s, args, &"brun".to_string(), 0);
    io::stdout().write_all(s.get_value().data());
}

pub fn hex_to_modern_sexp_inner(
    allocator: &mut Allocator,
    symbol_table: &HashMap<String, String>,
    loc: Srcloc,
    program: NodePtr,
) -> Result<Rc<sexp::SExp>, EvalErr> {
    let hash = sha256tree(allocator, program);
    let hash_str = hash.hex();
    let srcloc = symbol_table
        .get(&hash_str)
        .map(|f| Srcloc::start(f))
        .unwrap_or_else(|| loc.clone());

    match allocator.sexp(program) {
        SExp::Pair(a, b) => Ok(Rc::new(sexp::SExp::Cons(
            srcloc.clone(),
            hex_to_modern_sexp_inner(allocator, symbol_table, srcloc.clone(), a)?,
            hex_to_modern_sexp_inner(allocator, symbol_table, srcloc, b)?,
        ))),
        _ => convert_from_clvm_rs(allocator, srcloc, program).map_err(|_| {
            EvalErr(
                Allocator::null(allocator),
                "clvm_rs allocator failed".to_string(),
            )
        }),
    }
}

pub fn hex_to_modern_sexp(
    allocator: &mut Allocator,
    symbol_table: &HashMap<String, String>,
    loc: Srcloc,
    input_program: &String,
) -> Result<Rc<sexp::SExp>, RunFailure> {
    let input_serialized = Bytes::new(Some(BytesFromType::Hex(input_program.to_string())));

    let mut stream = Stream::new(Some(input_serialized.clone()));
    let sexp = sexp_from_stream(allocator, &mut stream, Box::new(SimpleCreateCLVMObject {}))
        .map(|x| x.1)
        .map_err(|_| RunFailure::RunErr(loc.clone(), "Bad conversion from hex".to_string()))?;

    hex_to_modern_sexp_inner(allocator, symbol_table, loc.clone(), sexp).map_err(|_| {
        RunFailure::RunErr(loc, "Failed to convert from classic to modern".to_string())
    })
}

#[derive(Clone, Debug)]
struct PriorResult {
    reference: usize,
    value: Rc<sexp::SExp>,
}

fn format_arg_inputs(args: &Vec<PriorResult>) -> String {
    let value_strings: Vec<String> = args
        .iter()
        .map(|pr| {
            return pr.reference.to_string();
        })
        .collect();
    return value_strings.join(", ");
}

fn get_arg_associations(
    associations: &HashMap<Number, PriorResult>,
    args: Rc<sexp::SExp>,
) -> Vec<PriorResult> {
    let mut arg_exp: Rc<sexp::SExp> = args;
    let mut result: Vec<PriorResult> = Vec::new();
    loop {
        match arg_exp.borrow() {
            sexp::SExp::Cons(_, arg, rest) => {
                match arg
                    .get_number()
                    .ok()
                    .as_ref()
                    .and_then(|n| associations.get(n))
                {
                    Some(n) => {
                        result.push(n.clone());
                    }
                    _ => {}
                }
                arg_exp = rest.clone();
            }
            _ => {
                return result;
            }
        }
    }
}

pub fn cldb(args: &Vec<String>) {
    let tool_name = "cldb".to_string();
    let mut hex = false;
    let dpr;
    let props = TArgumentParserProps {
        description: "Execute a clvm script.".to_string(),
        prog: format!("clvm_tools {}", tool_name),
    };

    let mut parser = ArgumentParser::new(Some(props));
    parser.add_argument(
        vec!["-i".to_string(), "--include".to_string()],
        Argument::new()
            .set_type(Rc::new(PathJoin {}))
            .set_help("add a search path for included files".to_string())
            .set_action(TArgOptionAction::Append)
            .set_default(ArgumentValue::ArgArray(vec![])),
    );
    parser.add_argument(
        vec!["-O".to_string(), "--optimize".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("run optimizer".to_string()),
    );
    parser.add_argument(
        vec!["-x".to_string(), "--hex".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("parse input program and arguments from hex".to_string()),
    );
    parser.add_argument(
        vec!["-y".to_string(), "--symbol-table".to_string()],
        Argument::new()
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("path to symbol file".to_string()),
    );
    parser.add_argument(
        vec!["path_or_code".to_string()],
        Argument::new()
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("filepath to clvm script, or a literal script".to_string()),
    );
    parser.add_argument(
        vec!["env".to_string()],
        Argument::new()
            .set_n_args(NArgsSpec::Optional)
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("clvm script environment, as clvm src, or hex".to_string()),
    );
    let arg_vec = args[1..].to_vec();
    let parsedArgs: HashMap<String, ArgumentValue>;

    let mut input_file = None;
    let mut input_program = "()".to_string();

    let prog_srcloc = Srcloc::start(&"*program*".to_string());
    let args_srcloc = Srcloc::start(&"*args*".to_string());

    let mut args = Rc::new(sexp::SExp::atom_from_string(
        args_srcloc.clone(),
        &"".to_string(),
    ));
    let mut parsed_args_result: String = "".to_string();
    let mut outputs_to_step = HashMap::<Number, PriorResult>::new();

    match parser.parse_args(&arg_vec) {
        Err(e) => {
            print!("FAIL: {}\n", e);
            return;
        }
        Ok(pa) => {
            parsedArgs = pa;
        }
    }

    match parsedArgs.get("path_or_code") {
        Some(ArgumentValue::ArgString(file, path_or_code)) => {
            input_file = file.clone();
            input_program = path_or_code.to_string();
        }
        _ => {}
    }

    match parsedArgs.get("env") {
        Some(ArgumentValue::ArgString(f, s)) => {
            parsed_args_result = s.to_string();
        }
        _ => {}
    }

    let run_program: Rc<dyn TRunProgram>;
    match parsedArgs.get("include") {
        Some(ArgumentValue::ArgArray(v)) => {
            let mut bare_paths = Vec::with_capacity(v.len());
            for p in v {
                match p {
                    ArgumentValue::ArgString(_, s) => bare_paths.push(s.to_string()),
                    _ => {}
                }
            }
            let special_runner = run_program_for_search_paths(&bare_paths);
            dpr = special_runner.clone();
            run_program = special_runner;
        }
        _ => {
            let ordinary_runner = run_program_for_search_paths(&Vec::new());
            dpr = ordinary_runner.clone();
            run_program = ordinary_runner;
        }
    }

    let mut allocator = Allocator::new();

    let symbol_table = parsedArgs
        .get("symbol_table")
        .and_then(|jstring| match jstring {
            ArgumentValue::ArgString(f, s) => {
                let decoded_symbol_table: Option<HashMap<String, String>> =
                    serde_json::from_str(&s).ok();
                decoded_symbol_table
            }
            _ => None,
        });

    let do_optimize = parsedArgs
        .get("optimize")
        .map(|x| match x {
            ArgumentValue::ArgBool(true) => true,
            _ => false,
        })
        .unwrap_or_else(|| false);
    let runner = Rc::new(DefaultProgramRunner::new());
    let use_filename = input_file
        .clone()
        .unwrap_or_else(|| "*command*".to_string());
    let opts = Rc::new(DefaultCompilerOpts::new(&use_filename)).set_optimize(do_optimize);

    let unopt_res = compile_file(&mut allocator, runner.clone(), opts.clone(), &input_program);

    let mut program = Rc::new(sexp::SExp::Nil(Srcloc::start(&"*nil*".to_string())));
    let mut output = Vec::new();
    let yamlette_string = |to_print: Vec<BTreeMap<String, String>>| match yamlette!(write; [[( # FORCE_QUOTES => to_print )]])
    {
        Ok(s) => s,
        Err(e) => format!("error producing yaml: {:?}", e),
    };

    let res = match parsedArgs.get("hex") {
        Some(ArgumentValue::ArgBool(true)) => {
            hex = true;
            hex_to_modern_sexp(
                &mut allocator,
                &symbol_table.unwrap_or_else(|| HashMap::new()),
                prog_srcloc.clone(),
                &input_program,
            )
            .map_err(|e| CompileErr(prog_srcloc, "Failed to parse hex".to_string()))
        }
        _ => {
            if do_optimize {
                unopt_res.and_then(|x| run_optimizer(&mut allocator, runner.clone(), Rc::new(x)))
            } else {
                unopt_res.map(|x| Rc::new(x))
            }
        }
    };

    match res {
        Ok(r) => {
            program = r.clone();
        }
        Err(c) => {
            let mut parse_error = BTreeMap::new();
            parse_error.insert("Error-Location".to_string(), c.0.to_string());
            parse_error.insert("Error".to_string(), c.1);
            output.push(parse_error.clone());
            print!("{}\n", yamlette_string(output));
            return;
        }
    }

    match parsedArgs.get("hex") {
        Some(ArgumentValue::ArgBool(true)) => {
            match hex_to_modern_sexp(
                &mut allocator,
                &HashMap::new(),
                args_srcloc.clone(),
                &parsed_args_result,
            ) {
                Ok(r) => {
                    args = r;
                }
                Err(p) => {
                    let mut parse_error = BTreeMap::new();
                    parse_error.insert("Error".to_string(), p.to_string());
                    output.push(parse_error.clone());
                    print!("{}\n", yamlette_string(output));
                    return;
                }
            }
        }
        _ => match parse_sexp(Srcloc::start(&"*arg*".to_string()), &parsed_args_result) {
            Ok(r) => {
                if r.len() > 0 {
                    args = r[0].clone();
                }
            }
            Err(c) => {
                let mut parse_error = BTreeMap::new();
                parse_error.insert("Error-Location".to_string(), c.0.to_string());
                parse_error.insert("Error".to_string(), c.1);
                output.push(parse_error.clone());
                print!("{}\n", yamlette_string(output));
                return;
            }
        },
    };

    let mut prim_map_ = HashMap::new();

    for p in prims::prims() {
        prim_map_.insert(p.0.clone(), Rc::new(p.1.clone()));
    }

    let prim_map = Rc::new(prim_map_);

    let program_lines: Vec<String> = input_program.lines().map(|x| x.to_string()).collect();
    let extract_text = |l: &Srcloc| {
        let use_line = if l.line < 1 { None } else { Some(l.line - 1) };
        let use_col = use_line.and_then(|_| if l.col < 1 { None } else { Some(l.col - 1) });
        let end_col = use_col.map(|c| l.until.map(|u| u.1 - 1).unwrap_or_else(|| c + 1));
        use_line
            .and_then(|use_line| {
                use_col.and_then(|use_col| {
                    end_col.and_then(|end_col| Some((use_line, use_col, end_col)))
                })
            })
            .and_then(|coords| {
                let use_line = coords.0;
                let mut use_col = coords.1;
                let mut end_col = coords.2;

                if use_line >= program_lines.len() {
                    None
                } else {
                    let line_text = program_lines[use_line].to_string();
                    if (use_col >= line_text.len()) {
                        None
                    } else if (end_col >= line_text.len()) {
                        end_col = line_text.len();
                        Some(line_text[use_col..end_col].to_string())
                    } else {
                        Some(line_text[use_col..end_col].to_string())
                    }
                }
            })
    };

    let whether_is_apply =
        |s: &sexp::SExp,
         collector: &mut BTreeMap<String, String>,
         if_true: &dyn Fn(&mut BTreeMap<String, String>),
         if_false: &dyn Fn(&mut BTreeMap<String, String>)| {
            match s {
                sexp::SExp::Integer(l, i) => {
                    if *i == 2_i32.to_bigint().unwrap() {
                        if_true(collector);
                        return;
                    }
                }
                _ => {}
            }

            if_false(collector);
        };

    let add_context = |s: &sexp::SExp,
                       c: &sexp::SExp,
                       args: Option<Rc<sexp::SExp>>,
                       context_result: &mut BTreeMap<String, String>| {
        whether_is_apply(
            s,
            context_result,
            &|context_result| match c {
                sexp::SExp::Cons(_, a, b) => {
                    context_result.insert("Env".to_string(), a.to_string());
                    context_result.insert("Env-Args".to_string(), b.to_string());
                }
                _ => {
                    context_result.insert("Function-Context".to_string(), c.to_string());
                }
            },
            &|context_result| match &args {
                Some(a) => {
                    context_result.insert("Arguments".to_string(), a.to_string());
                }
                _ => {}
            },
        );
    };

    let add_function = |input_file: Option<String>,
                        s: &sexp::SExp,
                        context_result: &mut BTreeMap<String, String>| {
        whether_is_apply(s, context_result, &|context_result| {}, &|context_result| {
            match extract_text(&s.loc()) {
                Some(name) => {
                    if Some(s.loc().file.to_string()) == input_file.clone() {
                        context_result.insert("Function".to_string(), name);
                    }
                }
                _ => {}
            }
        });
    };

    let mut step = start_step(program.clone(), args.clone());
    let mut in_expr = false;
    let mut to_print: BTreeMap<String, String> = BTreeMap::new();

    loop {
        let new_step = run_step(&mut allocator, runner.clone(), prim_map.clone(), &step);

        match &new_step {
            Ok(RunStep::OpResult(l, x, p)) => {
                if in_expr {
                    let history_len = get_history_len(p.clone());
                    to_print.insert("Result-Location".to_string(), l.to_string());
                    to_print.insert("Value".to_string(), x.to_string());
                    to_print.insert("Row".to_string(), output.len().to_string());
                    match x.get_number().ok() {
                        Some(n) => {
                            outputs_to_step.insert(
                                n,
                                PriorResult {
                                    reference: output.len(),
                                    value: x.clone(),
                                },
                            );
                        }
                        _ => {}
                    }
                    in_expr = false;
                    output.push(to_print.clone());
                    to_print = BTreeMap::new();
                    in_expr = false;
                }
            }
            Ok(RunStep::Done(l, x)) => {
                to_print.insert("Final-Location".to_string(), l.to_string());
                to_print.insert("Final".to_string(), x.to_string());
                output.push(to_print.clone());
                print!("{}\n", yamlette_string(output));
                return;
            }
            Ok(RunStep::Step(sexp, c, p)) => {}
            Ok(RunStep::Op(sexp, c, a, None, p)) => {
                let history_len = get_history_len(p.clone());
                to_print.insert("Operator-Location".to_string(), a.loc().to_string());
                to_print.insert("Operator".to_string(), sexp.to_string());
                match sexp.get_number().ok() {
                    Some(v) => {
                        if v == 11_u32.to_bigint().unwrap() {
                            let arg_associations =
                                get_arg_associations(&outputs_to_step, a.clone());
                            let args = format_arg_inputs(&arg_associations);
                            to_print.insert("Argument-Refs".to_string(), args);
                        }
                    }
                    _ => {}
                }
                add_context(sexp.borrow(), c.borrow(), Some(a.clone()), &mut to_print);
                add_function(input_file.clone(), sexp, &mut to_print);
                in_expr = true;
            }
            Ok(RunStep::Op(sexp, c, a, Some(v), p)) => {}
            Err(RunFailure::RunExn(l, s)) => {
                to_print.insert("Throw-Location".to_string(), l.to_string());
                to_print.insert("Throw".to_string(), s.to_string());
                output.push(to_print.clone());
                print!("{}\n", yamlette_string(output));
                return;
            }
            Err(RunFailure::RunErr(l, s)) => {
                to_print.insert("Failure-Location".to_string(), l.to_string());
                to_print.insert("Failure".to_string(), s.to_string());
                output.push(to_print.clone());
                print!("{}\n", yamlette_string(output));
                return;
            }
            _ => {}
        }

        step = new_step.unwrap_or_else(|_| step);
    }
}

struct RunLog<T> {
    log_entries: RefCell<Vec<T>>,
}

impl<T> RunLog<T> {
    fn push(&self, new_log: T) {
        self.log_entries.replace_with(|log| {
            let mut empty_log = Vec::new();
            swap(&mut empty_log, &mut *log);
            empty_log.push(new_log);
            return empty_log;
        });
    }

    fn finish(&self) -> Vec<T> {
        let mut empty_log = Vec::new();
        self.log_entries.replace_with(|log| {
            swap(&mut empty_log, &mut *log);
            return Vec::new();
        });
        return empty_log;
    }
}

fn calculate_cost_offset(
    allocator: &mut Allocator,
    run_program: Rc<dyn TRunProgram>,
    run_script: NodePtr,
) -> i64 {
    /*
     These commands are used by the test suite, and many of them expect certain costs.
     If boilerplate invocation code changes by a fixed cost, you can tweak this
     value so you don't have to change all the tests' expected costs.
     Eventually you should re-tare this to zero and alter the tests' costs though.
     This is a hack and need to go away, probably when we do dialects for real,
     and then the dialect can have a `run_program` API.
    */
    let almost_empty_list = enlist(allocator, &vec![allocator.null()]).unwrap();
    let cost = run_program
        .run_program(allocator, run_script, almost_empty_list, None)
        .map(|x| x.0)
        .unwrap_or_else(|_| 0);

    return 53 - cost as i64;
}

fn fix_log(
    allocator: &mut Allocator,
    log_result: &mut Vec<NodePtr>,
    log_updates: &Vec<(NodePtr, Option<NodePtr>)>,
) {
    let mut update_map: HashMap<NodePtr, Option<NodePtr>> = HashMap::new();
    for update in log_updates {
        update_map.insert(update.0, update.1);
    }

    for i in 0..log_result.len() {
        let entry = log_result[i];
        update_map.get(&entry).and_then(|v| *v).map(|v| {
            proper_list(allocator, entry, true).map(|list| {
                let mut updated = list.to_vec();
                updated.push(v);
                log_result[i] = enlist(allocator, &updated).unwrap();
            })
        });
    }
}

fn write_sym_output(
    compiled_lookup: &HashMap<String, String>,
    path: &String,
) -> Result<(), String> {
    m! {
        output <- serde_json::to_string(compiled_lookup).map_err(|_| {
            "failed to serialize to json".to_string()
        });

        fs::write(path.clone(), output).map_err(|_| {
            format!("failed to write {}", path)
        }).map(|_| ())
    }
}

pub fn launch_tool(
    stdout: &mut Stream,
    args: &Vec<String>,
    tool_name: &String,
    default_stage: u32,
) {
    let props = TArgumentParserProps {
        description: "Execute a clvm script.".to_string(),
        prog: format!("clvm_tools {}", tool_name),
    };

    let mut parser = ArgumentParser::new(Some(props));
    parser.add_argument(
        vec!["--strict".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Unknown opcodes are always fatal errors in strict mode".to_string()),
    );
    parser.add_argument(
        vec!["-x".to_string(), "--hex".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Read program and environment as hexadecimal bytecode".to_string()),
    );
    parser.add_argument(
        vec!["-s".to_string(), "--stage".to_string()],
        Argument::new()
            .set_type(Rc::new(StageImport {}))
            .set_help("stage number to include".to_string())
            .set_default(ArgumentValue::ArgInt(default_stage as i64)),
    );
    parser.add_argument(
        vec!["-v".to_string(), "--verbose".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Display resolve of all reductions, for debugging".to_string()),
    );
    parser.add_argument(
        vec!["-t".to_string(), "--table".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Print diagnostic table of reductions, for debugging".to_string()),
    );
    parser.add_argument(
        vec!["-c".to_string(), "--cost".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Show cost".to_string()),
    );
    parser.add_argument(
        vec!["--time".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Print execution time".to_string()),
    );
    parser.add_argument(
        vec!["-d".to_string(), "--dump".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("dump hex version of final output".to_string()),
    );
    parser.add_argument(
        vec!["--quiet".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Suppress printing the program result".to_string()),
    );
    parser.add_argument(
        vec!["-y".to_string(), "--symbol-table".to_string()],
        Argument::new()
            .set_type(Rc::new(PathJoin {}))
            .set_help(".SYM file generated by compiler".to_string()),
    );
    parser.add_argument(
        vec!["-n".to_string(), "--no-keywords".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("Output result as data, not as a program".to_string()),
    );
    parser.add_argument(
        vec!["-i".to_string(), "--include".to_string()],
        Argument::new()
            .set_type(Rc::new(PathJoin {}))
            .set_help("add a search path for included files".to_string())
            .set_action(TArgOptionAction::Append)
            .set_default(ArgumentValue::ArgArray(vec![])),
    );
    parser.add_argument(
        vec!["path_or_code".to_string()],
        Argument::new()
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("filepath to clvm script, or a literal script".to_string()),
    );
    parser.add_argument(
        vec!["env".to_string()],
        Argument::new()
            .set_n_args(NArgsSpec::Optional)
            .set_type(Rc::new(PathOrCodeConv {}))
            .set_help("clvm script environment, as clvm src, or hex".to_string()),
    );
    parser.add_argument(
        vec!["-m".to_string(), "--max-cost".to_string()],
        Argument::new()
            .set_type(Rc::new(IntConversion::new(Rc::new(|| "help".to_string()))))
            .set_default(ArgumentValue::ArgInt(11000000000))
            .set_help("Maximum cost".to_string()),
    );
    parser.add_argument(
        vec!["-O".to_string(), "--optimize".to_string()],
        Argument::new()
            .set_action(TArgOptionAction::StoreTrue)
            .set_help("run optimizer".to_string()),
    );

    let arg_vec = args[1..].to_vec();
    let parsedArgs: HashMap<String, ArgumentValue>;

    match parser.parse_args(&arg_vec) {
        Err(e) => {
            stdout.write_string(format!("FAIL: {}\n", e));
            return;
        }
        Ok(pa) => {
            parsedArgs = pa;
        }
    }

    let empty_map = HashMap::new();
    let keywords = match parsedArgs.get("no_keywords") {
        None => KEYWORD_FROM_ATOM(),
        Some(ArgumentValue::ArgBool(_b)) => &empty_map,
        _ => KEYWORD_FROM_ATOM(),
    };

    let dpr;
    let run_program: Rc<dyn TRunProgram>;
    match parsedArgs.get("include") {
        Some(ArgumentValue::ArgArray(v)) => {
            let mut bare_paths = Vec::with_capacity(v.len());
            for p in v {
                match p {
                    ArgumentValue::ArgString(_, s) => bare_paths.push(s.to_string()),
                    _ => {}
                }
            }
            let special_runner = run_program_for_search_paths(&bare_paths);
            dpr = special_runner.clone();
            run_program = special_runner;
        }
        _ => {
            let ordinary_runner = run_program_for_search_paths(&Vec::new());
            dpr = ordinary_runner.clone();
            run_program = ordinary_runner;
        }
    }

    let mut allocator = Allocator::new();

    let mut input_file = None;
    let mut input_serialized = None;
    let mut input_sexp;

    let time_start = SystemTime::now();
    let mut time_read_hex = SystemTime::now();
    let mut time_assemble = SystemTime::now();
    let time_parse_input;

    let mut input_program = "()".to_string();
    let mut input_args = "()".to_string();

    match parsedArgs.get("path_or_code") {
        Some(ArgumentValue::ArgString(file, path_or_code)) => {
            input_file = file.clone();
            input_program = path_or_code.to_string();
        }
        _ => {}
    }

    match parsedArgs.get("hex") {
        Some(_) => {
            let assembled_serialized =
                Bytes::new(Some(BytesFromType::Hex(input_program.to_string())));
            if input_args.len() == 0 {
                input_args = "80".to_string();
            }

            let env_serialized = Bytes::new(Some(BytesFromType::Hex(input_args.to_string())));
            time_read_hex = SystemTime::now();

            input_serialized = Some(
                Bytes::new(Some(BytesFromType::Raw(vec![0xff])))
                    .concat(&assembled_serialized)
                    .concat(&env_serialized),
            );

            let mut stream = Stream::new(input_serialized.clone());
            input_sexp = sexp_from_stream(
                &mut allocator,
                &mut stream,
                Box::new(SimpleCreateCLVMObject {}),
            )
            .map(|x| Some(x.1))
            .unwrap();
        }
        _ => {
            let src_sexp;
            match parsedArgs.get("path_or_code") {
                Some(ArgumentValue::ArgString(f, content)) => match read_ir(&content) {
                    Ok(s) => {
                        input_program = content.clone();
                        input_file = f.clone();
                        src_sexp = s;
                    }
                    Err(e) => {
                        stdout.write_string(format!("FAIL: {}\n", e));
                        return;
                    }
                },
                _ => {
                    stdout.write_string(format!("FAIL: {}\n", "non-string argument"));
                    return;
                }
            }

            let assembled_sexp = assemble_from_ir(&mut allocator, Rc::new(src_sexp)).unwrap();
            let mut parsed_args_result = "()".to_string();

            match parsedArgs.get("env") {
                Some(ArgumentValue::ArgString(f, s)) => {
                    parsed_args_result = s.to_string();
                }
                _ => {}
            }

            let env_ir = read_ir(&parsed_args_result).unwrap();
            let env = assemble_from_ir(&mut allocator, Rc::new(env_ir)).unwrap();
            time_assemble = SystemTime::now();

            input_sexp = allocator
                .new_pair(assembled_sexp, env)
                .map(|x| Some(x))
                .unwrap();
        }
    }

    // Symbol table related checks: should one be loaded, should one be saved.
    // This code is confusingly woven due to 'run' and 'brun' serving many roles.
    let mut symbol_table: Option<HashMap<String, String>> = None;
    let mut emit_symbol_output = false;

    let symbol_table_clone = parsedArgs
        .get("symbol_table")
        .and_then(|jstring| match jstring {
            ArgumentValue::ArgString(_, s) => fs::read_to_string(s).ok().and_then(|s| {
                let decoded_symbol_table: Option<HashMap<String, String>> =
                    serde_json::from_str(&s).ok();
                decoded_symbol_table
            }),
            _ => None,
        })
        .map(|st| {
            emit_symbol_output = true;
            symbol_table = Some(st.clone());
            st
        });

    match parsedArgs.get("verbose") {
        Some(ArgumentValue::ArgBool(true)) => {
            emit_symbol_output = true;
        }
        _ => {}
    }

    // In testing: short circuit for modern compilation.
    if input_sexp
        .map(|i| detect_modern(&mut allocator, i))
        .unwrap_or_else(|| false)
    {
        let do_optimize = parsedArgs
            .get("optimize")
            .map(|x| match x {
                ArgumentValue::ArgBool(true) => true,
                _ => false,
            })
            .unwrap_or_else(|| false);
        let runner = Rc::new(DefaultProgramRunner::new());
        let use_filename = input_file.unwrap_or_else(|| "*command*".to_string());
        let opts = Rc::new(DefaultCompilerOpts::new(&use_filename)).set_optimize(do_optimize);

        let unopt_res = compile_file(&mut allocator, runner.clone(), opts.clone(), &input_program);
        let res = if do_optimize {
            unopt_res.and_then(|x| run_optimizer(&mut allocator, runner, Rc::new(x)))
        } else {
            unopt_res.map(|x| Rc::new(x))
        };

        match res {
            Ok(r) => {
                print!("{}\n", r.to_string());

                let mut st = HashMap::new();
                build_symbol_table_mut(&mut st, &r);
                write_sym_output(&st, &"main.sym".to_string());
            }
            Err(c) => {
                print!("{}: {}\n", c.0.to_string(), c.1);
            }
        }

        return;
    }

    let mut pre_eval_f: Option<PreEval> = None;

    // Collections used to generate the run log.
    let log_entries: Arc<Mutex<RunLog<NodePtr>>> = Arc::new(Mutex::new(RunLog {
        log_entries: RefCell::new(Vec::new()),
    }));
    let log_updates: Arc<Mutex<RunLog<(NodePtr, Option<NodePtr>)>>> =
        Arc::new(Mutex::new(RunLog {
            log_entries: RefCell::new(Vec::new()),
        }));

    // clvm_rs uses boxed callbacks with unspecified lifetimes so in order to
    // support logging as intended, we must have values that can be moved so
    // the callbacks can become immortal.  Our strategy is to use channels
    // and threads for this.
    let (pre_eval_req_out, pre_eval_req_in) = channel();
    let (pre_eval_resp_out, pre_eval_resp_in): (Sender<()>, Receiver<()>) = channel();

    let (post_eval_req_out, post_eval_req_in) = channel();
    let (post_eval_resp_out, post_eval_resp_in): (Sender<()>, Receiver<()>) = channel();

    let post_eval_fn: Rc<dyn Fn(NodePtr, Option<NodePtr>)> = Rc::new(move |at, n| {
        post_eval_req_out.send((at, n));
        post_eval_resp_in.recv().unwrap();
    });

    let pre_eval_fn: Rc<dyn Fn(&mut Allocator, NodePtr)> = Rc::new(move |_allocator, new_log| {
        pre_eval_req_out.send(new_log);
        pre_eval_resp_in.recv().unwrap();
    });

    let closure: Rc<dyn Fn(NodePtr) -> Box<dyn Fn(Option<NodePtr>)>> = Rc::new(move |v| {
        let post_eval_fn_clone = post_eval_fn.clone();
        Box::new(move |n| {
            let post_eval_fn_clone_2 = post_eval_fn_clone.clone();
            (*post_eval_fn_clone_2)(v, n)
        })
    });

    if emit_symbol_output {
        let pre_eval_f_closure: Box<
            dyn Fn(
                &mut Allocator,
                NodePtr,
                NodePtr,
            ) -> Result<Option<Box<(dyn Fn(Option<NodePtr>))>>, EvalErr>,
        > = Box::new(move |allocator, sexp, args| {
            let pre_eval_clone = pre_eval_fn.clone();
            trace_pre_eval(
                allocator,
                &|allocator, n| (*pre_eval_clone)(allocator, n),
                symbol_table_clone.clone(),
                sexp,
                args,
            )
            .map(|t| {
                t.map(|log_ent| {
                    let closure_clone = closure.clone();
                    return (*closure_clone)(log_ent);
                })
            })
        });

        pre_eval_f = Some(pre_eval_f_closure);
    }

    let run_script = match parsedArgs.get("stage") {
        Some(ArgumentValue::ArgInt(0)) => stages::brun(&mut allocator),
        _ => stages::run(&mut allocator),
    };

    let mut output = "(didn't finish)".to_string();
    let cost_offset = calculate_cost_offset(&mut allocator, run_program.clone(), run_script);

    let max_cost = parsedArgs
        .get("max_cost")
        .map(|x| match x {
            ArgumentValue::ArgInt(i) => *i as i64 - cost_offset,
            _ => 0,
        })
        .unwrap_or_else(|| 0);
    let max_cost = max(0, max_cost);

    if input_sexp.is_none() {
        input_sexp = sexp_from_stream(
            &mut allocator,
            &mut Stream::new(input_serialized.clone()),
            Box::new(SimpleCreateCLVMObject {}),
        )
        .map(|x| Some(x.1))
        .unwrap();
    };

    // Part 2 of doing pre_eval: Have a thing that receives the messages and
    // performs some action.
    let log_entries_clone = log_entries.clone();
    thread::spawn(move || {
        let pre_in = pre_eval_req_in;
        let pre_out = pre_eval_resp_out;

        loop {
            match pre_in.recv() {
                Ok(received) => {
                    {
                        let locked = log_entries_clone.lock();
                        locked.unwrap().push(received);
                    }
                    pre_out.send(());
                }
                Err(_e) => {
                    break;
                }
            }
        }
    });

    let log_updates_clone = log_updates.clone();
    thread::spawn(move || {
        let post_in = post_eval_req_in;
        let post_out = post_eval_resp_out;

        loop {
            match post_in.recv() {
                Ok(received) => {
                    {
                        let locked = log_updates_clone.lock();
                        locked.unwrap().push(received);
                    }
                    post_out.send(());
                }
                Err(_e) => {
                    break;
                }
            }
        }
    });

    time_parse_input = SystemTime::now();

    let res = run_program
        .run_program(
            &mut allocator,
            run_script,
            input_sexp.unwrap(),
            Some(RunProgramOption {
                operator_lookup: None,
                max_cost: if max_cost == 0 {
                    None
                } else {
                    Some(max_cost as u64)
                },
                pre_eval_f: pre_eval_f,
                strict: parsedArgs
                    .get("strict")
                    .map(|_| true)
                    .unwrap_or_else(|| false),
            }),
        )
        .map(|run_program_result| {
            let mut cost: i64 = run_program_result.0 as i64;
            let result = run_program_result.1;
            let time_done = SystemTime::now();

            let _ = if !parsedArgs.get("cost").is_none() {
                if cost > 0 {
                    cost += cost_offset;
                }
                stdout.write_string(format!("cost = {}\n", cost));
            };

            let _ = match parsedArgs.get("time") {
                Some(ArgumentValue::ArgInt(_t)) => {
                    match parsedArgs.get("hex") {
                        Some(_) => {
                            stdout.write_string(format!(
                                "read_hex: {}\n",
                                time_read_hex
                                    .duration_since(time_start)
                                    .unwrap()
                                    .as_millis()
                            ));
                        }
                        _ => {
                            stdout.write_string(format!(
                                "assemble_from_ir: {}\n",
                                time_assemble
                                    .duration_since(time_start)
                                    .unwrap()
                                    .as_millis()
                            ));
                            stdout.write_string(format!(
                                "to_sexp_f: {}\n",
                                time_parse_input
                                    .duration_since(time_assemble)
                                    .unwrap()
                                    .as_millis()
                            ));
                        }
                    }
                    stdout.write_string(format!(
                        "run_program: {}\n",
                        time_done
                            .duration_since(time_parse_input)
                            .unwrap()
                            .as_millis()
                    ));
                }
                _ => {}
            };

            let _ = output = disassemble_with_kw(&mut allocator, result, keywords);
            let _ = match parsedArgs.get("dump") {
                Some(ArgumentValue::ArgBool(true)) => {
                    let mut f = Stream::new(None);
                    sexp_to_stream(&mut allocator, result, &mut f);
                    output = f.get_value().hex();
                }
                _ => match parsedArgs.get("quiet") {
                    Some(ArgumentValue::ArgBool(true)) => {
                        output = "".to_string();
                    }
                    _ => {}
                },
            };

            output
        });

    let output = collapse(res.map_err(|ex| {
        format!(
            "FAIL: {} {}",
            ex.1,
            disassemble_with_kw(&mut allocator, ex.0, keywords)
        )
    }));

    let compile_sym_out = dpr.get_compiles();
    if compile_sym_out.len() > 0 {
        write_sym_output(&compile_sym_out, &"main.sym".to_string());
    }

    stdout.write_string(format!("{}\n", output));

    // Third part of our scheme: now that we have results from the forward pass
    // and the pass doing the post callbacks, we can integrate them in the main
    // thread.  We didn't do this in the callbacks because we didn't want to
    // deal with a possibly escaping mutable allocator &.
    let mut log_content = log_entries.lock().unwrap().finish();
    let log_updates = log_updates.lock().unwrap().finish();
    fix_log(&mut allocator, &mut log_content, &log_updates);

    if emit_symbol_output {
        stdout.write_string(format!("\n"));
        trace_to_text(
            &mut allocator,
            stdout,
            &log_content,
            symbol_table.clone(),
            &disassemble,
        );
        if !parsedArgs.get("table").is_none() {
            trace_to_table(
                &mut allocator,
                stdout,
                &mut log_content,
                symbol_table,
                &disassemble,
            );
        }
    }
}

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
