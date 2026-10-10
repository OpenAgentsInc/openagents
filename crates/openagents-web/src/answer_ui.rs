//! An answer's components ([`openui_lang::Node`]) as `openagents-ui`
//! markup, server-side (#11187).
//!
//! Every part is a plain element: links are anchors, a button is a link
//! that opens the page where its flow starts, a command is a code block
//! with a Copy button, and an install command's systems are radio tabs
//! that switch without a script. Nothing here writes a script or a style
//! attribute, so the site's policy holds.
//!
//! A button loads its page in full (`hx-boost="false"`) because the page
//! it opens may send the reader on to another site, such as GitHub's
//! sign-in; a text link to our own site stays in the tab and is boosted.

use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{ButtonLink, ButtonVariant, Color};
use openagents_ui::content::{AnswerCard, AnswerColumns, AnswerSteps, CodeBlock, LinkCard, Tabs};
use openui_lang::embed::{UNIX_LABEL, WINDOWS_LABEL};
use openui_lang::{ButtonStyle, Node};

use crate::markdown::{autolinked, open_link};

/// Draws `root` for a reader who is (or is not, or may be) signed in.
/// `id` is unique on the page, for the reply's tab groups.
pub(crate) fn render(root: &Node, signed_in: Option<bool>, id: &str) -> Markup {
    let mut draw = Draw {
        signed_in,
        id,
        tabs: 0,
    };
    html! { div.oa-answer-ui { (draw.node(root)) } }
}

struct Draw<'a> {
    signed_in: Option<bool>,
    id: &'a str,
    tabs: usize,
}

/// A link target as the page writes it: our own site as a site path.
fn href(target: &str) -> String {
    markdown_stream::autolink::same_site(target).unwrap_or_else(|| target.to_owned())
}

