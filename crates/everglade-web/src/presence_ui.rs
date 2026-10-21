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

/// The most keys one profile-name question asks about.
pub const NAMES_MAX: usize = 50;

/// The site's names question for `keys` (#12261).
#[must_use]
pub fn names_url(keys: &[String]) -> String {
    let keys: Vec<&str> = keys.iter().take(NAMES_MAX).map(String::as_str).collect();
    format!("/verse/names?keys={}", keys.join(","))
}

/// The names in the site's answer, by key; a key with no profile is left
/// out.
#[must_use]
pub fn read_names(json: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    value["names"]
        .as_object()
        .map(|names| {
            names
                .iter()
                .filter_map(|(key, entry)| {
                    let name = entry["name"].as_str()?.trim();
                    (!name.is_empty() && name.chars().count() <= 64)
                        .then(|| (key.clone(), name.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// An agent or pylon the Grid shows (#12313), from `/verse/places`.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub label: String,
    /// Where it stands on the plaza, in meters: `[x, z]`.
    pub at: [f32; 2],
}

/// The places in the site's answer, agents on an inner ring and pylons on
/// an outer one, each at an angle its id fixes, so every visitor sees them
/// in the same spots.
#[must_use]
pub fn read_places(json: &str) -> Vec<Place> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let ring = |list: &serde_json::Value, radius: f32, kind: &str| -> Vec<Place> {
        list.as_array()
            .into_iter()
            .flatten()
            .take(64)
            .filter_map(|entry| {
                let id = entry["id"].as_str()?;
                let name = entry["name"].as_str()?.trim();
                if name.is_empty() || name.chars().count() > 64 {
                    return None;
                }
                let turn = id.bytes().fold(2_166_136_261_u32, |h, b| {
                    (h ^ u32::from(b)).wrapping_mul(16_777_619)
                }) as f32
                    / u32::MAX as f32;
                let angle = turn * std::f32::consts::TAU;
                Some(Place {
                    label: format!("{kind} {name}"),
                    at: [radius * angle.cos(), radius * angle.sin()],
                })
            })
            .collect()
    };
    let mut places = ring(&value["agents"], 24.0, "Agent");
    places.extend(ring(&value["pylons"], 36.0, "Pylon"));
    places
}

/// The most characters of a name a tag shows.
pub const TAG_NAME_MAX: usize = 24;
/// How many characters of the npub after `npub1` a tag shows.
const NPUB_SHOWN: usize = 6;

/// A player's key as a tag shows it: `npub1` and the first few characters
/// of the rest, then an ellipsis. A key that isn't 64 hex characters shows
/// its own first characters.
#[must_use]
pub fn npub_prefix(pubkey: &str) -> String {
    let mut bytes = [0u8; 32];
    let parsed = pubkey.len() == 64
        && bytes.iter_mut().enumerate().all(|(i, byte)| {
            u8::from_str_radix(&pubkey[2 * i..2 * i + 2], 16)
                .map(|b| *byte = b)
                .is_ok()
        });
    if !parsed {
        let short: String = pubkey.chars().take(8).collect();
        return format!("{short}…");
    }
    let npub = nostr::nip19::encode_npub(&bytes);
    format!("{}…", &npub[..5 + NPUB_SHOWN])
}

/// The name a tag shows: trimmed, on one line, at most [`TAG_NAME_MAX`]
/// characters (the last an ellipsis when cut); `None` when empty.
#[must_use]
pub fn tag_name(name: &str) -> Option<String> {
    let one_line: String = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if one_line.is_empty() {
        return None;
    }
    if one_line.chars().count() <= TAG_NAME_MAX {
        return Some(one_line);
    }
    let cut: String = one_line.chars().take(TAG_NAME_MAX - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// The two lines over a player (#12406): the npub prefix on top, and the
/// name under it, from the player's own frames, else their profile's. With
/// no name, only the prefix.
#[must_use]
pub fn tag(own: Option<&str>, profile: Option<&str>, pubkey: &str) -> (String, Option<String>) {
    (
        npub_prefix(pubkey),
        own.and_then(tag_name)
            .or_else(|| profile.and_then(tag_name)),
    )
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
    fn places_stand_on_rings_where_their_ids_put_them() {
        let json = r#"{"agents":[{"id":"aa","name":"Helper"},{"id":"bb","name":" "}],"pylons":[{"id":"30200:k:box","name":"box-1"}]}"#;
        let places = read_places(json);
        assert_eq!(places.len(), 2);
        assert_eq!(places[0].label, "Agent Helper");
        let r0 = places[0].at[0].hypot(places[0].at[1]);
        let r1 = places[1].at[0].hypot(places[1].at[1]);
        assert!((r0 - 24.0).abs() < 0.01 && (r1 - 36.0).abs() < 0.01);
        assert_eq!(read_places(json), places, "the same spots every time");
        assert!(read_places("nope").is_empty());
    }

    #[test]
    fn profile_names_fill_tags_only_where_frames_gave_none() {
        let keys: Vec<String> = (0..60).map(|i| format!("{i:064x}")).collect();
        let url = names_url(&keys);
        assert!(url.starts_with("/verse/names?keys="));
        assert_eq!(url.matches(',').count(), NAMES_MAX - 1);
        let read =
            read_names(r#"{"names":{"aa":{"name":"Pat Verse"},"bb":{"name":"  "},"cc":{}}}"#);
        assert_eq!(read, vec![("aa".to_string(), "Pat Verse".to_string())]);
        assert!(read_names("not json").is_empty());
        let key = "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e";
        assert_eq!(
            tag(Some("Own"), Some("Profile"), key).1.as_deref(),
            Some("Own")
        );
        assert_eq!(
            tag(None, Some("Profile"), key).1.as_deref(),
            Some("Profile")
        );
        assert_eq!(
            tag(Some("  "), Some("Profile"), key).1.as_deref(),
            Some("Profile")
        );
        assert_eq!(tag(None, None, key), (npub_prefix(key), None));
    }

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
    #[test]
    fn tags_show_the_npub_prefix_over_a_bounded_name() {
        let key = "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e";
        let npub = nostr::nip19::encode_npub(&{
            let mut b = [0u8; 32];
            for (i, byte) in b.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&key[2 * i..2 * i + 2], 16).unwrap();
            }
            b
        });
        let prefix = npub_prefix(key);
        assert_eq!(prefix, format!("{}…", &npub[..11]));
        assert!(prefix.starts_with("npub1") && prefix.chars().count() == 12);
        // Not a key: its own start, never a panic.
        assert_eq!(npub_prefix("abc"), "abc…");
        assert_eq!(npub_prefix(&"zz".repeat(32)), "zzzzzzzz…");
        // Names stay on one line and short.
        assert_eq!(tag_name(" Pat \n Verse ").as_deref(), Some("Pat Verse"));
        assert_eq!(tag_name("\u{7}").as_deref(), None);
        let long = tag_name(&"x".repeat(80)).unwrap();
        assert_eq!(long.chars().count(), TAG_NAME_MAX);
        assert!(long.ends_with('…'));
        assert_eq!(
            tag_name(&"é".repeat(TAG_NAME_MAX)).unwrap().chars().count(),
            TAG_NAME_MAX
        );
    }
}
