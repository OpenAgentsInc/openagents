//! `openagents psionic` — harness around the in-tree GGUF library.

use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::inference::{InferenceExit, admit_path, inspect};

#[derive(Args, Debug)]
pub struct PsionicArgs {
    #[command(subcommand)]
    pub action: PsionicAction,
}

#[derive(Subcommand, Debug)]
pub enum PsionicAction {
    /// Dump GGUF architecture, tensor names, sizes. No mmap of weights.
    Inspect {
        #[arg(help = "GGUF path")]
        artifact: PathBuf,
    },
    /// Admission only: family, digest, required tensors
    Admit {
        #[arg(help = "GGUF path")]
        artifact: PathBuf,
    },
    /// List compiled backends
    Backends,
    /// Library graph, feature flags, provenance pin
    Doctor,
}

pub fn run(args: PsionicArgs, json: bool) -> Result<(), InferenceExit> {
    match args.action {
        PsionicAction::Inspect { artifact } => inspect(&artifact, json),
        PsionicAction::Admit { artifact } => admit_path(&artifact, json),
        PsionicAction::Backends => {
            let metal = psionic_gguf::metal_wrap::metal_compiled();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "backends": if metal { vec!["cpu", "metal"] } else { vec!["cpu"] }
                    })
                );
            } else if metal {
                println!("cpu");
                println!("metal");
            } else {
                println!("cpu");
            }
            Ok(())
        }
        PsionicAction::Doctor => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "crate": "psionic-gguf",
                        "provenance": psionic_gguf::PROVENANCE_PIN,
                        "metal": psionic_gguf::metal_wrap::metal_compiled(),
                    })
                );
            } else {
                println!("psionic-gguf {}", psionic_gguf::PROVENANCE_PIN);
                println!("metal {}", psionic_gguf::metal_wrap::metal_compiled());
            }
            Ok(())
        }
    }
}
