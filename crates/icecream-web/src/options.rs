//! What the page's address fragment asks for, `#seed=7&skip`: a fixed
//! house, and switches for tests and captures.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Play this house instead of a new one each run.
    pub seed: Option<u64>,
    /// Start playing straight away: no title card, no tutorial.
    pub skip: bool,
    /// The built-in bot plays (tests and captures).
    pub bot: bool,
    /// Steps per frame times this (tests): 1 to 8.
    pub speed: u32,
    /// The Ice Cream Man stays out of the house (captures).
    pub calm: bool,
    /// The run starts with no fart clouds (captures of the fudge).
    pub no_farts: bool,
    /// The camera keeps the Ice Cream Man in view over your shoulder
    /// (captures).
    pub watch: bool,
    /// A still picture: the Ice Cream Man in the bedroom, facing you
    /// (captures).
    pub portrait: bool,
}

impl Options {
    #[must_use]
    pub fn parse(hash: &str) -> Self {
        let mut options = Self {
            speed: 1,
            ..Self::default()
        };
        for pair in hash.trim_start_matches('#').split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            match key {
                "seed" => options.seed = value.parse().ok(),
                "skip" => options.skip = true,
                "bot" => {
                    options.bot = true;
                    options.skip = true;
                }
                "speed" => options.speed = value.parse().unwrap_or(1).clamp(1, 8),
                "calm" => options.calm = true,
                "nofarts" => options.no_farts = true,
                "watch" => options.watch = true,
                "portrait" => {
                    options.portrait = true;
                    options.skip = true;
                }
                _ => {}
            }
        }
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_read_from_the_fragment() {
        let o = Options::parse("#seed=7&bot&speed=4&calm&nofarts&watch&portrait");
        assert_eq!(o.seed, Some(7));
        assert!(o.bot && o.skip && o.calm && o.no_farts && o.watch && o.portrait);
        assert_eq!(o.speed, 4);
        let none = Options::parse("");
        assert_eq!(none.seed, None);
        assert_eq!(none.speed, 1);
        assert!(!none.skip);
        assert_eq!(Options::parse("#speed=99").speed, 8);
        assert_eq!(Options::parse("#seed=x").seed, None);
    }
}
