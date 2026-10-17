//! Ship Lab in the web chat (#11651): Lagrange 2's spacecraft document,
//! edited by talking.
//!
//! When this server is told where a Ship Lab runs (`OPENAGENTS_SHIP_LAB`, a
//! loopback URL: the Lab keeps its ships on the Mac it runs on), its tools
//! (`ship_list`, `ship_open`, `ship_edit`, `ship_versions`,
//! `ship_compare`; the Lab serves their declarations at `/api/tools`) join
//! the other Labs' in one chat turn ([`crate::labs`]): the model's typed
//! function choice is the route. No words of the message are matched here.
//! Every edit is checked by the Lab (`Spacecraft::edit`: mass budget,
//! thrust line, test burn, latch loads, the assembly tree), and a refusal
//! names the check.
//!
//! A tool's result carries a card (`"card": "ship_lab"`): the version and
//! its numbers, or why an edit was refused. [`thread_entry`] draws it under
//! the answer.

use std::time::Duration;

use maud::{Markup, html};
use serde_json::Value;

/// How long one Lab call may take.
const LAB_DEADLINE: Duration = Duration::from_secs(30);
/// The card kind a Ship Lab tool returns.
pub(crate) const CARD: &str = "ship_lab";

/// The Ship Lab this server talks to, when one is configured: loopback only.
pub(crate) fn lab_url() -> Option<String> {
    loopback(&std::env::var("OPENAGENTS_SHIP_LAB").ok()?)
}

