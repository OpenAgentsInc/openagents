//! The blog: `/blog`, every post newest first with its date and summary,
//! and `/blog/{slug}`, one post. It works like the docs: each post is
//! Markdown in `content/blog/`, compiled into the binary, rendered by the
//! docs' renderer, and also served as Markdown at `/blog/{slug}.md` (and
//! `/blog.md` for the index). The site's `llms.txt`, `llms-full.txt`, and
//! sitemap list the posts (`crate::agent_ready`), and `/blog/feed.xml` is
//! an Atom feed of them.
//!
//! A post opens with front matter between `---` lines: `title`, `date`
//! (`YYYY-MM-DD`), `author`, and `summary`, one `key: value` each. The
//! body that follows has no title of its own; the page draws it.
//!
//! The site had a blog once before (`89436ebaba`, one post about the old
//! Coder Terminal), removed with that product in `0b63a7d27a`; this one
//! follows the docs' current look and agent plumbing instead.

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::shell::Breadcrumb;

use crate::App;
use crate::agent_ready::SITE;
use crate::layout::escape;
use crate::markdown;
use crate::ui_page::{UiPage, action_link, problem, prose};

/// The posts, by slug. [`posts`] sorts them newest first.
pub(crate) const POSTS: [(&str, &str); 1] =
    [("coder-1-0", include_str!("../../content/blog/coder-1-0.md"))];

/// What the blog is, in one line.
pub(crate) const LEAD: &str = "News from OpenAgents: releases, and what we're building.";

/// The Atom feed's address.
pub(crate) const FEED: &str = "/blog/feed.xml";

/// One post: its front matter and its Markdown body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Post {
    pub(crate) slug: &'static str,
    pub(crate) title: String,
    /// `YYYY-MM-DD`.
    pub(crate) date: String,
    pub(crate) author: String,
    pub(crate) summary: String,
    pub(crate) body: &'static str,
}

impl Post {
    /// The post's page.
    pub(crate) fn path(&self) -> String {
        format!("/blog/{}", self.slug)
    }

    /// The post as one Markdown document: its title, its byline, its body.
    pub(crate) fn markdown(&self) -> String {
        format!(
            "# {}\n\n*{} \u{b7} {}*\n\n{}",
            self.title,
            long_date(&self.date),
            self.author,
            self.body.trim_start()
        )
    }
}

/// Reads `source`'s front matter. `None` when it has none, or misses a
/// field.
pub(crate) fn parse(slug: &'static str, source: &'static str) -> Option<Post> {
    let rest = source.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let (head, body) = (&rest[..end], &rest[end + "\n---\n".len()..]);
    let field = |key: &str| {
        head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| {
                let value = value.trim();
                value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(value)
                    .to_owned()
            })
        })
    };
    let post = Post {
        slug,
        title: field("title")?,
        date: field("date")?,
        author: field("author")?,
        summary: field("summary")?,
        body,
    };
    (!post.title.is_empty() && valid_date(&post.date)).then_some(post)
}

