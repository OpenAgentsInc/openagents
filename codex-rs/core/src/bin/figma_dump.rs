use codex_core::figma_tools::{ListSubnodesArgs, handle_list_subnodes};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        eprintln!("Usage: figma_dump <figma_url> <node_id> [max_depth] [max_total] [output.json]" );
        std::process::exit(2);
    }
    let figma_url = args.remove(0);
    let node_id = args.remove(0);
    let max_depth = args.get(0).and_then(|s| s.parse::<u32>().ok());
    let max_total = args.get(1).and_then(|s| s.parse::<usize>().ok());
    let output = args.get(2).map(|s| PathBuf::from(s));

    let base_dir = std::env::current_dir().ok();
    let res = handle_list_subnodes(
        ListSubnodesArgs { figma_url, node_id, max_depth, max_total },
        base_dir.as_deref(),
    ).await?;

    let text = serde_json::to_string_pretty(&res)?;
    if let Some(path) = output {
        std::fs::write(&path, text.as_bytes())?;
        println!("Wrote {}", path.display());
    } else {
        println!("{}", text);
    }
    Ok(())
}

