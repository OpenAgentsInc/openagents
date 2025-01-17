use askama::Result;
use pulldown_cmark::{html, Options, Parser};

pub fn markdown(s: &str) -> Result<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    
    let parser = Parser::new_ext(s, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    
    Ok(html_output)
}
