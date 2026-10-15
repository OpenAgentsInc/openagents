//! Our own campaign links, `/go/{code}`: the checked-in list.
//!
//! A code is `{channel}-{series}`. Channels: `x` (X), `yt` (YouTube),
//! `tt` (TikTok), `ig` (Instagram), `li` (LinkedIn). Each code has one
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

    /// The series: what comes after the first `-`.
    pub fn series(&self) -> &'static str {
        self.code.split_once('-').map_or("", |(_, s)| s)
    }
}

/// The channels a code may start with.
pub const CHANNELS: [&str; 5] = ["x", "yt", "tt", "ig", "li"];

const fn code(code: &'static str, destination: &'static str) -> Code {
    Code { code, destination }
}

/// Every campaign code.
pub const CODES: [Code; 20] = [
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
];

/// The code named `code`, if it is one of ours.
pub fn find(code: &str) -> Option<&'static Code> {
    CODES.iter().find(|c| c.code == code)
}
