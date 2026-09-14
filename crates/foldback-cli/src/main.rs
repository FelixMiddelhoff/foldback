// SPDX-License-Identifier: MIT OR Apache-2.0
mod report;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use report::{analyze, Report};

/// Foldback — a desync/divergence debugger for lockstep/rollback
/// multiplayer games.
#[derive(Parser)]
#[command(name = "foldback", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a human-readable report of a .foldback session file.
    Analyze {
        /// Path to the .foldback session file.
        file: PathBuf,
    },
    /// Exit non-zero if the session file contains a divergence — for CI.
    CiCheck {
        /// Path to the .foldback session file.
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze { file } => run_analyze(&file),
        Command::CiCheck { file } => run_ci_check(&file),
    }
}

fn run_analyze(file: &Path) -> ExitCode {
    match analyze(file) {
        Ok(report) => {
            print_report(&report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_ci_check(file: &Path) -> ExitCode {
    match analyze(file) {
        Ok(report) => match report.divergence {
            Some(d) => {
                eprintln!("DIVERGENCE DETECTED at tick {}", d.0);
                eprintln!();
                print_report_to_stderr(&report);
                ExitCode::from(1)
            }
            None => {
                println!(
                    "clean: no divergence detected across {} ticks",
                    report.tick_hash_count
                );
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn print_report(r: &Report) {
    println!("format_version : {}", r.header.format_version);
    println!("tick_rate_hz   : {}", r.header.tick_rate_hz);
    println!("peer_count     : {}", r.header.peer_count);
    println!("build_id       : {}", hex(&r.header.build_id));
    println!("ended cleanly  : {}", r.ended_cleanly);
    println!();
    println!("tick hashes    : {}", r.tick_hash_count);
    println!("entity hashes  : {}", r.entity_hash_count);
    println!("field hashes   : {}", r.field_hash_count);
    println!("snapshots      : {}", r.snapshot_count);
    println!("metadata       : {}", r.metadata_count);
    println!();
    match r.divergence {
        Some(d) => println!("divergence     : tick {}", d.0),
        None => println!("divergence     : none found"),
    }
}

fn print_report_to_stderr(r: &Report) {
    eprintln!("format_version : {}", r.header.format_version);
    eprintln!("tick_rate_hz   : {}", r.header.tick_rate_hz);
    eprintln!("peer_count     : {}", r.header.peer_count);
    eprintln!("tick hashes    : {}", r.tick_hash_count);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
