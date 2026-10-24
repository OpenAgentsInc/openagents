//! Our own campaign links, `/go/{code}`: the checked-in list.
//!
//! A code is `{channel}-{series}`. Channels: `x` (X), `yt` (YouTube),
//! `tt` (TikTok), `ig` (Instagram), `li` (LinkedIn), and the Kiki beta
//! launch's channels (`youtube`, `reddit`, `itch`, `discord`, `hn`,
//! `email`, `friend`). Each code has one
//! fixed destination on this site. To add a series, add its codes here
//! (one per channel it runs on) and deploy; a code not in this list goes to
//! `/` and is counted as unknown.

/// One campaign code and where it lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    pub code: &'static str,
    pub destination: &'static str,
}

impl Code {
    /// The channel: what comes before the first `-`.
    pub fn channel(&self) -> &'static str {
        self.code.split_once('-').map_or(self.code, |(c, _)| c)
    }

    /// The series: what comes after the first `-`, or the whole code when
    /// it is one word (`hn`, `discord`, `email`).
    pub fn series(&self) -> &'static str {
        self.code.split_once('-').map_or(self.code, |(_, s)| s)
    }

    /// The destination's path, without its campaign tags.
    pub fn path(&self) -> &'static str {
        self.destination
            .split_once('?')
            .map_or(self.destination, |(path, _)| path)
    }
}

/// The channels a code may start with.
pub const CHANNELS: [&str; 12] = [
    "x", "yt", "tt", "ig", "li", "youtube", "reddit", "itch", "discord", "hn", "email", "friend",
];

const fn code(code: &'static str, destination: &'static str) -> Code {
    Code { code, destination }
}

/// Every campaign code.
pub const CODES: [Code; 31] = [
    // The cost hook: what an accepted fix costs. Lands on pricing.
    code("x-cost", "/pricing"),
    code("yt-cost", "/pricing"),
    code("tt-cost", "/pricing"),
    code("ig-cost", "/pricing"),
    code("li-cost", "/pricing"),
    // The capability hook: one Coder in front of every coding agent.
    code("x-capability", "/"),
    code("yt-capability", "/"),
    code("tt-capability", "/"),
    code("ig-capability", "/"),
    code("li-capability", "/"),
    // Straight to the download page.
    code("x-download", "/download"),
    code("yt-download", "/download"),
    code("tt-download", "/download"),
    code("ig-download", "/download"),
    code("li-download", "/download"),
    // The launch posts.
    code("x-launch", "/"),
    code("yt-launch", "/"),
    code("tt-launch", "/"),
    code("ig-launch", "/"),
    code("li-launch", "/"),
    // The Kiki beta launch (2026-10-24): each lands on the homepage with
    // UTM tags, so the playtest research funnel shows each channel.
    code(
        "x-chris",
        "/?utm_source=x&utm_medium=social&utm_campaign=x-chris",
    ),
    code(
        "x-openagents",
        "/?utm_source=x&utm_medium=social&utm_campaign=x-openagents",
    ),
    code(
        "youtube-kiki",
        "/?utm_source=youtube&utm_medium=video&utm_campaign=youtube-kiki",
    ),
    code(
        "reddit-gamedev",
        "/?utm_source=reddit&utm_medium=community&utm_campaign=reddit-gamedev",
    ),
    code(
        "reddit-indiedev",
        "/?utm_source=reddit&utm_medium=community&utm_campaign=reddit-indiedev",
    ),
    code(
        "reddit-playmygame",
        "/?utm_source=reddit&utm_medium=community&utm_campaign=reddit-playmygame",
    ),
    code(
        "itch-community",
        "/?utm_source=itch&utm_medium=community&utm_campaign=itch-community",
    ),
    code(
        "discord",
        "/?utm_source=discord&utm_medium=community&utm_campaign=discord",
    ),
    code("hn", "/?utm_source=hn&utm_medium=community&utm_campaign=hn"),
    code(
        "email",
        "/?utm_source=email&utm_medium=email&utm_campaign=email",
    ),
    code(
        "friend-share",
        "/?utm_source=friend&utm_medium=share&utm_campaign=friend-share",
    ),
];

/// The code named `code`, if it is one of ours.
pub fn find(code: &str) -> Option<&'static Code> {
    CODES.iter().find(|c| c.code == code)
}
