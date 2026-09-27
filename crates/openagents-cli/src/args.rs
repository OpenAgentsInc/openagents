//! A small argument reader: `--flag VALUE` options, `--switch` toggles, and
//! the positional words that remain, in order.

use std::collections::BTreeMap;

/// Parsed command-line words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    positional: Vec<String>,
    options: BTreeMap<String, Vec<String>>,
    switches: Vec<String>,
}

impl Args {
    /// Read `words`. Names in `switches` take no value; every other `--name`
    /// takes the next word. `--` ends option parsing.
    ///
    /// # Errors
    /// Reports an option with no value.
    pub fn parse(words: &[String], switches: &[&str]) -> Result<Self, String> {
        let mut args = Self::default();
        let mut index = 0;
        let mut only_positional = false;
        while index < words.len() {
            let word = &words[index];
            if !only_positional && word == "--" {
                only_positional = true;
            } else if !only_positional && let Some(name) = word.strip_prefix("--") {
                if switches.contains(&name) {
                    args.switches.push(name.to_owned());
                } else if let Some((name, value)) = name.split_once('=') {
                    args.options
                        .entry(name.to_owned())
                        .or_default()
                        .push(value.to_owned());
                } else {
                    index += 1;
                    let value = words
                        .get(index)
                        .ok_or_else(|| format!("--{name} needs a value"))?;
                    args.options
                        .entry(name.to_owned())
                        .or_default()
                        .push(value.clone());
                }
            } else {
                args.positional.push(word.clone());
            }
            index += 1;
        }
        Ok(args)
    }

    pub fn positional(&self) -> &[String] {
        &self.positional
    }

    /// The same options and switches as `from`, with `positional` as the
    /// remaining words, for a command that nests another.
    #[must_use]
    pub fn from_positional(positional: &[String], from: &Self) -> Self {
        Self {
            positional: positional.to_vec(),
            options: from.options.clone(),
            switches: from.switches.clone(),
        }
    }

    pub fn option(&self, name: &str) -> Option<&str> {
        self.options
            .get(name)
            .and_then(|values| values.last())
            .map(String::as_str)
    }

    pub fn options(&self, name: &str) -> Vec<&str> {
        self.options
            .get(name)
            .map(|values| values.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn switch(&self, name: &str) -> bool {
        self.switches.iter().any(|switch| switch == name)
    }

    /// A numeric option, or `fallback` when absent.
    ///
    /// # Errors
    /// Reports a value that is not a number.
    pub fn number<T: std::str::FromStr>(&self, name: &str, fallback: T) -> Result<T, String> {
        match self.option(name) {
            None => Ok(fallback),
            Some(text) => text
                .parse()
                .map_err(|_| format!("--{name} takes a number, not `{text}`")),
        }
    }

    /// Three comma- or space-separated numbers, as `x,y,z`.
    ///
    /// # Errors
    /// Reports anything that is not three finite numbers.
    pub fn vec3(text: &str) -> Result<[f32; 3], String> {
        let parts: Vec<f32> = text
            .split([',', ' '])
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<f32>().map_err(|_| ()))
            .collect::<Result<_, ()>>()
            .map_err(|()| format!("`{text}` is not X,Y,Z"))?;
        match parts.as_slice() {
            [x, y, z] if parts.iter().all(|value| value.is_finite()) => Ok([*x, *y, *z]),
            _ => Err(format!("`{text}` is not X,Y,Z")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn reads_options_switches_and_positionals() {
        let args = Args::parse(
            &words("say --to near hello --json --relay=wss://a --relay wss://b"),
            &["json"],
        )
        .unwrap();
        assert_eq!(args.positional(), ["say", "hello"]);
        assert_eq!(args.option("to"), Some("near"));
        assert_eq!(args.options("relay"), ["wss://a", "wss://b"]);
        assert!(args.switch("json"));
        assert!(!args.switch("quiet"));
    }

    #[test]
    fn refuses_a_dangling_option() {
        assert!(Args::parse(&words("look --radius"), &[]).is_err());
    }

    #[test]
    fn parses_vectors() {
        assert_eq!(Args::vec3("1,2.5,-3").unwrap(), [1.0, 2.5, -3.0]);
        assert!(Args::vec3("1,2").is_err());
        assert!(Args::vec3("a,b,c").is_err());
    }
}