fn loopback(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/').to_owned();
    (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")).then_some(url)
}

fn client(deadline: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder().timeout(deadline).build().ok()
}

async fn get_json(url: &str, deadline: Duration) -> Result<Value, String> {
    client(deadline)
        .ok_or("no HTTP client")?
        .get(url)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// One tool call on the Lab: its JSON result.
pub(crate) async fn run_tool(lab: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    if !name.starts_with("ship_")
        || !name[5..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_')
    {
        return Err(format!("{name} isn't a Ship Lab tool"));
    }
    client(LAB_DEADLINE)
        .ok_or("no HTTP client")?
        .post(format!("{lab}/api/tool/{name}"))
        .json(arguments)
        .send()
        .await
        .map_err(|e| format!("Ship Lab didn't answer ({})", e.without_url()))?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// The Ship Lab on this Mac, when one is configured and answering.
pub(crate) async fn manifest_here() -> Option<crate::labs::Lab> {
    let lab = lab_url()?;
    let manifest = async {
        let tools = get_json(&format!("{lab}/api/tools"), Duration::from_secs(3)).await?;
        let docs = get_json(&format!("{lab}/api/docs"), Duration::from_secs(3)).await?;
        Ok::<_, String>((
            tools["tools"].as_array().cloned().unwrap_or_default(),
            docs["documents"].as_array().cloned().unwrap_or_default(),
        ))
    };
    match manifest.await {
        Ok((tools, documents)) => Some(crate::labs::Lab {
            kind: crate::labs::Kind::Ship,
            url: lab,
            tools,
            documents,
            hosted: None,
            platform: None,
        }),
        Err(why) => {
            eprintln!("openagents-web: ship lab: not answering at {lab}: {why}");
            None
        }
    }
}

/// What the model is told about Ship Lab: the tools and the ships.
pub(crate) fn guidance(documents: &[Value]) -> String {
    let mut out = String::from(GUIDE);
    out.push_str("\nShips:\n");
    if documents.is_empty() {
        out.push_str("- none yet\n");
    }
    for doc in documents {
        out.push_str(&format!(
            "- {} \"{}\" ({} versions)\n",
            doc["id"].as_str().unwrap_or_default(),
            doc["title"].as_str().unwrap_or_default().replace('"', "'"),
            doc["versions"].as_u64().unwrap_or_default(),
        ));
    }
    out
}

const GUIDE: &str = "

This chat can change the spacecraft built in Verse's Lagrange 2 factory (the tug and any other ship in Ship Lab, on this Mac) with the ship_* tools. When the message is about the ship, the tug, its tanks, engine, habitat, keel, mass, delta-v or its checks, open it with ship_open to read its part ids, then change it with ship_edit, naming the parts. \"Bigger\" or \"smaller\" by a percentage is scale with factor (20% bigger is 1.2). When the person says \"the tank\" and the ship has two tanks, change both, as one ship_edit naming both, so they are checked together. Every edit is checked (mass budget, thrust line, test burn, latch loads); when one is refused, say plainly which check refused it and its numbers, and that the ship didn't change; don't try a different edit unless asked. The chat shows a card with the ship's numbers under your answer, so say in a sentence or two, as \"we\", what changed and the numbers the tool returned.
";

/// The Ship Lab cards a reply keeps.
pub(crate) fn cards(meta: &openagents_chat::router::Meta) -> Vec<Value> {
    meta.cards
        .iter()
        .filter(|card| card["card"] == CARD)
        .take(1)
        .cloned()
        .collect()
}

fn thousands(x: f64) -> String {
    let n = x.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// The card for a Ship Lab result: the version, its numbers, a refusal.
pub(crate) fn card_markup(card: &Value) -> Markup {
    let n = &card["numbers"];
    let f = |k: &str| n[k].as_f64().unwrap_or(0.0);
    html! {
        div.oa-ship-card style="margin:12px 0;padding:12px;border:1px solid rgba(127,127,127,.28);border-radius:12px;max-width:720px" {
            p style="margin:0 0 8px" {
                strong { (card["title"].as_str().unwrap_or_default()) " · v" (card["version"].as_u64().unwrap_or(1)) }
                " " span style="opacity:.7" { (card["summary"].as_str().unwrap_or_default()) }
            }
            @if let Some(refused) = card["refused"].as_str() {
                p style="margin:0 0 8px" { "Not changed. " (refused) }
            }
            ul style="margin:0;padding-left:20px;font-size:13px" {
                li { "Mass with margin " (thousands(f("budget_mass_kg"))) " kg of " (thousands(f("max_mass_kg"))) " kg (" (f("mass_margin_percent")) "% to spare)" }
                li { "Propellant " (thousands(f("propellant_kg"))) " kg · " (thousands(f("delta_v_m_s"))) " m/s of delta-v (the test burn needs " (thousands(f("test_burn_m_s"))) ")" }
                li { "Thrust line " (format!("{:.2}", f("thrust_offset_m"))) " m from the centre of mass (limit " (format!("{:.2}", f("max_thrust_offset_m"))) " m)" }
                @if let Some(part) = n["tightest_latch"]["part"].as_str() {
                    li { "Tightest latch: " (part) ", " (n["tightest_latch"]["margin_percent"].as_f64().unwrap_or(0.0)) "% under its rating" }
                }
            }
        }
    }
}

/// Where a reply with a Ship Lab card shows it.
pub(crate) fn thread_entry(reply: Option<&openagents_chat::router::Meta>) -> Markup {
    match reply.and_then(|meta| meta.cards.iter().find(|card| card["card"] == CARD)) {
        Some(card) => card_markup(card),
        None => html! {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_loopback_labs_are_used() {
        assert_eq!(
            loopback("http://127.0.0.1:8796/").as_deref(),
            Some("http://127.0.0.1:8796")
        );
        assert!(loopback("https://example.com").is_none());
    }

    #[test]
    fn the_card_reads_in_plain_words_and_says_when_nothing_changed() {
        let card = json!({"card": CARD, "doc": "tug", "title": "Lagrange tug", "version": 2,
            "summary": "Both tanks 20% bigger", "refused": "mass budget: 120870 kg with contingency exceeds the 115000 kg budget",
            "numbers": {"budget_mass_kg": 105000.0, "max_mass_kg": 115000.0, "mass_margin_percent": 8.7,
                "propellant_kg": 44000.0, "delta_v_m_s": 1903.2, "test_burn_m_s": 150.0,
                "thrust_offset_m": 0.1, "max_thrust_offset_m": 0.25,
                "tightest_latch": {"part": "keel-fwd", "margin_percent": 40.0}}});
        let text = card_markup(&card).into_string();
        assert!(text.contains("Not changed."), "{text}");
        assert!(text.contains("105,000 kg of 115,000 kg"), "{text}");
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
}
