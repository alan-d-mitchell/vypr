use std::{env, fs, path::{Path}, process::{self}};

use clap::{ArgAction, CommandFactory, Parser as ClapParser};

use lexer::lexer::Lexer;
use parser::parser::Parser;
use semantic::analyzer::Analyzer;
use vm::compiler::Compiler;
use vm::vm::VM;

#[derive(ClapParser, Debug)]
#[command(name = "vypr", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    #[arg(long, action = ArgAction::SetTrue, help = "emit tokens to a .tokens file")]
    tokens: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "emit ast nodes to a .nodes file")]
    ast: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "emit bytecode to a .chunk file")]
    bytecode: bool,

    #[arg(short, long, value_name = "OUTPUT", help = "specify name of output file")]
    output: Option<String>
}

fn main() {
    let cli = Cli::parse();

    if cli.input.is_none() {
        Cli::command().print_help().unwrap();
        println!();

        process::exit(1);
    }

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

    let mut lexer = Lexer::new(&contents);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[LEXER] {}", e);
            process::exit(1);
        }
    };

    if cli.tokens {
        let output = tokens.iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join("\n");
        
        let fname = input_path.with_extension("tokens").to_string_lossy().into_owned();
        if let Err(e) = fs::write(&fname, output) {
            eprintln!("[ERROR] failed to write tokens to file '{}': {}", fname, e);
            process::exit(1);
        }

        println!("[INFO] tokens written to: {}", fname);
    }

    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(ast) => ast,
        Err(e) => {
            eprintln!("[PARSER] {}", e);
            process::exit(1);
        }
    };

    if cli.ast {
        let output = ast.iter()
            .map(|node| format!("{:#?}", node))
            .collect::<Vec<_>>()
            .join("\n\n");
        
        let fname = input_path.with_extension("nodes").to_string_lossy().into_owned();
        if let Err(e) = fs::write(&fname, output) {
            eprintln!("[ERROR] failed to write nodes to file '{}': {}", fname, e);
            process::exit(1);
        }

        println!("[INFO] ast nodes written to: {}", fname);
    }
    
    let mut analyzer = Analyzer::new();
    match analyzer.analyze(&ast) {
        Ok(_) => println!("[INFO] Semantic analysis passed"),
        Err(e) => {
            eprintln!("[SEMANTIC ERROR] {}", e);
            process::exit(1);
        }
    }

    let compiler = Compiler::new();
    match compiler.compile(ast) {
        Ok(chunk) => {
            if cli.bytecode {
                let output = format!("{:#?}", chunk);
                let fname = input_path.with_extension("chunk").to_string_lossy().into_owned();

                if let Err(e) = fs::write(&fname, output) {
                    eprintln!("[ERROR] failed to write bytecode to file '{}': {}", fname, e);
                    process::exit(1);
                }

                println!("[INFO] bytecode chunk written to: {}", fname);
            }

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
