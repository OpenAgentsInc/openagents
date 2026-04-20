use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use wgpui::inspect_rive_bytes;

#[derive(Debug, Parser)]
#[command(about = "Inspect a .riv file for artboards, scenes, and inputs")]
struct Args {
    path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.path)
        .with_context(|| format!("failed to read {}", args.path.display()))?;
    let artboards = inspect_rive_bytes(&bytes).map_err(anyhow::Error::from)?;

    println!("file: {}", args.path.display());
    if artboards.is_empty() {
        println!("no artboards found");
        return Ok(());
    }

    for artboard in artboards {
        println!("artboard[{}]", artboard.index);
        if artboard.state_machines.is_empty() {
            println!("  state_machines: none");
        } else {
            println!("  state_machines:");
            for scene in artboard.state_machines {
                match scene.duration_seconds {
                    Some(duration) => {
                        println!(
                            "    - index:{} name:{} duration:{duration:.3}s",
                            scene.index, scene.name
                        );
                    }
                    None => println!("    - index:{} name:{}", scene.index, scene.name),
                }
            }
        }

        if artboard.linear_animations.is_empty() {
            println!("  linear_animations: none");
        } else {
            println!("  linear_animations:");
            for scene in artboard.linear_animations {
                match scene.duration_seconds {
                    Some(duration) => {
                        println!(
                            "    - index:{} name:{} duration:{duration:.3}s",
                            scene.index, scene.name
                        );
                    }
                    None => println!("    - index:{} name:{}", scene.index, scene.name),
                }
            }
        }
    }

    Ok(())
}
