//! What a request is reduced to before anything is counted ([`Client`]):
//! a traffic class and a device class from the `User-Agent`, the
//! referring site's name from `Referer`, and whether the browser asked not
//! to be tracked. Nothing else of the request is read: not its address
//! (`X-Forwarded-For` is never looked at), cookies, query, or body.

use axum::http::{HeaderMap, header};

/// Who sent the request, as far as its `User-Agent` and `Accept` say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Traffic {
    /// A browser.
    Human,
    /// An AI agent or a program fetching on someone's behalf (an assistant's
    /// browsing tool, a coding agent, `curl`, an HTTP library), or any
    /// request that asks for Markdown.
    Agent,
    /// A crawler, monitor, link previewer, or a request with no agent name.
    Bot,
}

impl Traffic {
    pub fn name(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Bot => "bot",
        }
    }
}

/// The kind of device a browser runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Mobile,
    Tablet,
    Desktop,
}

impl Device {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mobile => "mobile",
            Self::Tablet => "tablet",
            Self::Desktop => "desktop",
        }
    }
}

/// Assistants and agents fetching a page for a person, named in their
/// `User-Agent` (lowercase).
const AGENTS: [&str; 22] = [
    "chatgpt-user",
    "claude-user",
    "perplexity-user",
    "mistralai-user",
    "duckassistbot",
    "openagents",
    "coder/",
    "codex",
    "claude-code",
    "cursor",
    "windsurf",
    "copilot",
    "devin",
    "manus",
    "operator",
    "browser-use",
    "mcp",
    "goose",
    "aider",
    "opencode",
    "gemini-cli",
    "agent",
];

/// Crawlers, monitors and previewers (lowercase).
const BOTS: [&str; 16] = [
    "bot",
    "crawl",
    "spider",
    "slurp",
    "facebookexternalhit",
    "embedly",
    "preview",
    "monitor",
    "uptime",
    "pingdom",
    "lighthouse",
    "headless",
    "scan",
    "fetcher",
    "archiver",
    "google-extended",
];

/// HTTP tools and libraries (lowercase): a person or an agent's script.
const TOOLS: [&str; 16] = [
    "curl/",
    "wget/",
    "python",
    "httpx",
    "aiohttp",
    "node-fetch",
    "undici",
    "axios",
    "go-http-client",
    "okhttp",
    "powershell",
    "libwww",
    "java/",
    "ruby",
    "reqwest",
    "deno",
];

/// The traffic class of a `User-Agent` and `Accept` pair.
pub fn traffic(agent: &str, accept: &str) -> Traffic {
    let agent = agent.to_ascii_lowercase();
    if AGENTS.iter().any(|name| agent.contains(name)) {
        return Traffic::Agent;
    }
    if agent.trim().is_empty() || BOTS.iter().any(|name| agent.contains(name)) {
        return Traffic::Bot;
    }
    if TOOLS.iter().any(|name| agent.contains(name))
        || accept.to_ascii_lowercase().contains("text/markdown")
    {
        return Traffic::Agent;
    }
    if agent.starts_with("mozilla/") {
        Traffic::Human
    } else {
        Traffic::Bot
    }
}

/// The device class of a browser's `User-Agent`.
pub fn device(agent: &str) -> Device {
    if agent.contains("iPad") || (agent.contains("Android") && !agent.contains("Mobile")) {
        Device::Tablet
    } else if agent.contains("Mobi") || agent.contains("iPhone") || agent.contains("Android") {
        Device::Mobile
    } else {
        Device::Desktop
    }
}

/// The referring site's name (`news.ycombinator.com`), lowercase, without
/// `www.`; `None` for no referrer, this site itself, or anything that is
/// not a plain host name. Never the path or query.
pub fn referrer(referer: &str, own_host: &str) -> Option<String> {
    let url = url::Url::parse(referer).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let own = own_host
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    if host == own || host == own.strip_prefix("www.").unwrap_or(&own) {
        return None;
    }
    let plain = !host.is_empty()
        && host.len() <= 80
        && host.contains('.')
        // A host name, never an address.
        && host.bytes().any(|b| b.is_ascii_alphabetic())
        && !host.contains(':')
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        && !host.rsplit('.').next().is_some_and(|tld| tld.bytes().all(|b| b.is_ascii_digit()));
    Some(if plain {
        site(&host)
    } else {
        "other".to_owned()
    })
}

