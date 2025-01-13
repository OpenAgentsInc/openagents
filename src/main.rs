use openagents::run;
use openagents::templates; // P06e4

fn main() -> std::io::Result<()> {
    templates::render_header_template(vec![]); // P54a1
    run()
}
