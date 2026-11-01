use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    version,
    about = "Validate and freeze Simon Says workflow definitions (execution is not yet implemented)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Check a workflow definition and its authoritative references.
    Validate { template: PathBuf },
    /// Write a new immutable-by-convention, integrity-checked snapshot.
    Freeze {
        template: PathBuf,
        snapshot: PathBuf,
    },
    /// Verify a snapshot's recorded digest.
    Inspect { snapshot: PathBuf },
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Validate { template } => {
            let s = simon_says::Snapshot::create(&template)?;
            println!("valid: {}", s.sha256);
        }
        Command::Freeze { template, snapshot } => {
            let s = simon_says::Snapshot::create(&template)?;
            s.write_new(&snapshot)?;
            println!("frozen: {}", s.sha256);
        }
        Command::Inspect { snapshot } => {
            let s = simon_says::load_snapshot(&snapshot)?;
            println!("{}", serde_json::to_string_pretty(&s)?);
        }
    }
    Ok(())
}
