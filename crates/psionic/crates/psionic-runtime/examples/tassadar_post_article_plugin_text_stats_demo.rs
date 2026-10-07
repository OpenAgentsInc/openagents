use psionic_runtime::{invoke_text_stats_json_packet, TextStatsConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packet = serde_json::to_vec(&serde_json::json!({
        "text": "alpha beta\n\ngamma delta"
    }))?;
    let outcome = invoke_text_stats_json_packet("json", &packet, &TextStatsConfig::default());
    let response = outcome.response.ok_or("missing response")?;
    println!(
        "tool=plugin_text_stats schema={}",
        outcome.receipt.output_or_refusal_schema_id
    );
    println!("{}", serde_json::to_string_pretty(&response)?);
    println!(
        "receipt_id={} receipt_digest={}",
        outcome.receipt.receipt_id, outcome.receipt.receipt_digest
    );
    Ok(())
}
