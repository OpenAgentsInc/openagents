#[path = "storybook/app.rs"]
mod app;
#[path = "storybook/constants.rs"]
mod constants;
#[path = "storybook/demos/mod.rs"]
mod demos;
#[path = "storybook/helpers.rs"]
mod helpers;
#[path = "storybook/sections/mod.rs"]
mod sections;
#[path = "storybook/state.rs"]
mod state;

fn main() {
    app::run();
}
