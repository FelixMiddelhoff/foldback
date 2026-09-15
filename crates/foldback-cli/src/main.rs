// SPDX-License-Identifier: MIT OR Apache-2.0
mod lint;
mod report;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use report::{analyze, extract_schema, Report};

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
    /// List every `#[derive(FoldbackHash)]`-tracked type in a source
    /// tree — reflective-hashing visibility tooling (plan §4). Bevy/Rust
    /// only for now; Unity/Godot scanners aren't built yet.
    Lint {
        /// Which binding's marker syntax to scan for.
        #[arg(long, value_enum)]
        engine: Engine,
        /// Directory to scan (recursively) for `.rs` files.
        path: PathBuf,
    },
    /// Compare two `.foldback` files' recorded reflective-hashing schemas
    /// and report any type whose tagged field set changed between them —
    /// schema-drift detection (foldback-reflective-hashing.md §7). Exits
    /// non-zero if drift is found, for CI use the same way `ci-check`
    /// gates on divergence.
    SchemaDiff {
        /// The earlier `.foldback` recording (e.g. an older build).
        before: PathBuf,
        /// The later `.foldback` recording (e.g. the current build, or
        /// the session actually being analyzed).
        after: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Engine {
    Bevy,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze { file } => run_analyze(&file),
        Command::CiCheck { file } => run_ci_check(&file),
        Command::Lint { engine, path } => run_lint(engine, &path),
        Command::SchemaDiff { before, after } => run_schema_diff(&before, &after),
    }
}

fn run_schema_diff(before: &Path, after: &Path) -> ExitCode {
    let before_schema = match extract_schema(before) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let after_schema = match extract_schema(after) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let drift = foldback_core::schema::detect_drift(&before_schema, &after_schema);
    if drift.is_empty() {
        println!(
            "no schema drift across {} type(s) common to both files",
            before_schema
                .keys()
                .filter(|k| after_schema.contains_key(*k))
                .count()
        );
        ExitCode::SUCCESS
    } else {
        for d in &drift {
            println!("SCHEMA DRIFT: {}", d.type_name);
            println!("  before : {}", d.before);
            println!("  after  : {}", d.after);
        }
        ExitCode::from(1)
    }
}

fn run_lint(engine: Engine, path: &Path) -> ExitCode {
    let Engine::Bevy = engine;
    match lint::scan_bevy(path) {
        Ok(types) if types.is_empty() => {
            println!(
                "no #[derive(FoldbackHash)] types found under {}",
                path.display()
            );
            ExitCode::SUCCESS
        }
        Ok(types) => {
            for t in &types {
                println!("{} ({})", t.name, t.file.display());
                println!("  tracked   : {}", join_or_none(&t.tracked));
                println!("  untracked : {}", join_or_none(&t.untracked));
                println!();
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn join_or_none(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
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
