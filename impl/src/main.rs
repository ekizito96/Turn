//! Turn CLI: turn run <file>

use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use turn::run;

#[derive(Parser)]
#[command(name = "turn")]
#[command(about = "Turn - object-oriented programming language for agentic software")]
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
            let source = fs::read_to_string(&file)?;
            let result = run(&source)?;
            println!("{}", result);
        }
    }
    Ok(())
}
