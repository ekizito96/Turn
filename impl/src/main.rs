//! Turn CLI: turn run <file>

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use tower_lsp::{LspService, Server};
use turn::analysis::Analysis;
use turn::{FileStore, Runner, ToolRegistry};

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

        /// Deny host filesystem, environment, network tools, imports, and identity grants
        #[arg(long)]
        sandbox: bool,
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

    /// Inspect a suspended or saved Turn agent's VM state
    Inspect {
        /// Agent ID to inspect
        id: String,

        /// Path to store directory
        #[arg(long, default_value = ".turn_store")]
        store: PathBuf,
    },

    /// List durable effect journal records for an agent
    Effects {
        /// Agent ID whose journal should be read
        id: String,

        /// Path to store directory
        #[arg(long, default_value = ".turn_store")]
        store: PathBuf,

        /// Include effect arguments and result/error payloads
        #[arg(long)]
        show_payloads: bool,

        /// Emit JSON instead of the human-readable table
        #[arg(long)]
        json: bool,
    },

    /// Add a package dependency
    Add {
        /// Package name (e.g. "std")
        name: String,

        /// URL to source file (e.g. raw github link)
        url: String,
    },

    /// Check provider availability and credentials
    Doctor,
}

fn credential_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "gemini" => Some("GEMINI_API_KEY"),
        "grok" => Some("XAI_API_KEY"),
        "azure_openai" | "azure_openai_responses" => Some("AZURE_OPENAI_KEY"),
        "azure_anthropic" => Some("AZURE_ANTHROPIC_KEY"),
        "typesafe" => Some("TYPESAFE_API_KEY"),
        "mock" | "ollama" => None,
        _ => None,
    }
}

