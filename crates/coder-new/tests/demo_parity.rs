//! Native Demo remains tied to the independently retained original terminal frames.
use coder_new::{App, Mode, Screen, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "../../coder-demo-ui/tests/support/coder_noir.rs"]
mod coder_noir;

fn apply(app: &mut App, action: &Value) {
    if let Some(text) = action["paste"].as_str() {
        app.handle(Event::Paste(text.into()));
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
            _ => panic!("unknown original reference key"),
        };
        app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
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
fn native_demo_projection_preserves_all_pre_extraction_cells_and_cursors() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../coder-demo-ui/tests/fixtures/native-d2fb95d33d.json"
    ))
    .unwrap();
    let noir = coder_noir::expected(&golden);
    let mut app = App::default();
    assert!(app.mode == Mode::Demo);
    let mut dimensions = (0, 0);
    let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
    for (index, reference) in golden["frames"].as_array().unwrap().iter().enumerate() {
        let width = reference["width"].as_u64().unwrap() as u16;
        let height = reference["height"].as_u64().unwrap() as u16;
        if dimensions != (width, height) {
            app = App::default();
            terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            dimensions = (width, height);
        }
        for action in reference["actions"].as_array().unwrap() {
            apply(&mut app, action);
        }
        let original = terminal
            .draw(|frame| ui::render(frame, &mut app))
            .unwrap()
            .buffer
            .clone();
        #[allow(deprecated)]
        let cells = original
            .content
            .iter()
            .map(|c| {
                json!([
                    c.symbol(),
                    format!("{:?}", c.fg),
                    format!("{:?}", c.bg),
                    c.modifier.bits(),
                    c.skip
                ])
            })
            .collect::<Vec<_>>();
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&cells).unwrap()));
        assert_eq!(
            digest, noir["frames"][index]["cell_sha256"],
            "original native {} at {width}×{height}",
            reference["name"]
        );
        let position = terminal.get_cursor_position().unwrap();
        let cursor = terminal
            .backend()
            .cursor_visible()
            .then_some([position.x, position.y]);
        assert_eq!(
            json!(cursor),
            reference["cursor"],
            "original native cursor {} at {width}×{height}",
            reference["name"]
        );
        assert_eq!(app.scroll, reference["scroll"].as_u64().unwrap() as u16);
    }
    assert_eq!(golden["frames"].as_array().unwrap().len(), 93);
}
