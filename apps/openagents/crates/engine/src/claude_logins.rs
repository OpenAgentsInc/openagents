//! App chats' Claude logins from the host's capacity book (#12215).
//!
//! The book (`capacity.json` in the task store, `OPENAGENTS_TASKS`, else
//! `~/.openagents/tasks`) holds the named Claude logins `openagents capacity
//! add-login` added and each login's recorded limits (#12211). A chat's
//! Claude Code launches on the login it already runs on while that one has
//! capacity, else on the first one that has: the signed-in login first, then
//! the named ones in the order they were added. A usage limit a chat meets is
//! recorded against its login, so the next turn moves to another one.

use std::path::PathBuf;
use std::sync::Arc;

use microcoder_loop::account::Identify;
use microcoder_loop::capacity::{self, Book, Kind, Provider, Refusal};
use microcoder_loop::logins as book_logins;
use openagents_harness::claude::logins::{ClaudeLogins, DEFAULT_LOGIN, UsageLimit};

/// The capacity book in `dir`, as [`ClaudeLogins`].
pub struct BookLogins {
    dir: PathBuf,
    identify: Identify,
    now: fn() -> u64,
}

impl BookLogins {
    /// The book in `dir`, with `identify` naming the signed-in login and
    /// `now` the clock, so a test reads no real login.
    #[must_use]
    pub fn new(dir: PathBuf, identify: Identify, now: fn() -> u64) -> Self {
        Self { dir, identify, now }
    }

    fn book(&self) -> Book {
        Book::load_with(&self.dir, self.identify)
    }
}

/// The host's book, or `None` in tests, which never read the real home.
#[must_use]
pub fn from_home() -> Option<Arc<dyn ClaudeLogins>> {
    if cfg!(test) {
        return None;
    }
    let dir = std::env::var_os("OPENAGENTS_TASKS")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::repos::home_dir().join(".openagents/tasks"));
    Some(Arc::new(BookLogins::new(
        dir,
        microcoder_loop::account::identify,
        microcoder_loop::failover::unix_now,
    )))
}

/// Whether `name` has capacity at `now` in `book` and, for a named login,
/// a token that can be read.
fn usable(book: &Book, name: &str, now: u64) -> bool {
    if name == DEFAULT_LOGIN {
        return book.blocking_login(Provider::Claude, None, now).is_none();
    }
    book.logins_of(Provider::Claude)
        .find(|login| login.name == name)
        .is_some_and(|login| {
            book.blocking_login(Provider::Claude, Some(name), now)
                .is_none()
                && book_logins::read_token(&login.token_file).is_some()
        })
}

impl ClaudeLogins for BookLogins {
    fn choose(&self, live: Option<&str>) -> Option<String> {
        let book = self.book();
        if book.logins_of(Provider::Claude).next().is_none() {
            // No named logins: nothing to choose between.
            return None;
        }
        let now = (self.now)();
        if let Some(live) = live
            && usable(&book, live, now)
        {
            return Some(live.to_owned());
        }
        std::iter::once(DEFAULT_LOGIN)
            .chain(
                book.logins_of(Provider::Claude)
                    .map(|login| login.name.as_str()),
            )
            .find(|name| usable(&book, name, now))
            .map(str::to_owned)
    }

    fn launch_env(&self, name: &str) -> Option<Vec<(String, String)>> {
        if name == DEFAULT_LOGIN {
            return None;
        }
        let book = self.book();
        let login = book.logins_of(Provider::Claude).find(|l| l.name == name)?;
        let chosen = book_logins::Chosen {
            name: login.name.clone(),
            token: book_logins::read_token(&login.token_file)?,
        };
        Some(
            chosen
                .vars()
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        )
    }

    fn record_limit(&self, name: &str, limit: &UsageLimit) {
        let now = (self.now)();
        // A plan window that ran out is a usage limit; without one it is a
        // rate limit, which holds for a short while only.
        let kind = if limit.window.is_some() {
            Kind::UsageLimit
        } else {
            Kind::RateLimit
        };
        let refusal = Refusal::new(Provider::Claude, kind, now, limit.resets_at)
            .on_login((name != DEFAULT_LOGIN).then(|| name.to_owned()));
        if let Err(error) = capacity::record_with(&self.dir, refusal, self.identify) {
            tracing::warn!(login = %name, %error, "the capacity book was not updated");
        } else {
            tracing::info!(login = %name, "recorded a Claude usage limit");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "sk-ant-oat01-fixture-token-0123456789";

    fn nobody(_: Provider) -> Option<String> {
        None
    }
    fn clock() -> u64 {
        1_800_000_000
    }

    fn book_with(names: &[&str]) -> (tempfile::TempDir, BookLogins) {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let tokens = dir.path().join("claude-logins");
        let token = book_logins::Token::find(TOKEN).unwrap();
        for name in names {
            let file = book_logins::save_token(&tokens, name, &token).unwrap();
            capacity::add_login(&store, Provider::Claude, name, &file, 1).unwrap();
        }
        (dir, BookLogins::new(store, nobody, clock))
    }

    fn week() -> UsageLimit {
        UsageLimit {
            resets_at: Some(clock() + 3_600),
            window: Some("seven_day".into()),
        }
    }

    #[test]
    fn without_named_logins_nothing_is_chosen() {
        let (_dir, logins) = book_with(&[]);
        assert_eq!(logins.choose(None), None);
        assert_eq!(logins.choose(Some("default")), None);
        assert_eq!(logins.launch_env(DEFAULT_LOGIN), None);
    }

    #[test]
    fn a_chat_keeps_its_login_and_moves_when_it_runs_out() {
        let (_dir, logins) = book_with(&["alt", "spare"]);
        assert_eq!(logins.choose(None).as_deref(), Some("default"));
        assert_eq!(logins.choose(Some("spare")).as_deref(), Some("spare"));
        logins.record_limit(DEFAULT_LOGIN, &week());
        assert_eq!(logins.choose(Some("default")).as_deref(), Some("alt"));
        // A chat on a login with capacity stays on it.
        assert_eq!(logins.choose(Some("spare")).as_deref(), Some("spare"));
        logins.record_limit("alt", &week());
        assert_eq!(logins.choose(Some("alt")).as_deref(), Some("spare"));
        logins.record_limit("spare", &week());
        assert_eq!(logins.choose(Some("spare")), None, "every login is out");
    }

    #[test]
    fn a_named_login_launches_on_its_token_and_name() {
        let (_dir, logins) = book_with(&["alt"]);
        let env = logins.launch_env("alt").unwrap();
        assert!(env.contains(&("CLAUDE_CODE_OAUTH_TOKEN".into(), TOKEN.into())));
        assert!(env.contains(&("OA_CLAUDE_LOGIN".into(), "alt".into())));
        assert_eq!(logins.launch_env("missing"), None);
    }

    #[test]
    fn a_login_whose_token_is_gone_is_passed_over() {
        let (dir, logins) = book_with(&["alt", "spare"]);
        std::fs::remove_file(dir.path().join("claude-logins/alt")).unwrap();
        logins.record_limit(DEFAULT_LOGIN, &week());
        assert_eq!(logins.choose(Some("default")).as_deref(), Some("spare"));
    }
}
