//! The golden records were captured from native Coder before extracting the demo.

use coder_demo_ui::{App, capture};
use coder_ui::demo::{Key, KeyCode, Screen};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "support/coder_noir.rs"]
mod coder_noir;

fn apply(app: &mut App, action: &Value) {
    if let Some(text) = action["paste"].as_str() {
        app.paste(text);
    } else if let Some(key) = action["key"].as_str() {
        let code = match key {
            "Down" => KeyCode::Down,
            "Up" => KeyCode::Up,
            "Left" => KeyCode::Left,
            "Enter" => KeyCode::Enter,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,
            "Escape" => KeyCode::Esc,
            "Tab" => KeyCode::Tab,
            "F2" => KeyCode::F(2),
            "Space" => KeyCode::Char(' '),
            _ => panic!("unknown golden key"),
        };
        app.key(Key {
            code,
            ctrl: false,
            alt: false,
            super_key: false,
            shift: false,
            release: false,
        });
    } else if let Some(index) = action["plugin"].as_u64() {
        app.open_plugins();
        app.plugins.selected = index as usize;
        app.open_plugin_settings();
    } else if action["models"] == true {
        app.screen = Screen::Conversation;
        app.open_models();
    } else if action["tick"] == true {
        app.tick();
    }
}

#[test]
fn all_original_native_cells_layout_and_cursors_match_with_coder_noir_roles() {
    let golden: Value =
        serde_json::from_str(include_str!("fixtures/native-d2fb95d33d.json")).unwrap();
    assert_eq!(golden["source"], "d2fb95d33d1d5c668be3d85c53c9bedaaab174af");
    let noir = coder_noir::expected(&golden);
    let mut dimensions = (0, 0);
    let mut app = App::default();
    let mut mismatches = Vec::new();
    for (index, reference) in golden["frames"].as_array().unwrap().iter().enumerate() {
        let width = reference["width"].as_u64().unwrap() as u16;
        let height = reference["height"].as_u64().unwrap() as u16;
        if dimensions != (width, height) {
            app = App::default();
            dimensions = (width, height);
        }
        for action in reference["actions"].as_array().unwrap() {
            apply(&mut app, action);
        }
        let frame = capture(&mut app, width, height);
        let cells = frame
            .cells
            .iter()
            .map(|c| {
                json!([
                    c.symbol,
                    format!("{:?}", c.foreground),
                    format!("{:?}", c.background),
                    c.modifiers.bits(),
                    c.skip
                ])
            })
            .collect::<Vec<_>>();
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&cells).unwrap()));
        let cursor = frame
            .cursor
            .map(|(x, y)| json!([x, y]))
            .unwrap_or(Value::Null);
        let rows = frame
            .cells
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol.as_str()).collect::<String>())
            .collect::<Vec<_>>();
        if digest != noir["frames"][index]["cell_sha256"].as_str().unwrap()
            || cursor != reference["cursor"]
            || app.scroll != reference["scroll"].as_u64().unwrap() as u16
        {
            let row_difference = rows
                .iter()
                .zip(reference["rows"].as_array().unwrap())
                .enumerate()
                .find(|(_, (actual, expected))| actual.as_str() != expected.as_str().unwrap())
                .map(|(row, (actual, expected))| {
                    format!(" row{row} actual={actual:?} expected={expected}")
                });
            mismatches.push(format!(
                "{} {width}×{height}: digest {digest}, cursor {cursor}, scroll {}{}",
                reference["name"],
                app.scroll,
                row_difference.unwrap_or_else(|| " (text rows equal; inspect cell style)".into())
            ));
        }
    }
    assert_eq!(golden["frames"].as_array().unwrap().len(), 93);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn memory_frames_keep_native_cursor_blink_and_escape_svg_text() {
    let mut app = App::default();
    app.paste("<svg> & \"quoted\"");
    let visible = capture(&mut app, 80, 24);
    assert!(visible.cursor.is_some());
    let svg = coder_demo_ui::svg(&visible);
    assert!(svg.contains("&lt;"));
    assert!(svg.contains("&amp;"));
    assert!(!svg.contains("<animate"));
    for _ in 0..4 {
        app.tick();
    }
    assert!(capture(&mut app, 80, 24).cursor.is_none());
}

#[test]
fn svg_copy_keeps_spaces_once_and_omits_only_wide_continuations() {
    use coder_demo_ui::{Cell, Color, Modifier, Snapshot};
    let cell = |symbol: &str| Cell {
        symbol: symbol.into(),
        foreground: Color::Rgb(200, 200, 200),
        background: Color::Rgb(10, 10, 10),
        modifiers: Modifier::empty(),
        skip: false,
    };
    let snapshot = Snapshot {
        width: 7,
        height: 1,
        cells: ["a", " ", "日", " ", "<", "&", "b"]
            .into_iter()
            .map(cell)
            .collect(),
        cursor: Some((6, 0)),
    };
    let svg = coder_demo_ui::svg(&snapshot);
    let row = svg.split("xml:space=\"preserve\">").nth(1).unwrap();
    let row = row.split("</text>").next().unwrap();
    let text = row
        .split("</tspan>")
        .filter_map(|span| span.rsplit_once('>').map(|(_, text)| text))
        .collect::<String>();
    assert_eq!(text, "a 日&lt;&amp;b");
    assert!(row.contains("x=\"9\" y=\"15\""));
    assert!(!row.contains("x=\"27\""));
    assert!(row.contains("x=\"36\" y=\"15\""));
    assert!(svg.contains("user-select:none;pointer-events:none"));
    assert_eq!(row.matches("<tspan ").count(), 6);
}

#[test]
fn svg_uses_coder_noir_cursor_and_ansi_without_recoloring_explicit_rgb() {
    use coder_demo_ui::{Cell, Color, Modifier, Snapshot};
    let snapshot = Snapshot {
        width: 3,
        height: 1,
        cells: [Color::Blue, Color::Indexed(6), Color::Rgb(12, 34, 56)]
            .into_iter()
            .map(|foreground| Cell {
                symbol: "x".into(),
                foreground,
                background: Color::Reset,
                modifiers: Modifier::empty(),
                skip: false,
            })
            .collect(),
        cursor: Some((0, 0)),
    };
    let svg = coder_demo_ui::svg(&snapshot);
    assert!(svg.contains(&format!("fill=\"#{:06x}\"", coder_ui::coder_noir::ANSI[4])));
    assert!(svg.contains(&format!("fill=\"#{:06x}\"", coder_ui::coder_noir::ANSI[6])));
    assert!(svg.contains("fill=\"#0c2238\""));
    assert!(svg.contains(&format!(
        "height=\"20\" fill=\"#{:06x}\"",
        coder_ui::coder_noir::CURSOR
    )));
}
