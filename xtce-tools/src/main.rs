//! `xtce-tools` — command-line utilities for XTCE files.
//!
//! # Subcommands
//!
//! - **`gen-dissector`** — Generates a Wireshark Lua dissector that decodes
//!   UDP packets matching the leaf containers defined in the XTCE file.
//!   See [`dissector`].
//!
//! - **`gen-testdata`** — Generates a PCAP file with one synthetic UDP packet
//!   per leaf container, useful for testing the generated Lua dissector.
//!   See [`testdata`].
//!
//! Both subcommands share the container-flattening pipeline in [`layout`],
//! which resolves inheritance chains and computes absolute bit offsets for
//! every parameter field.

mod dissector;
mod layout;
mod testdata;

use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtce-tools", about = "XTCE utility tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a Wireshark Lua dissector from an XTCE file.
    GenDissector {
        /// Path to the XTCE XML file.
        input: PathBuf,
        /// UDP port the dissector listens on.
        #[arg(short, long, default_value = "4321")]
        port: u16,
        /// Output path (defaults to <input_stem>.lua).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate PCAP test data from an XTCE file (one packet per leaf container).
    GenTestdata {
        /// Path to the XTCE XML file.
        input: PathBuf,
        /// UDP destination port written into each packet.
        #[arg(short, long, default_value = "4321")]
        port: u16,
        /// Output path (defaults to <input_stem>_test.pcap).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenDissector { input, port, output } => {
            let ss = load_xtce(&input);
            let leaves = layout::find_leaf_containers(&ss);

            if leaves.is_empty() {
                eprintln!("Warning: no leaf containers found in {:?}", input);
            } else {
                eprintln!("Found {} leaf container(s).", leaves.len());
            }

            let lua = dissector::generate_lua(&leaves, port);
            let out_path = output.unwrap_or_else(|| default_output(&input, ".lua"));
            fs::write(&out_path, lua.as_bytes()).unwrap_or_else(|e| {
                eprintln!("Error writing {:?}: {e}", out_path);
                process::exit(1);
            });
            eprintln!("Dissector written to {:?}", out_path);
        }
        Commands::GenTestdata { input, port, output } => {
            let ss = load_xtce(&input);
            let leaves = layout::find_leaf_containers(&ss);

            if leaves.is_empty() {
                eprintln!("Warning: no leaf containers found in {:?}", input);
            } else {
                eprintln!("Found {} leaf container(s).", leaves.len());
            }

            let pcap = testdata::generate_pcap(&leaves, port);
            let out_path = output.unwrap_or_else(|| default_output(&input, "_test.pcap"));
            fs::write(&out_path, &pcap).unwrap_or_else(|e| {
                eprintln!("Error writing {:?}: {e}", out_path);
                process::exit(1);
            });
            eprintln!(
                "PCAP ({} packets, {} bytes) written to {:?}",
                leaves.len(),
                pcap.len(),
                out_path
            );
        }
    }
}

/// Parse an XTCE file, printing an error and exiting with code 1 on failure.
fn load_xtce(path: &Path) -> xtce_core::model::SpaceSystem {
    xtce_core::parser::parse_file(path).unwrap_or_else(|e| {
        eprintln!("Error parsing {:?}: {e}", path);
        process::exit(1);
    })
}

/// Derive a default output path by replacing the input file's extension with
/// `suffix` (e.g. `.lua` or `.pcap`), in the same directory.
fn default_output(input: &Path, suffix: &str) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let parent = input.parent().unwrap_or(Path::new("."));
    parent.join(format!("{stem}{suffix}"))
}
