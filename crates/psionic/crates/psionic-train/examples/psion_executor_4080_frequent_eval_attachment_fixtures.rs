use std::path::Path;

use psionic_train::write_builtin_executor_4080_frequent_eval_attachment_packet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packet =
        write_builtin_executor_4080_frequent_eval_attachment_packet(workspace_root.as_path())?;
    println!(
        "wrote psion executor 4080 frequent-eval attachment packet {}",
        packet.packet_digest
    );
    Ok(())
}
