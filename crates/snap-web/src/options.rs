//! What the page's address asks for after `#`: `job=N` opens job N
//! straight away, `kit` shows the kit sheet, `contrast=high` turns on high
//! contrast, and `quiet` starts with the sound off. Separate them with `&`.

/// The page's options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub job: Option<usize>,
    pub kit: bool,
    pub contrast: bool,
    pub quiet: bool,
}

impl Options {
    /// Reads `#job=2&contrast=high`.
    #[must_use]
    pub fn parse(hash: &str) -> Self {
        let mut options = Self::default();
        for part in hash.trim_start_matches('#').split('&') {
            match part.split_once('=') {
                Some(("job", n)) => {
                    options.job = n
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=snap_rules::level::COUNT).contains(n));
                }
                Some(("contrast", "high")) => options.contrast = true,
                _ if part == "kit" => options.kit = true,
                _ if part == "quiet" => options.quiet = true,
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
    fn options_read_from_the_hash() {
        assert_eq!(Options::parse(""), Options::default());
        let all = Options::parse("#job=3&contrast=high&quiet");
        assert_eq!(all.job, Some(3));
        assert!(all.contrast && all.quiet && !all.kit);
        assert!(Options::parse("#kit").kit);
        assert_eq!(Options::parse("#job=99").job, None);
        assert_eq!(Options::parse("#job=x").job, None);
    }
}
