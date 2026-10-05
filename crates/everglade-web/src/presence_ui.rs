//! What the browser Grid's presence reads from the page's URL, and where
//! its player card's buttons sit. Plain data, so it is tested natively.

/// Space inside the card and around its buttons, in CSS pixels.
pub const CARD_PAD: f32 = 10.0;
const CARD_SIZE: [f32; 2] = [264.0, 96.0];
const CARD_BUTTON_HEIGHT: f32 = 34.0;

/// A player card's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Block,
    Mute,
    Close,
}

/// What the page's URL asks of presence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// No presence session at all.
    pub offline: bool,
    /// The relay, when not the public one.
    pub relay: Option<String>,
    /// A display name to take and keep.
    pub name: Option<String>,
}

impl Options {
    /// Reads `offline`, `relay=`, and `name=` from a URL's query string.
    #[must_use]
    pub fn parse(query: &str) -> Self {
        let mut options = Self::default();
        for part in query.trim_start_matches('?').split('&') {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            let value = decode(value);
            match key {
                "offline" => options.offline = true,
                "relay" if !value.is_empty() => options.relay = Some(value),
                "name" if !value.is_empty() => options.name = Some(value),
                _ => {}
            }
        }
        options
    }
}

/// Percent-decodes a query value, with `+` as a space; a malformed escape
/// stays as written.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The card, `[x, y, width, height]` in CSS pixels: centered near the top.
pub fn card_rect(size: [f32; 2]) -> [f32; 4] {
    let width = CARD_SIZE[0].min(size[0] - 2.0 * CARD_PAD).max(0.0);
    [(size[0] - width) / 2.0, 72.0, width, CARD_SIZE[1]]
}

pub fn card_buttons(size: [f32; 2]) -> [(Button, [f32; 4]); 3] {
    let [x, y, width, height] = card_rect(size);
    let button = (width - 4.0 * CARD_PAD) / 3.0;
    let top = y + height - CARD_PAD - CARD_BUTTON_HEIGHT;
    let at = |n: f32| {
        [
            x + CARD_PAD + n * (button + CARD_PAD),
            top,
            button,
            CARD_BUTTON_HEIGHT,
        ]
    };
    [
        (Button::Block, at(0.0)),
        (Button::Mute, at(1.0)),
        (Button::Close, at(2.0)),
    ]
}

pub fn card_hit(size: [f32; 2], at: [f32; 2]) -> Option<Button> {
    card_buttons(size)
        .into_iter()
        .find(|(_, [x, y, w, h])| (*x..=x + w).contains(&at[0]) && (*y..=y + h).contains(&at[1]))
        .map(|(button, _)| button)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_names_the_relay_and_the_player() {
        let options =
            Options::parse("?zone=grid&gl&relay=ws%3A%2F%2F127.0.0.1%3A7447&name=Browser+One");
        assert_eq!(options.relay.as_deref(), Some("ws://127.0.0.1:7447"));
        assert_eq!(options.name.as_deref(), Some("Browser One"));
        assert!(!options.offline);
        assert!(Options::parse("zone=grid&offline").offline);
        assert_eq!(Options::parse("name=%zz").name.as_deref(), Some("%zz"));
    }

    #[test]
    fn card_buttons_sit_inside_the_card() {
        let size = [1280.0, 720.0];
        let [x, y, w, h] = card_rect(size);
        for (button, [bx, by, bw, bh]) in card_buttons(size) {
            assert!(bx >= x && by >= y && bx + bw <= x + w && by + bh <= y + h);
            assert_eq!(card_hit(size, [bx + bw / 2.0, by + bh / 2.0]), Some(button));
        }
        assert_eq!(card_hit(size, [0.0, 0.0]), None);
    }
}
