//! Turn CLI: turn run <file>

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use turn::{FileStore, Runner, ToolRegistry};

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
        
        /// Agent ID (for persistence)
        #[arg(long, default_value = "default_agent")]
        id: String,
        
        /// Path to store directory
        #[arg(long, default_value = ".turn_store")]
        store: PathBuf,
    },
    
    /// Start Turn server (HTTP API)
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,

        /// Path to store directory
        #[arg(long, default_value = ".turn_store")]
        store: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { file, id, store } => {
            let source = fs::read_to_string(&file)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", file.display(), e))?;
            
            // Setup Store and Tools
            let store = FileStore::new(store);
            let tools = ToolRegistry::new();
            
            // Setup Runner
            let mut runner = Runner::new(store, tools);
            
            // Run
            match runner.run(&id, &source, Some(file.clone())) {
                Ok(result) => println!("{}", result),
                Err(e) => {
                    // Try to format error nicely if it's a lex/parse error
                    let msg = e.to_string();
                    let loc = if let Some(lex_err) = e.downcast_ref::<turn::lexer::LexError>() {
                        lex_err.offset().map(|o| turn::offset_to_line_col(&source, o))
                    } else if let Some(parse_err) = e.downcast_ref::<turn::parser::ParseError>() {
                        Some(turn::offset_to_line_col(&source, parse_err.offset()))
                    } else {
                        None
                    };
                    match loc {
                        Some((l, c)) => eprintln!("{}:{}:{}: {}", file.display(), l, c, msg),
                        None => eprintln!("{}: {}", file.display(), msg),
                    }
                    std::process::exit(1);
                }
            }
        }
        Commands::Serve { port, store } => {
            // Start async runtime for server
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            
            rt.block_on(async {
                if let Err(e) = turn::server::serve(port, store).await {
                    eprintln!("Server error: {}", e);
                }
            });
        }
    }
    Ok(())
}