/// Whether `date` is a real `YYYY-MM-DD` day.
fn valid_date(date: &str) -> bool {
    let parts: Vec<&str> = date.split('-').collect();
    let [year, month, day] = parts[..] else {
        return false;
    };
    let number = |text: &str, width: usize| {
        (text.len() == width && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse::<u32>().ok())
            .flatten()
    };
    matches!(
        (number(year, 4), number(month, 2), number(day, 2)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

/// `2026-10-15` as `October 15, 2026`.
pub(crate) fn long_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return date.to_owned();
    };
    match (month.parse::<usize>(), day.parse::<u32>()) {
        (Ok(month @ 1..=12), Ok(day)) => format!("{} {day}, {year}", MONTHS[month - 1]),
        _ => date.to_owned(),
    }
}

/// Every post, newest first (by date, then slug).
pub(crate) fn posts() -> Vec<Post> {
    let mut posts: Vec<Post> = POSTS
        .iter()
        .filter_map(|(slug, source)| parse(slug, source))
        .collect();
    posts.sort_by(|a, b| b.date.cmp(&a.date).then(a.slug.cmp(b.slug)));
    posts
}

/// The post `slug`, if there is one.
pub(crate) fn post(slug: &str) -> Option<Post> {
    posts().into_iter().find(|post| post.slug == slug)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/blog", get(index))
        .route("/blog.md", get(index_md))
        .route(FEED, get(feed))
        .route("/blog/{slug}", get(page))
}

/// The feed's link, for a blog page's `<head>`.
fn feed_link() -> PreEscaped<String> {
    PreEscaped(format!(
        "<link rel=\"alternate\" type=\"application/atom+xml\" title=\"OpenAgents blog\" href=\"{FEED}\">"
    ))
}

async fn index(headers: HeaderMap) -> Response {
    index_page(&headers)
}

/// `/blog` for `headers`: every post, newest first, with its date and
/// summary.
pub(crate) fn index_page(headers: &HeaderMap) -> Response {
    let mut body = format!(
        "<h1>Blog</h1><p class=\"oa-page-lead\">{}</p>",
        escape(LEAD)
    );
    for post in posts() {
        body.push_str(&format!(
            "<h2><a href=\"{path}\">{title}</a></h2><p class=\"oa-page-meta\">{date} \u{b7} {author}</p><p>{summary}</p>",
            path = post.path(),
            title = escape(&post.title),
            date = escape(&long_date(&post.date)),
            author = escape(&post.author),
            summary = escape(&post.summary),
        ));
    }
    body.push_str(&format!(
        "<p class=\"oa-page-meta\">Follow along with the <a href=\"{FEED}\">feed</a>, or read \
any post as Markdown: add <code>.md</code> to its address.</p>"
    ));
    UiPage::new("Blog")
        .section("/blog")
        .path("/blog")
        .description(LEAD)
        .head(feed_link())
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(headers)
}

/// `/blog.md`: the index as Markdown.
pub(crate) fn index_markdown() -> String {
    format!("# Blog\n\n{LEAD}\n\n{}", list(false))
}

/// The posts as Markdown list items, newest first, each with its date and
/// summary. `absolute` writes full addresses.
pub(crate) fn list(absolute: bool) -> String {
    let base = if absolute { SITE } else { "" };
    posts()
        .iter()
        .map(|post| {
            format!(
                "- [{}]({base}/blog/{}.md) ({}): {}\n",
                post.title, post.slug, post.date, post.summary
            )
        })
        .collect()
}

async fn index_md() -> Response {
    crate::agent_ready::markdown_doc("/blog", "Blog", LEAD, &index_markdown())
}

/// `/blog/{slug}` and `/blog/{slug}.md`.
async fn page(Path(slug): Path<String>, headers: HeaderMap) -> Response {
    if let Some(name) = slug.strip_suffix(".md")
        && let Some(post) = post(name)
    {
        return crate::agent_ready::markdown_doc(
            &post.path(),
            &post.title,
            &post.summary,
            &post.markdown(),
        );
    }
    post_page(&headers, &slug)
}

/// The post `slug` for `headers`, or the not-found page.
pub(crate) fn post_page(headers: &HeaderMap, slug: &str) -> Response {
    let Some(post) = post(slug) else {
        return problem(
            headers,
            StatusCode::NOT_FOUND,
            "Post not found",
            "No blog post has that name.",
            ("/blog", "All posts"),
        );
    };
    let header = format!(
        "<h1>{}</h1><p class=\"oa-page-meta\">{} \u{b7} {}</p>",
        escape(&post.title),
        escape(&long_date(&post.date)),
        escape(&post.author)
    );
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(PreEscaped(format!(
            "{header}{}",
            markdown::render_document(post.body)
        ))))
        nav.oa-page-actions aria-label="More from the blog" {
            (action_link("All posts", "/blog"))
        }
    });
    UiPage::new(post.title.clone())
        .breadcrumb(Breadcrumb::new(post.title.clone()).crumb("Blog", "/blog"))
        .section("/blog")
        .path(post.path())
        .description(post.summary.clone())
        .head(feed_link())
        .scriptless()
        .content(content)
        .respond(headers)
}

