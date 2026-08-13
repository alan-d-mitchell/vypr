use std::{env, fs, path::Path, process};
use clap::{CommandFactory, Parser as ClapParser};

use lexer::lexer::Lexer;
use mir::{builder::MIRBuilder, mir::MIRProgram};
use parser::parser::Parser;
use semantic::analyzer::Analyzer;
use vir::builder::VIRBuilder;
use vm::{compiler::Compiler, serializer::Serializer};
use vm::vm::VM;

#[derive(ClapParser, Debug)]
#[command(name = "vypr", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    #[arg(long, value_name = "TYPES", help = "comma separated list of types to emit: tokens, ast, vir, mir-dbg, mir, chunk-dbg, chunk, vyc")]
    emit: Option<String>,

    #[arg(short, long, value_name = "OUTPUT", help = "specify name of output file")]
    output: Option<String>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EmitType {
    TOKENS,
    AST,
    VIR,
    MirDBG,
    MIR,
    ChunkDBG,
    CHUNK,
    VYC,
}

fn parse_emit_flag(s: &str) -> Vec<EmitType> {
    let clean_s = s.trim();
    
    let content = if clean_s.starts_with('{') && clean_s.ends_with('}') {
        &clean_s[1..clean_s.len()-1]
    } else {
        clean_s
    };

    content.split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            match part {
                "tokens" => Some(EmitType::TOKENS),
                "ast" => Some(EmitType::AST),
                "vir" => Some(EmitType::VIR),
                "mir-dbg" => Some(EmitType::MirDBG),
                "mir" => Some(EmitType::MIR),
                "chunk-dbg" => Some(EmitType::ChunkDBG),
                "chunk" => Some(EmitType::CHUNK),
                "vyc" => Some(EmitType::VYC),
                _ => {
                    eprintln!("[WARNING] unknown emit type '{}'", part);
                    None
                }
            }
        })
        .collect()
}