/// The site a host name belongs to: its last two labels, or three under a
/// two-letter country domain with a short second level (`bbc.co.uk`), so
/// a subdomain that could name a person (`someone.github.io`) is dropped.
fn site(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    let keep = match labels.as_slice() {
        [.., second, top] if top.len() == 2 && second.len() <= 3 && labels.len() >= 3 => 3,
        _ => 2,
    };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

/// A request reduced to what is counted.
#[derive(Clone, Debug)]
pub struct Client {
    pub traffic: Traffic,
    pub device: Device,
    /// The referring site, if another site sent the visitor.
    pub referrer: Option<String>,
    /// `DNT: 1` or `Sec-GPC: 1`: only the anonymous page view is counted.
    pub quiet: bool,
    /// An HTMX request for part of a page (not a boosted navigation).
    pub fragment: bool,
}

impl Client {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let text = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
        };
        let agent = text(header::USER_AGENT.as_str());
        let accept = text(header::ACCEPT.as_str());
        let quiet = text("dnt").trim() == "1" || text("sec-gpc").trim() == "1";
        let boosted = headers.contains_key("hx-boosted");
        Self {
            traffic: traffic(agent, accept),
            device: device(agent),
            referrer: referrer(text(header::REFERER.as_str()), text(header::HOST.as_str())),
            quiet,
            fragment: headers.contains_key("hx-request") && !boosted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_agents_and_bots_are_told_apart() {
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36";
        assert_eq!(traffic(chrome, "text/html"), Traffic::Human);
        assert_eq!(traffic(chrome, "text/markdown"), Traffic::Agent);
        assert_eq!(
            traffic(
                "Mozilla/5.0 AppleWebKit/537.36; compatible; ChatGPT-User/1.0",
                ""
            ),
            Traffic::Agent
        );
        assert_eq!(traffic("Claude-User/1.0", ""), Traffic::Agent);
        assert_eq!(traffic("curl/8.7.1", "*/*"), Traffic::Agent);
        assert_eq!(
            traffic("Mozilla/5.0 (compatible; Googlebot/2.1)", "text/html"),
            Traffic::Bot
        );
        assert_eq!(
            traffic("Mozilla/5.0 (compatible; ClaudeBot/1.0)", ""),
            Traffic::Bot
        );
        assert_eq!(traffic("", ""), Traffic::Bot);
        assert_eq!(traffic("SomethingElse/1", ""), Traffic::Bot);
    }

    #[test]
    fn devices_come_from_the_agent_name() {
        assert_eq!(
            device("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Mobile/15E148"),
            Device::Mobile
        );
        assert_eq!(device("Mozilla/5.0 (iPad; CPU OS 18_0)"), Device::Tablet);
        assert_eq!(
            device("Mozilla/5.0 (Linux; Android 15; Pixel 9) Mobile"),
            Device::Mobile
        );
        assert_eq!(
            device("Mozilla/5.0 (Linux; Android 15; SM-X910)"),
            Device::Tablet
        );
        assert_eq!(device("Mozilla/5.0 (X11; Linux x86_64)"), Device::Desktop);
    }

    #[test]
    fn referrers_keep_only_the_site_name() {
        assert_eq!(
            referrer(
                "https://news.ycombinator.com/item?id=4242",
                "openagents.com"
            )
            .as_deref(),
            Some("ycombinator.com")
        );
        assert_eq!(
            referrer("https://someone.github.io/post", "openagents.com").as_deref(),
            Some("github.io")
        );
        assert_eq!(
            referrer("https://www.bbc.co.uk/news", "openagents.com").as_deref(),
            Some("bbc.co.uk")
        );
        assert_eq!(
            referrer("https://www.google.com/", "openagents.com").as_deref(),
            Some("google.com")
        );
        assert_eq!(
            referrer("https://openagents.com/docs", "openagents.com"),
            None
        );
        assert_eq!(
            referrer("https://www.openagents.com/", "openagents.com"),
            None
        );
        assert_eq!(referrer("", "openagents.com"), None);
        assert_eq!(referrer("android-app://com.slack", "openagents.com"), None);
        assert_eq!(
            referrer("http://203.0.113.9:8080/x", "openagents.com").as_deref(),
            Some("other")
        );
        assert_eq!(
            referrer("http://[2001:db8::1]/x", "openagents.com").as_deref(),
            Some("other")
        );
    }
}