impl Draw<'_> {
    fn name(&mut self) -> String {
        self.tabs += 1;
        format!("{}-tabs{}", self.id, self.tabs)
    }

    fn nodes(&mut self, nodes: &[Node]) -> Vec<Markup> {
        nodes.iter().map(|node| self.node(node)).collect()
    }

    fn node(&mut self, node: &Node) -> Markup {
        match node {
            Node::Stack { children } => {
                let children = self.nodes(children);
                html! { div.oa-answer-stack { @for child in &children { (child) } } }
            }
            Node::Columns { children } => {
                let mut columns = AnswerColumns::new();
                for child in self.nodes(children) {
                    columns = columns.child(child);
                }
                columns.render()
            }
            Node::Card { title, children } => {
                let mut card = AnswerCard::new(title);
                for child in self.nodes(children) {
                    card = card.child(child);
                }
                card.render()
            }
            Node::Text { text } => html! { p { (PreEscaped(autolinked(text, false))) } },
            Node::Link { label, href } => {
                html! { p { (PreEscaped(open_link(href))) (label) (PreEscaped("</a>")) } }
            }
            Node::Button {
                label,
                href: target,
                style,
                show,
            } => {
                if !show.shows(self.signed_in) {
                    return html! {};
                }
                let button = ButtonLink::new(label, href(target)).attr("hx-boost", "false");
                match style {
                    ButtonStyle::Primary => button.render(),
                    ButtonStyle::Secondary => button
                        .variant(ButtonVariant::Outline)
                        .color(Color::Secondary)
                        .render(),
                }
            }
            Node::CodeBlock { code, language } => {
                let mut block = CodeBlock::new(code);
                if let Some(language) = language {
                    block = block.language(language);
                }
                block.render()
            }
            Node::Command { unix, windows } => match windows {
                Some(windows) => Tabs::new(&self.name(), "Install command")
                    .tab_for(
                        "unix",
                        UNIX_LABEL,
                        CodeBlock::new(unix).language("bash").wrap(true),
                    )
                    .tab_for(
                        "windows",
                        WINDOWS_LABEL,
                        CodeBlock::new(windows).language("powershell").wrap(true),
                    )
                    .render(),
                None => CodeBlock::new(unix).language("bash").wrap(true).render(),
            },
            Node::Steps { steps } => {
                let mut list = AnswerSteps::new();
                for step in steps {
                    let body = self.nodes(&step.children);
                    list = list.step(&step.title, body);
                }
                list.render()
            }
            Node::Tabs { tabs } => {
                let mut set = Tabs::new(&self.name(), "Choices");
                for tab in tabs {
                    let body = self.nodes(&tab.children);
                    set = set.tab(&tab.label, html! { @for block in &body { (block) } });
                }
                set.render()
            }
            Node::LinkCard {
                title,
                description,
                href: target,
            } => LinkCard::new(title, description, href(target)).render(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONNECT: &str = r#"root = Columns([web, computer])
web = Card("On the web", [Text("Connect GitHub at https://openagents.com/projects."), Button("Connect GitHub", href="/auth/github/repos?access=private", show="signed_in"), Button("Log in to connect", href="/login?return_to=/projects", show="signed_out")])
computer = Card("On your computer", [Steps([install, login])])
install = Step("Install Coder", [Command("curl -fsSL https://openagents.com/cli/install.sh | bash", windows="irm https://openagents.com/cli/install.ps1 | iex")])
login = Step("Sign in", [CodeBlock("coder login", "bash"), Button("Approve sign-in", href="https://openagents.com/device", style="secondary"), Link("GitHub", "https://github.com/OpenAgentsInc/openagents")])
"#;

    fn draw(signed_in: Option<bool>) -> String {
        render(&openui_lang::parse(CONNECT).root.unwrap(), signed_in, "m3").into_string()
    }

    #[test]
    fn the_connect_answer_draws_real_controls() {
        let html = draw(Some(true));
        assert!(html.contains("class=\"oa-answer-columns\""), "{html}");
        assert_eq!(
            html.matches("class=\"oa-answer-card\"").count(),
            2,
            "{html}"
        );
        // A real button to the flow, in the tab, loaded in full.
        assert!(
            html.contains("href=\"/auth/github/repos?access=private\""),
            "{html}"
        );
        assert!(html.contains("hx-boost=\"false\""), "{html}");
        assert!(!html.contains("Log in to connect"), "{html}");
        // Our own absolute URL becomes a site path; text URLs are links.
        assert!(html.contains("href=\"/device\""), "{html}");
        assert!(
            html.contains("<a href=\"/projects\">https://openagents.com/projects</a>."),
            "{html}"
        );
        // External links open in a new tab.
        assert!(
            html.contains("href=\"https://github.com/OpenAgentsInc/openagents\" target=\"_blank\" rel=\"noopener noreferrer\""),
            "{html}"
        );
        // OS tabs, each a copyable block.
        assert!(html.contains("name=\"m3-tabs1\""), "{html}");
        assert!(
            html.contains(
                "data-oa-copy=\"curl -fsSL https://openagents.com/cli/install.sh | bash\""
            ),
            "{html}"
        );
        assert!(
            html.contains("data-oa-copy=\"irm https://openagents.com/cli/install.ps1 | iex\""),
            "{html}"
        );
        assert!(html.contains("data-oa-copy=\"coder login\""), "{html}");
        assert!(
            html.contains("<span class=\"oa-answer-step__marker\" aria-hidden=\"true\">2</span>"),
            "{html}"
        );
        // No script, handler, or inline style.
        assert!(
            !html.contains("<script") && !html.contains("onclick") && !html.contains("style="),
            "{html}"
        );
        for hit in oa_copy::violations(&oa_copy::visible_text(&html), &[]) {
            panic!("machine talk: {hit:?}");
        }
    }

    #[test]
    fn a_signed_out_reader_gets_the_sign_in_button() {
        let html = draw(Some(false));
        assert!(html.contains("Log in to connect"), "{html}");
        assert!(
            html.contains("href=\"/login?return_to=/projects\""),
            "{html}"
        );
        assert!(!html.contains(">Connect GitHub<"), "{html}");
        // Unknown: the signed-in button.
        assert!(draw(None).contains("Connect GitHub"));
    }
}
