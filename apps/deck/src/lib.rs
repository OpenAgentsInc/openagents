#[cfg(target_arch = "wasm32")]
mod app;
pub mod deck;
#[cfg(target_arch = "wasm32")]
mod input;
#[cfg(any(target_arch = "wasm32", test))]
mod state;

use deck::model::Deck;
use deck::parser::DeckParser;

const EMBEDDED_DECK_SOURCE: &str = include_str!("../content/five-markets.deck.md");

pub fn parse_embedded_deck() -> Result<Deck, String> {
    DeckParser::new().parse(EMBEDDED_DECK_SOURCE)
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = boot().await {
            input::report_boot_error(&error);
        }
    });
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn boot() -> Result<(), String> {
    let deck = parse_embedded_deck()?;
    app::boot_browser_app(deck).await
}

#[cfg(test)]
mod tests {
    use super::parse_embedded_deck;

    #[test]
    fn embedded_deck_parses() {
        let parsed = parse_embedded_deck();
        assert!(parsed.is_ok(), "embedded deck should parse: {parsed:?}");
        let Ok(deck) = parsed else {
            return;
        };

        assert_eq!(deck.metadata.title, "Autopilot + Five Markets");
        assert_eq!(deck.slides.len(), 6);
    }
}
