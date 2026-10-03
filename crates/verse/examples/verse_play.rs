// Interactive original procedural chamber entry point.
mod chamber_app;
fn main() -> Result<(), String> {
    chamber_app::run(true)
}
