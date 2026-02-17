//! Turn CLI: turn run <file>

use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use turn::run;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "turn")]
#[command(about = "Turn - object-oriented programming language for agentic software")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Turn program
    Run {
        /// Path to .turn file
        file: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { file } => {
            let source = fs::read_to_string(&file)
                .map_err(|e| format!("failed to read {}: {}", file.display(), e))?;
            let result = run(&source).map_err(|e| {
                let msg = e.to_string();
                let loc = if let Some(lex_err) = e.downcast_ref::<turn::lexer::LexError>() {
                    lex_err.offset().map(|o| turn::offset_to_line_col(&source, o))
                } else if let Some(parse_err) = e.downcast_ref::<turn::parser::ParseError>() {
                    Some(turn::offset_to_line_col(&source, parse_err.offset()))
                } else {
                    None
                };
                match loc {
                    Some((l, c)) => format!("{}:{}:{}: {}", file.display(), l, c, msg),
                    None => format!("{}: {}", file.display(), msg),
                }
            })?;
            println!("{}", result);
        }
    }
    Ok(())
}
