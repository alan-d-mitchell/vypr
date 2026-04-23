use std::{env, fs, path::{Path}, process::{self}, collections::HashSet};
use clap::{CommandFactory, Parser as ClapParser};

use lexer::lexer::Lexer;
use parser::parser::Parser;
use semantic::analyzer::Analyzer;
use vm::{compiler::Compiler, serializer::{self, Serializer}};
use vm::vm::VM;

#[derive(ClapParser, Debug)]
#[command(name = "vypr", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    #[arg(long, value_name = "TYPES", help = "comma separated list of types to emit: tokens, ast, bytecode[=debug|both]")]
    emit: Option<String>,

    #[arg(short, long, value_name = "OUTPUT", help = "specify name of output file")]
    output: Option<String>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BytecodeMode {
    Binary, // .vyc
    Debug,  // .chunk
    Both,   // .vyc and .chunk
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EmitType {
    Tokens,
    Ast,
    Bytecode(BytecodeMode),
}

// Helper to parse the --emit string
fn parse_emit_flag(s: &str) -> Vec<EmitType> {
    let clean_s = s.trim();
    
    // 1. Strip curly braces if present
    let content = if clean_s.starts_with('{') && clean_s.ends_with('}') {
        &clean_s[1..clean_s.len()-1]
    } else {
        clean_s
    };

    // 2. Split by comma and map to Enum
    content.split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part == "tokens" {
                EmitType::Tokens
            } else if part == "ast" {
                EmitType::Ast
            } else if part.starts_with("bytecode") {
                // Check for =debug or =both
                if let Some((_, val)) = part.split_once('=') {
                    match val {
                        "debug" => EmitType::Bytecode(BytecodeMode::Debug),
                        "both"  => EmitType::Bytecode(BytecodeMode::Both),
                        _ => {
                            eprintln!("[WARNING] unknown bytecode mode '{}', defaulting to binary", val);
                            EmitType::Bytecode(BytecodeMode::Binary)
                        }
                    }
                } else {
                    // Just "bytecode" -> Binary only
                    EmitType::Bytecode(BytecodeMode::Binary)
                }
            } else {
                eprintln!("[WARNING] unknown emit type '{}'", part);
                EmitType::Tokens // Fallback or panic, but better to skip in production code
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
    
    // Check flags for quick lookups
    let emit_tokens = emit_types.contains(&EmitType::Tokens);
    let emit_ast = emit_types.contains(&EmitType::Ast);
    
    // Find bytecode mode (if any)
    let bytecode_mode = emit_types.iter().find_map(|t| {
        if let EmitType::Bytecode(mode) = t { Some(mode) } else { None }
    });

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
        eprintln!("[SEMANTIC ERROR] {}", e);
        process::exit(1);
    }

    // --- PHASE 4: COMPILATION ---
    let compiler = Compiler::new();
    match compiler.compile(ast) {
        Ok(chunk) => {
            
            if let Some(mode) = bytecode_mode {
                let script_name = input_path.file_stem()
                    .unwrap_or(std::ffi::OsStr::new("script"))
                    .to_string_lossy();

                // 1. Emit .chunk (Debug Text)
                if *mode == BytecodeMode::Debug || *mode == BytecodeMode::Both {
                    let output = chunk.disassemble(&script_name);
                    let debug_fname = input_path.with_extension("chunk").to_string_lossy().into_owned();
                    fs::write(&debug_fname, output).ok();
                    println!("[INFO] debug bytecode written to: {}", debug_fname);
                }

                // 2. Emit .vyc (Binary Serialized)
                if *mode == BytecodeMode::Binary || *mode == BytecodeMode::Both {
                    let fname = input_path.with_extension("vyc").to_string_lossy().into_owned();
                    let mut serializer = Serializer::new(&fname).expect("failed to create .vyc file");
                    
                    match serializer.serialize(&chunk) {
                        Ok(_) => println!("[INFO] binary bytecode written to: {}", fname),
                        Err(e) => eprintln!("[ERROR] failed to write bytecode: {}", e),
                    }
                }
            }

            // --- FINAL STOP CHECK ---
            // If we emitted anything, we STOP here. 
            // If the user provided NO emit flags, we RUN the VM.
            if !emit_types.is_empty() {
                return;
            }

            // --- PHASE 5: EXECUTION ---
            let mut vm = VM::new(chunk);
            if let Err(e) = vm.run() {
                eprintln!("[RUNTIME ERROR] {:?}", e);
                process::exit(1);
            }
        },

        Err(e) => {
            eprintln!("[COMPILER ERROR] {}", e);
            process::exit(1);
        }
    }
}