fn print_provider_status(kind: &str, provider: &str) {
    let source = if turn::tools::bundled_provider_available(provider) {
        "bundled"
    } else {
        "external"
    };
    println!("{kind}: {provider} ({source} driver)");

    if let Some(variable) = credential_for_provider(provider) {
        let status = std::env::var(variable)
            .map(|value| {
                if value.trim().is_empty() {
                    "missing"
                } else {
                    "set"
                }
            })
            .unwrap_or("missing");
        println!("  {variable}: {status}");
    }
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
        Commands::Run {
            file,
            id,
            store,
            sandbox,
        } => {
            let source_content = fs::read_to_string(&file)
                .map_err(|e| anyhow::anyhow!("failed to read {}: {}", file.display(), e))?;

            // Setup Store and Tools
            let store = FileStore::new(store);
            let tools = if sandbox {
                ToolRegistry::playground()
            } else {
                ToolRegistry::new()
            };

            // Setup Runner
            let mut runner = Runner::new(store, tools);
            if sandbox {
                runner = runner.sandboxed();
            }

            // Run
            match runner.run(&id, &source_content, Some(file.clone())) {
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
        Commands::Inspect { id, store } => {
            use turn::store::Store;
            let file_store = FileStore::new(store.clone());
            match file_store.load(&id) {
                Ok(Some(state)) => {
                    let cyan = "\x1b[1;36m";
                    let yellow = "\x1b[1;33m";
                    let green = "\x1b[1;32m";
                    let magenta = "\x1b[1;35m";
                    let dim = "\x1b[2;37m";
                    let reset = "\x1b[0m";

                    println!(
                        "{cyan}========================================================={reset}"
                    );
                    println!(
                        "⚡ {}TURN VM INSPECTOR: {}{reset} (PID: {})",
                        yellow, id, state.pid
                    );
                    println!(
                        "{cyan}========================================================={reset}"
                    );

                    let ip = state
                        .frames
                        .last()
                        .map(|f| f.ip.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    println!("STATUS: Suspended (Instruction Pointer: {})", ip);
                    println!("GAS REMAINING: {} ops", state.gas_remaining);
                    if let Some(pending) = &state.runtime.pending_effect {
                        println!("PENDING EFFECT: {}", pending.tool_name);
                    }

                    println!("\n{cyan}[1] 🧠 THE TRIPARTITE CONTEXT{reset}");
                    println!(
                        "{dim}---------------------------------------------------------{reset}"
                    );

                    let ctx = &state.runtime.context;
                    println!("{green}[P0: SYSTEM / PRIMACY] (Locked){reset}");
                    if ctx.p0_system.is_empty() {
                        println!("  {dim}- (Empty){reset}");
                    } else {
                        for item in &ctx.p0_system {
                            println!("  - {}", item);
                        }
                    }

                    println!(
                        "\n{dim}[P2: EPISODIC / MIDDLE] (Demoted - Low Attention Zone){reset}"
                    );
                    if ctx.p2_episodic.is_empty() {
                        println!("  {dim}- (Empty){reset}");
                    } else {
                        for item in &ctx.p2_episodic {
                            println!("  {dim}- {}{reset}", item);
                        }
                    }

                    println!("\n{magenta}[P1: WORKING / RECENCY] (High Attention Zone){reset}");
                    if ctx.p1_working.is_empty() {
                        println!("  {dim}- (Empty){reset}");
                    } else {
                        for item in &ctx.p1_working {
                            println!("  - {}", item);
                        }
                    }

                    println!("\n{cyan}[2] 💾 DURABLE MEMORY (remember / recall){reset}");
                    println!(
                        "{dim}---------------------------------------------------------{reset}"
                    );
                    if state.runtime.memory.is_empty() {
                        println!("  {dim}(Empty){reset}");
                    } else {
                        for (k, v) in &state.runtime.memory {
                            println!("  {} => {}", k, v);
                        }
                    }

                    println!("\n{cyan}[3] 📬 ACTOR MAILBOX{reset}");
                    println!(
                        "{dim}---------------------------------------------------------{reset}"
                    );
                    if state.mailbox.is_empty() {
                        println!("  {dim}(Empty){reset}");
                    } else {
                        println!("  {} message(s) queued:", state.mailbox.len());
                        for (i, msg) in state.mailbox.iter().enumerate() {
                            println!("  [Message {}] {}", i, msg);
                        }
                    }

                    println!("\n{cyan}[4] 📊 COGNITIVE BELIEF STATE{reset}");
                    println!(
                        "{dim}---------------------------------------------------------{reset}"
                    );
                    if let Some(conf) = state.runtime.last_confidence {
                        let color = if conf >= 0.85 {
                            green
                        } else if conf >= 0.70 {
                            yellow
                        } else {
                            "\x1b[1;31m"
                        };
                        println!("  Last Inference Confidence: {}{:.2}{reset}", color, conf);
                    } else {
                        println!("  {dim}No inference executed yet.{reset}");
                    }

                    println!("\n{cyan}[5] 🧬 SUPERVISOR TREE{reset}");
                    println!(
                        "{dim}---------------------------------------------------------{reset}"
                    );
                    println!(
                        "  [PID {}] {yellow}{}{reset} (Status: Suspended)",
                        state.pid, id
                    );
                    for p in &state.scheduler {
                        let status = if p.frames.is_empty() {
                            "Complete"
                        } else {
                            "Running/Yielded"
                        };
                        println!("   ├── [PID {}] child_process (Status: {})", p.pid, status);
                    }
                    println!(
                        "{cyan}========================================================={reset}"
                    );
                }
                Ok(None) => {
                    eprintln!(
                        "Error: No saved state found for agent '{}' in {}",
                        id,
                        store.display()
                    );
                }
                Err(e) => {
                    eprintln!("Error loading state: {}", e);
                }
            }
        }
        Commands::Doctor => {
            let inference_provider =
                std::env::var("TURN_LLM_PROVIDER").unwrap_or_else(|_| "openai".to_string());
            let decision_provider =
                std::env::var("TURN_DECISION_PROVIDER").unwrap_or_else(|_| "typesafe".to_string());

            println!("Turn {VERSION}");
            print_provider_status("Inference", &inference_provider);
            print_provider_status("Decision", &decision_provider);
        }
        Commands::Effects {
            id,
            store,
            show_payloads,
            json,
        } => {
            use turn::store::{EffectOutcome, Store};

            let file_store = FileStore::new(store);
            let records = file_store.list_effects(&id)?;
            if json {
                let rows = records
                    .iter()
                    .map(|record| {
                        let (status, cost) = match &record.outcome {
                            EffectOutcome::Success { cost, .. } => ("success", Some(*cost)),
                            EffectOutcome::Failure { .. } => ("failure", None),
                        };
                        let mut row = serde_json::json!({
                            "effect_id": record.effect_id,
                            "tool_name": record.tool_name,
                            "status": status,
                            "cost": cost,
                            "completed_at_ms": record.completed_at_ms,
                        });
                        if show_payloads {
                            row["arg"] = serde_json::to_value(&record.arg)?;
                            match &record.outcome {
                                EffectOutcome::Success { value, .. } => {
                                    row["result"] = serde_json::to_value(value)?;
                                }
                                EffectOutcome::Failure { error } => {
                                    row["error"] = serde_json::json!(error);
                                }
                            }
                        }
                        Ok::<_, serde_json::Error>(row)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else if records.is_empty() {
                println!("No effects recorded for agent '{id}'.");
            } else {
                println!("EFFECT ID\tTOOL\tSTATUS\tCOST\tCOMPLETED (ms)");
                for record in records {
                    let (status, cost) = match &record.outcome {
                        EffectOutcome::Success { cost, .. } => ("success", cost.to_string()),
                        EffectOutcome::Failure { .. } => ("failure", "-".to_string()),
                    };
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        record.effect_id, record.tool_name, status, cost, record.completed_at_ms
                    );
                    if show_payloads {
                        println!("  arg: {}", record.arg);
                        match record.outcome {
                            EffectOutcome::Success { value, .. } => {
                                println!("  result: {value}");
                            }
                            EffectOutcome::Failure { error } => {
                                println!("  error: {error}");
                            }
                        }
                    }
                }
            }
        }
        Commands::Add { name, url } => {
            let modules_dir = PathBuf::from(".turn_modules");
            if !modules_dir.exists() {
                fs::create_dir(&modules_dir)?;
            }

            println!("Fetching {} from {}...", name, url);
            let response = reqwest::blocking::get(&url)?.error_for_status()?.text()?;

            let file_path = modules_dir.join(format!("{}.tn", name));
            fs::write(&file_path, response)?;

            println!("Package '{}' installed to {}", name, file_path.display());
        }
    }
    Ok(())
}
