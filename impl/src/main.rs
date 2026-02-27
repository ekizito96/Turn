//! Turn CLI: turn run <file>

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tower_lsp::{LspService, Server};
use turn::analysis::Analysis;
use turn::{FileStore, Store, Runner, ToolRegistry};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "turn")]
#[command(about = "Turn - systems language for agentic computation")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Turn program
    Run {
        /// Path to .tn file
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

    /// Start Turn LSP server (stdio)
    Lsp,

    /// Add a package dependency
    Add {
        /// Package name (e.g. "std")
        name: String,

        /// URL to source file (e.g. raw github link)
        url: String,
    },

    /// Replay a historical Turn execution snapshot natively
    Replay {
        /// Agent ID to replay
        id: String,

        /// Path to store directory
        #[arg(long, default_value = ".turn_store")]
        store: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Lsp => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;

            rt.block_on(async {
                let stdin = tokio::io::stdin();
                let stdout = tokio::io::stdout();

                let (service, socket) = LspService::new(|client| turn::lsp::Backend {
                    client,
                    analysis: RwLock::new(Analysis::new()),
                    documents: RwLock::new(HashMap::new()),
                });
                Server::new(stdin, stdout, socket).serve(service).await;
            });
        }
        Commands::Run { file, id, store } => {
            let source_content = fs::read_to_string(&file)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", file.display(), e))?;

            // Setup Store and Tools
            let store = FileStore::new(store);
            let mut tools = ToolRegistry::new();
            turn::llm_tools::register_advanced_llm(&mut tools);

            // Setup Runner
            let mut runner = Runner::new(store, tools);

            if let Ok(key) = std::env::var("AZURE_OPENAI_API_KEY") {
                runner.inject_capability("AZURE_OPENAI_API_KEY", &key);
            }

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;

            rt.block_on(async {
                match runner.run(&id, &source_content, Some(file.clone())).await {
                    Ok(result) => println!("{}", result),
                    Err(e) => {
                        // Try to format error nicely if it's a lex/parse error
                        let msg = e.to_string();
                        let loc = if let Some(lex_err) = e.downcast_ref::<turn::lexer::LexError>() {
                            lex_err
                                .offset()
                                .map(|o| turn::offset_to_line_col(&source_content, o))
                        } else {
                            e.downcast_ref::<turn::parser::ParseError>()
                                .map(|parse_err| {
                                    turn::offset_to_line_col(&source_content, parse_err.offset())
                                })
                        };
                        match loc {
                            Some((l, c)) => eprintln!("{}:{}:{}: {}", file.display(), l, c, msg),
                            None => eprintln!("{}: {}", file.display(), msg),
                        }
                        std::process::exit(1);
                    }
                }
            });
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
        Commands::Add { name, url } => {
            let modules_dir = PathBuf::from(".turn_modules");
            if !modules_dir.exists() {
                fs::create_dir(&modules_dir)?;
            }

            println!("Fetching {} from {}...", name, url);
            let output = std::process::Command::new("curl")
                .arg("-s")
                .arg("-L")
                .arg(&url)
                .output()?;

            if !output.status.success() {
                anyhow::bail!("Failed to download package '{}'", name);
            }

            let response = String::from_utf8(output.stdout)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 from {}: {}", url, e))?;

            let file_path = modules_dir.join(format!("{}.tn", name));
            fs::write(&file_path, response)?;

            println!("Package '{}' installed to {}", name, file_path.display());
        }
        Commands::Replay { id, store } => {
            use std::io::Write;
            let db = FileStore::new(store);
            let versions = db.list_versions(&id)?;
            if versions.is_empty() {
                anyhow::bail!("No replay WAL history found for agent '{}'", id);
            }
            println!("Loaded {} historical snapshots for '{}'", versions.len(), id);
            
            let mut current_idx = versions.len() - 1;
            
            loop {
                let version = versions[current_idx];
                let state = db.load_version(&id, version)?.ok_or_else(|| anyhow::anyhow!("Missing version {} pointer corruption", version))?;
                
                println!("\n=== Replay Frame: {}/{} (WAL Version {}) ===", current_idx + 1, versions.len(), version);
                println!("Token Budget: {}", state.token_budget);
                println!("Stack Size: {}", state.stack.len());
                println!("Mailbox Messages: {}", state.mailbox.len());
                if let Some(top) = state.stack.last() {
                    println!("Top of Stack: {}", top);
                }
                
                print!("\n[n]ext, [p]revious, [s]tate detail, [q]uit: ");
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                match input.trim() {
                    "n" => {
                        if current_idx + 1 < versions.len() {
                            current_idx += 1;
                        } else {
                            println!("Already at the latest execution frame.");
                        }
                    }
                    "p" => {
                        if current_idx > 0 {
                            current_idx -= 1;
                        } else {
                            println!("Already at the earliest execution frame.");
                        }
                    }
                    "s" => {
                        println!("== State Detail Graph ==");
                        println!("{:#?}", state);
                    }
                    "q" => break,
                    _ => println!("Invalid boundary check. Use n, p, s, or q."),
                }
            }
        }
    }
    Ok(())
}