fn main() {
    let cli = Cli::parse();

    if cli.input.is_none() {
        Cli::command().print_help().unwrap();
        println!();
        process::exit(1);
    }

    // 1. Determine what we need to emit
    let emit_types = if let Some(emit_str) = &cli.emit {
        parse_emit_flag(emit_str)
    } else {
        Vec::new()
    };
    
    let emit_tokens = emit_types.contains(&EmitType::TOKENS);
    let emit_ast = emit_types.contains(&EmitType::AST);
    let emit_vir = emit_types.contains(&EmitType::VIR);
    let emit_mir_dbg = emit_types.contains(&EmitType::MirDBG);
    let emit_mir = emit_types.contains(&EmitType::MIR);
    let emit_chunk_dbg = emit_types.contains(&EmitType::ChunkDBG);
    let emit_chunk = emit_types.contains(&EmitType::CHUNK);
    let emit_vyc = emit_types.contains(&EmitType::VYC);

    let input = cli.input.unwrap();
    let input_path = Path::new(&input);

    match input_path.extension().and_then(|e| e.to_str()) {
        Some("vypr" | "py") => {}
        _ => {
            eprintln!("[ERROR] '{}' is not a vypr or python file", input);
            process::exit(1);
        }
    }

    let contents = match fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            println!("[ERROR] failed while reading '{}': {}", input, e);
            process::exit(1);
        }
    };

    // --- PHASE 1: LEXER ---
    let mut lexer = Lexer::new(&contents);
    let tokens = lexer.tokenize();

    if !lexer.errors.is_empty() {
        for error in &lexer.errors {
            error.report(&contents, &input);
        }
        process::exit(1);
    }

    if emit_tokens {
        let output = tokens.iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join("\n");
        
        let fname = input_path.with_extension("tokens").to_string_lossy().into_owned();
        fs::write(&fname, output).ok();
        println!("[INFO] tokens written to: {}", fname);
    }

    // --- PHASE 2: PARSER ---
    let mut parser = Parser::new(tokens);
    let ast = parser.parse();

    if !parser.errors.is_empty() {
        for error in &parser.errors {
            error.report(&contents, &input);
        }
        process::exit(1);
    }

    if emit_ast {
        let output = ast.iter()
            .map(|node| format!("{:#?}", node))
            .collect::<Vec<_>>()
            .join("\n\n");
        
        let fname = input_path.with_extension("nodes").to_string_lossy().into_owned();
        fs::write(&fname, output).ok();
        println!("[INFO] ast nodes written to: {}", fname);
    }
    
    // --- PHASE 3: SEMANTIC ANALYSIS ---
    let mut analyzer = Analyzer::new();
    if let Err(e) = analyzer.analyze(&ast) {
        e.report(&contents, &input);
        process::exit(1);
    }

    // --- PHASE 4: VIR (HIR) LOWERING ---
    let mut vir_builder = VIRBuilder::new();
    vir_builder.inject_globals(analyzer.export_globals());
    let vir_program = vir_builder.build(&ast);

    if emit_vir {
        let output = format!("{}", vir_program);
        let fname = input_path.with_extension("vir").to_string_lossy().into_owned();
        fs::write(&fname, output).ok();
        println!("[INFO] vir written to: {}", fname);
    }

    // --- PHASE 5: MIR LOWERING ---
    let mut mir_program = MIRProgram { functions: Vec::new() };
    for function in vir_program.functions {
        let is_script = function.name == "<script>";
        let mir_builder = MIRBuilder::new(is_script);
        let mir_function = mir_builder.build_function(function);
        mir_program.functions.push(mir_function);
    }

    if emit_mir_dbg {
        let output = format!("{}", mir_program);
        let fname = input_path.with_extension("dbg.mir").to_string_lossy().into_owned();
        fs::write(&fname, output).ok();
        println!("[INFO] dbg mir written to: {}", fname);
    }

    if emit_chunk_dbg {
        let compiler = Compiler::new();
        match compiler.compile_program(&mir_program) {
            Ok(chunk) => {
                let script_name = input_path.file_stem()
                    .unwrap_or(std::ffi::OsStr::new("script"))
                    .to_string_lossy();

                let output = chunk.disassemble(&script_name);
                let debug_fname = input_path.with_extension("dbg.chunk").to_string_lossy().into_owned();
                fs::write(&debug_fname, output).ok();
                println!("[INFO] dbg bytecode written to: {}", debug_fname);
            }

            Err(e) => {
                e.report(&contents, &input);
                process::exit(1);
            }
        }
    }

    mir::optimizer::Optimizer::optimize(&mut mir_program);

    if emit_mir {
        let output = format!("{}", mir_program);
        let fname = input_path.with_extension("mir").to_string_lossy().into_owned();
        fs::write(&fname, output).ok();
        println!("[INFO] release mir written to: {}", fname);
    }

    // --- PHASE 6: COMPILATION ---
    let compiler = Compiler::new();
    match compiler.compile_program(&mir_program) {
        Ok(chunk) => {
            let script_name = input_path.file_stem()
                .unwrap_or(std::ffi::OsStr::new("script"))
                .to_string_lossy();

            // 1. Emit .chunk (Debug Text)
            if emit_chunk {
                let output = chunk.disassemble(&script_name);
                let debug_fname = input_path.with_extension("chunk").to_string_lossy().into_owned();
                fs::write(&debug_fname, output).ok();
                println!("[INFO] release bytecode written to: {}", debug_fname);
            }

            // 2. Emit .vyc (Binary Serialized)
            if emit_vyc {
                let fname = input_path.with_extension("vyc").to_string_lossy().into_owned();
                let mut serializer = Serializer::new(&fname).expect("failed to create .vyc file");
                
                match serializer.serialize(&chunk) {
                    Ok(_) => println!("[INFO] binary bytecode written to: {}", fname),
                    Err(e) => eprintln!("[ERROR] failed to write bytecode: {}", e),
                }
            }

            // --- FINAL STOP CHECK ---
            // If we emitted anything, we STOP here. 
            if !emit_types.is_empty() {
                return;
            }

            // --- PHASE 7: EXECUTION ---
            let mut vm = VM::new(chunk);
            if let Err(e) = vm.run() {
                e.report(&contents, &input);
                process::exit(1);
            }
        },

        Err(e) => {
            e.report(&contents, &input);
            process::exit(1);
        }
    }
}
