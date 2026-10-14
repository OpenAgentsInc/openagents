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
    /// The run starts with every earned weapon (captures).
    pub arm: bool,
    /// The run starts as if just won, in free roam with the sprinkle
    /// staff (captures; the version must allow free roam).
    pub free_roam: bool,
    /// Start in first person, whatever the version says (captures).
    pub first_person: bool,
    /// An edible cone on the floor two steps ahead at the start
    /// (captures).
    pub cone: bool,
    /// Pick this hot-bar tool at the start, by name (captures).
    pub tool: Option<String>,
    /// Game Lab's version of the game (`docs/game-lab/README.md`): its
    /// numbers, `t.NAME=VALUE` (`icecream_rules::tuning::FIELDS`), played
    /// as data without building the game again.
    pub tune: Vec<(String, String)>,
    /// Its words, `c.NAME=TEXT` (percent-encoded): title, tagline,
    /// escaped_line, scooped_line.
    pub words: Vec<(String, String)>,
}

/// `%XX` and `+` decoded; a bad escape is kept as it is.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = |b: u8| (b as char).to_digit(16);
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Options {
    /// The run's numbers: the defaults with the fragment's changes (a
    /// value out of bounds is left at its default).
    #[must_use]
    pub fn tuning(&self) -> icecream_rules::Tuning {
        let mut tuning = icecream_rules::Tuning::default();
        for (name, value) in &self.tune {
            if let Ok(value) = value.parse::<f64>() {
                let _ = tuning.set(name, value);
            }
        }
        tuning
    }

    /// A word the fragment changed, by name.
    #[must_use]
    pub fn word(&self, name: &str) -> Option<&str> {
        self.words
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, text)| text.as_str())
            .filter(|text| !text.trim().is_empty())
    }
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
                "arm" => options.arm = true,
                "freeroam" => {
                    options.free_roam = true;
                    options.skip = true;
                }
                "fp" => options.first_person = true,
                "cone" => options.cone = true,
                "tool" => options.tool = Some(value.to_owned()),
                _ => {
                    if let Some(name) = key.strip_prefix("t.") {
                        options.tune.push((name.to_owned(), value.to_owned()));
                    } else if let Some(name) = key.strip_prefix("c.") {
                        options.words.push((name.to_owned(), decode(value)));
                    }
                }
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
        let o = Options::parse(
            "#seed=7&bot&speed=4&calm&nofarts&watch&portrait&arm&freeroam&fp&tool=spear&cone",
        );
        assert_eq!(o.seed, Some(7));
        assert!(o.bot && o.skip && o.calm && o.no_farts && o.watch && o.portrait);
        assert!(o.arm && o.free_roam && o.first_person && o.cone);
        assert_eq!(o.tool.as_deref(), Some("spear"));
        assert_eq!(o.speed, 4);
        let none = Options::parse("");
        assert_eq!(none.seed, None);
        assert_eq!(none.speed, 1);
        assert!(!none.skip);
        assert_eq!(Options::parse("#speed=99").speed, 8);
        assert_eq!(Options::parse("#seed=x").seed, None);
    }

    #[test]
    fn a_game_lab_version_comes_as_data() {
        let o = Options::parse(
            "#seed=7&t.man_chase=4.48&t.extra_closets=2&t.unlock=1e9&c.title=The%20Slow%20Man&c.tagline=&t.minion_count=3&t.earned_weapons=1&t.yellow=1&t.first_person=1",
        );
        let t = o.tuning();
        assert!((t.man_chase - 4.48).abs() < 1e-6);
        assert_eq!(t.extra_closets, 2);
        assert_eq!(
            (t.minion_count, t.earned_weapons, t.first_person),
            (3, 1, 1)
        );
        assert!((t.yellow - 1.0).abs() < 1e-6);
        assert_eq!(
            t.unlock,
            icecream_rules::tuning::UNLOCK,
            "out of bounds stays put"
        );
        assert_eq!(o.word("title"), Some("The Slow Man"));
        assert_eq!(o.word("tagline"), None);
        assert_eq!(decode("a%2Cb+c%zz"), "a,b c%zz");
    }
}
