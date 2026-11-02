use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use simon_says::{controller, routing::Status};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    version,
    about = "Run local sequential workflows with durable action records"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Check a workflow definition and its authoritative references.
    Validate { template: PathBuf },
    /// Write a new integrity-checked snapshot.
    Freeze {
        template: PathBuf,
        snapshot: PathBuf,
    },
    /// Inspect a snapshot and verify its recorded digest.
    Inspect { snapshot: PathBuf },
    /// Start a new run in a new directory outside the project checkout.
    Run {
        template: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        run_dir: PathBuf,
        /// Freeze this UTF-8 file as the original task context for every role.
        #[arg(long)]
        context: Option<PathBuf>,
    },
    /// Show durable state and the last safe checkpoint.
    Status { run_dir: PathBuf },
    /// Continue a ready run. Uncertain preparation or running actions are never replayed.
    Resume { run_dir: PathBuf },
    /// Approve the exact reviewed checkpoint and continue the frozen route.
    Approve {
        run_dir: PathBuf,
        #[arg(long)]
        expected_commit: String,
        #[arg(long)]
        expected_digest: String,
        #[arg(long)]
        expected_approval: String,
        #[arg(long)]
        note: Option<String>,
    },
    /// Recover an interrupted initialization after verifying the recorded checkout.
    RecoverInit {
        run_dir: PathBuf,
        /// Confirm checkout processes have exited and external effects are reconciled.
        #[arg(long)]
        acknowledge_external_effects: bool,
        /// Explicitly repeat checkout only if the recorded target is absent and unregistered.
        #[arg(long, conflicts_with = "adopt_existing")]
        retry_create: bool,
        /// Explicitly adopt an existing verified checkout.
        #[arg(long, conflicts_with = "retry_create")]
        adopt_existing: bool,
    },
    /// Remove only clean, verified review worktrees from a completed run.
    CleanupReviews {
        run_dir: PathBuf,
        #[arg(long)]
        acknowledge_processes_exited: bool,
    },
    /// Explicitly reset a controlled worktree after uncertain preparation or execution.
    Reset {
        run_dir: PathBuf,
        #[arg(long)]
        acknowledge_external_effects: bool,
    },
    /// Adopt a recorded Reviewer that echoed its exact inspected candidate, without rerunning it.
    AdoptReviewer {
        run_dir: PathBuf,
        #[arg(long)]
        expected_action: String,
        #[arg(long)]
        expected_candidate: String,
        #[arg(long)]
        acknowledge_external_effects: bool,
        #[arg(long)]
        reconcile_check: bool,
    },
    /// Adopt a recorded completed Worker after safely reconciling its checkout, then inspect before resuming.
    AdoptWorker {
        run_dir: PathBuf,
        #[arg(long)]
        expected_action: String,
        #[arg(long)]
        expected_candidate: String,
        /// Confirm processes have exited and external effects have been reconciled.
        #[arg(long)]
        acknowledge_external_effects: bool,
        /// Explicitly repeat a previously interrupted or failed adoption check after reconciliation.
        #[arg(long)]
        reconcile_check: bool,
    },
}
fn print_result(record: simon_says::store::RunRecord) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&record)?);
    if record.state.status == Status::Stopped {
        bail!("workflow stopped; inspect the recorded reason")
    }
    Ok(())
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
        Command::Run {
            template,
            project,
            run_dir,
            context,
        } => {
            controller::initialize_with_context(&template, &project, &run_dir, context.as_deref())?;
            print_result(controller::execute(&run_dir)?)?;
        }
        Command::Status { run_dir } => println!(
            "{}",
            serde_json::to_string_pretty(&controller::status(&run_dir)?)?
        ),
        Command::Resume { run_dir } => print_result(controller::execute(&run_dir)?)?,
        Command::Approve {
            run_dir,
            expected_commit,
            expected_digest,
            expected_approval,
            note,
        } => {
            let record = controller::approve(
                &run_dir,
                &expected_commit,
                &expected_digest,
                &expected_approval,
                note,
            )?;
            if record.state.status == Status::Ready {
                print_result(controller::execute(&run_dir)?)?;
            } else {
                print_result(record)?;
            }
        }
        Command::RecoverInit {
            run_dir,
            acknowledge_external_effects,
            retry_create,
            adopt_existing,
        } => {
            controller::recover_initialization(
                &run_dir,
                acknowledge_external_effects,
                retry_create,
                adopt_existing,
            )?;
            println!("initialization recovered; inspect status before resuming");
        }
        Command::CleanupReviews {
            run_dir,
            acknowledge_processes_exited,
        } => {
            if !acknowledge_processes_exited {
                bail!("confirm all action processes and descendants have exited with --acknowledge-processes-exited");
            }
            let count = simon_says::cleanup::cleanup_reviews(&run_dir)?;
            println!("removed {count} verified review worktree(s); action evidence and worker checkout retained");
        }
        Command::Reset {
            run_dir,
            acknowledge_external_effects,
        } => {
            controller::reset(&run_dir, acknowledge_external_effects)?;
            println!("reset to recorded checkpoint; inspect status before resuming");
        }
        Command::AdoptReviewer {
            run_dir,
            expected_action,
            expected_candidate,
            acknowledge_external_effects,
            reconcile_check,
        } => {
            let record = simon_says::adopt_reviewer::adopt(
                &run_dir,
                &expected_action,
                &expected_candidate,
                acknowledge_external_effects,
                reconcile_check,
            )?;
            print_result(record)?;
        }
        Command::AdoptWorker {
            run_dir,
            expected_action,
            expected_candidate,
            acknowledge_external_effects,
            reconcile_check,
        } => {
            let record = simon_says::adopt_worker::adopt(
                &run_dir,
                &expected_action,
                &expected_candidate,
                acknowledge_external_effects,
                reconcile_check,
            )?;
            print_result(record)?;
        }
    }
    Ok(())
}