/// `/blog/feed.xml`: an Atom feed (RFC 4287) of every post, newest first.
pub(crate) fn atom() -> String {
    let posts = posts();
    let updated = posts
        .first()
        .map_or("1970-01-01", |post| post.date.as_str());
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<feed xmlns=\"http://www.w3.org/2005/Atom\">\n\
  <title>OpenAgents blog</title>\n\
  <subtitle>{lead}</subtitle>\n\
  <id>{SITE}/blog</id>\n\
  <link href=\"{SITE}/blog\"/>\n\
  <link rel=\"self\" href=\"{SITE}{FEED}\"/>\n\
  <updated>{updated}T00:00:00Z</updated>\n",
        lead = escape(LEAD)
    );
    for post in posts {
        out.push_str(&format!(
            "  <entry>\n\
    <title>{title}</title>\n\
    <id>{SITE}{path}</id>\n\
    <link href=\"{SITE}{path}\"/>\n\
    <link rel=\"alternate\" type=\"text/markdown\" href=\"{SITE}{path}.md\"/>\n\
    <published>{date}T00:00:00Z</published>\n\
    <updated>{date}T00:00:00Z</updated>\n\
    <author><name>{author}</name></author>\n\
    <summary>{summary}</summary>\n\
    <content type=\"html\">{content}</content>\n\
  </entry>\n",
            title = escape(&post.title),
            path = post.path(),
            date = post.date,
            author = escape(&post.author),
            summary = escape(&post.summary),
            content = escape(&markdown::render_document(post.body)),
        ));
    }
    out.push_str("</feed>\n");
    out
}

async fn feed() -> Response {
    let mut response = (StatusCode::OK, atom()).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/atom+xml; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every post has its front matter: a title, a real date, an author,
    /// and a summary; its body has no title of its own; slugs are unique.
    #[test]
    fn every_post_has_its_front_matter() {
        assert_eq!(posts().len(), POSTS.len());
        for (slug, source) in POSTS {
            let post = parse(slug, source).unwrap_or_else(|| panic!("{slug}: front matter"));
            assert!(
                !post.author.is_empty() && !post.summary.is_empty(),
                "{slug}"
            );
            assert!(
                !post.body.trim_start().starts_with("# "),
                "{slug}: the page draws the title"
            );
            assert!(
                slug.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{slug}"
            );
            assert_eq!(POSTS.iter().filter(|(s, _)| *s == slug).count(), 1);
        }
    }

    #[test]
    fn front_matter_and_dates_parse() {
        let post = parse(
            "x",
            "---\ntitle: \"A: B\"\ndate: 2026-02-03\nauthor: Us\nsummary: S.\n---\n\nBody.\n",
        )
        .unwrap();
        assert_eq!(post.title, "A: B");
        assert_eq!(long_date(&post.date), "February 3, 2026");
        assert_eq!(post.body.trim(), "Body.");
        assert!(parse("x", "# No front matter\n").is_none());
        assert!(
            parse(
                "x",
                "---\ntitle: T\ndate: 2026-13-01\nauthor: A\nsummary: S\n---\n"
            )
            .is_none()
        );
        assert!(parse("x", "---\ntitle: T\ndate: 2026-01-01\nauthor: A\n---\n").is_none());
    }

    /// The feed is well formed enough to read: one entry a post, its
    /// address absolute, and nothing unescaped from a post's text.
    #[test]
    fn the_feed_lists_every_post() {
        let feed = atom();
        assert!(feed.starts_with("<?xml"));
        assert_eq!(feed.matches("<entry>").count(), POSTS.len());
        for post in posts() {
            assert!(feed.contains(&format!("<id>{SITE}/blog/{}</id>", post.slug)));
        }
        assert!(!feed.contains("<p>"), "the content is escaped");
    }
}
