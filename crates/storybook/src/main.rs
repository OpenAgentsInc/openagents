//! Storybook: Visual component explorer for unit and hud crates
//!
//! Run with: cargo run -p storybook [story_name]
//! Example: cargo run -p storybook pin_states

mod stories;
mod story;

use clap::Parser;
use gpui::{
    App, AnyView, Bounds, Context, Render, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, size,
};
use std::borrow::Cow;
use strum::IntoEnumIterator;

use crate::stories::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString, strum::EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum ComponentStory {
    /// Pin states: Empty, Valid, Invalid, Constant
    PinStates,
    /// Unit view with different lifecycle states
    UnitView,
    /// Connection bezier curves with different states
    Connections,
    /// Graph view with physics simulation
    GraphView,
    /// All components in one view
    KitchenSink,
    /// Unit runtime: system units execution
    UnitRuntime,
    /// Value types: dynamic Value with JS-like coercion
    ValueTypes,
    /// Unit chains: connecting units for complex computations
    UnitChains,
}

impl ComponentStory {
    pub fn story(&self, _window: &mut Window, cx: &mut App) -> AnyView {
        match self {
            Self::PinStates => cx.new(|_| PinStatesStory).into(),
            Self::UnitView => cx.new(|_| UnitViewStory).into(),
            Self::Connections => cx.new(|_| ConnectionsStory).into(),
            Self::GraphView => cx.new(|cx| GraphViewStory::new(cx)).into(),
            Self::KitchenSink => cx.new(|cx| KitchenSinkStory::new(cx)).into(),
            Self::UnitRuntime => cx.new(|_| UnitRuntimeStory).into(),
            Self::ValueTypes => cx.new(|_| ValueTypesStory).into(),
            Self::UnitChains => cx.new(|_| UnitChainsStory).into(),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::PinStates => "Pin states: Empty, Valid, Invalid, Constant",
            Self::UnitView => "Unit view with different lifecycle states",
            Self::Connections => "Connection bezier curves with different states",
            Self::GraphView => "Graph view with physics simulation",
            Self::KitchenSink => "All components in one view",
            Self::UnitRuntime => "System units: arithmetic, logic, comparison, control",
            Self::ValueTypes => "Dynamic Value type with JS-like coercion",
            Self::UnitChains => "Connecting units for complex computations",
        }
    }
}

#[derive(Parser)]
#[command(name = "storybook", about = "Visual component explorer for unit/hud")]
struct Args {
    /// The story to display
    story: Option<String>,

    /// List all available stories
    #[arg(short, long)]
    list: bool,
}

fn main() {
    let args = Args::parse();

    if args.list {
        println!("Available stories:\n");
        for story in ComponentStory::iter() {
            println!("  {:15} - {}", story.to_string(), story.description());
        }
        return;
    }

    let story = match args.story {
        Some(name) => {
            use std::str::FromStr;
            match ComponentStory::from_str(&name) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Unknown story: {}", name);
                    eprintln!("Run with --list to see available stories");
                    std::process::exit(1);
                }
            }
        }
        None => ComponentStory::KitchenSink,
    };

    gpui::Application::new().run(move |cx: &mut App| {
        // Load Berkeley Mono fonts if available
        let _ = cx.text_system().add_fonts(vec![
            Cow::Borrowed(include_bytes!("../../commander/assets/fonts/BerkeleyMono-Regular.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("../../commander/assets/fonts/BerkeleyMono-Bold.ttf").as_slice()),
        ]);

        let size = size(px(1200.0), px(800.0));
        let bounds = Bounds::centered(None, size, cx);

        let _window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(format!("Storybook - {}", story).into()),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            move |window, cx| {
                let story_view = story.story(window, cx);
                cx.new(|_| StoryWrapper { story: story_view })
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

/// Wrapper that provides consistent styling around stories
struct StoryWrapper {
    story: AnyView,
}

impl Render for StoryWrapper {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x1e1e1e))
            .text_color(gpui::rgb(0xe0e0e0))
            .font_family("Berkeley Mono")
            .child(self.story.clone())
    }
}
