//! Named Claude Code logins (#12211): several subscription tokens a host
//! can rotate through when one login's usage limit runs out.
//!
//! The capacity book ([`crate::capacity::Book::logins`]) keeps each login's
//! name and the path of its token file, never the token. The token comes
//! from `claude setup-token` and lives in its own `0600` file under
//! [`token_dir`]: `$OA_SECRETS_DIR/claude-logins`, else
//! `~/work/.secrets/claude-logins`, the folder the operator's other Claude
//! token already lives in. A login runs Claude Code with the token as
//! `CLAUDE_CODE_OAUTH_TOKEN` alone, never beside an API key, and with
//! [`LOGIN_VAR`] naming the login, so a session launched on it can record
//! a limit against the right login.
//!
//! The Claude Code sign-in on this computer is always the first login
//! ([`DEFAULT`]); named logins follow in the order they were added.

use std::path::{Path, PathBuf};

/// The name the signed-in Claude Code login goes by. It can't be added.
pub const DEFAULT: &str = "default";

/// The variable a launch on a named login sets to the login's name.
pub const LOGIN_VAR: &str = "OA_CLAUDE_LOGIN";

/// The variable Claude Code reads a subscription token from.
pub const TOKEN_VAR: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// The variables a launch on a named login removes, so an API key never
/// takes precedence over the login's token.
pub const API_KEY_VARS: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// What a subscription token from `claude setup-token` starts with.
pub const TOKEN_PREFIX: &str = "sk-ant-oat";

/// The folder login tokens are kept in: `$OA_SECRETS_DIR/claude-logins`,
/// else `~/work/.secrets/claude-logins`. `None` without a home folder.
#[must_use]
pub fn token_dir() -> Option<PathBuf> {
    let secrets = std::env::var_os("OA_SECRETS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join("work/.secrets")))?;
    Some(secrets.join("claude-logins"))
}

/// Whether `name` can name a login: 1 to 64 letters, digits, `.`, `_`, or
/// `-`, not starting with `.` or `-`, and not [`DEFAULT`].
///
/// # Errors
/// Says what is wrong with the name.
pub fn check_name(name: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if name.is_empty() || name.len() > 64 || !name.chars().all(allowed) {
        return Err(format!(
            "`{name}` can't name a login; use up to 64 letters, digits, dots, dashes, or underscores"
        ));
    }
    if name.starts_with(['.', '-']) {
        return Err(format!("a login name can't start with `{}`", &name[..1]));
    }
    if name == DEFAULT {
        return Err(format!(
            "`{DEFAULT}` is the Claude Code sign-in on this computer; pick another name"
        ));
    }
    Ok(())
}

/// A subscription token. It is never printed: its `Debug` says only that
/// it is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// The token from text such as `claude setup-token` printed: the first
    /// word that starts with [`TOKEN_PREFIX`], cut at the first character
    /// a token can't hold. `None` when the text holds no such word.
    #[must_use]
    pub fn find(text: &str) -> Option<Token> {
        let token_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_');
        text.split(|c: char| !token_char(c))
            .find(|word| word.starts_with(TOKEN_PREFIX) && word.len() > TOKEN_PREFIX.len() + 8)
            .map(|word| Token(word.to_owned()))
    }

    /// The token's text, for the launch's environment only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(redacted)")
    }
}

/// A named login chosen for a launch: its name and its token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub name: String,
    pub token: Token,
}

impl Chosen {
    /// The variables a launch on this login sets: the token and the name.
    #[must_use]
    pub fn vars(&self) -> [(&'static str, &str); 2] {
        [(TOKEN_VAR, self.token.expose()), (LOGIN_VAR, &self.name)]
    }
}

/// Saves `token` for the login `name` in `dir`, as `dir/NAME` (`0600`, the
/// folder `0700`), replacing an earlier one. Returns the file's path.
///
/// # Errors
/// Reports a bad name or a file that can't be written.
pub fn save_token(dir: &Path, name: &str, token: &Token) -> Result<PathBuf, String> {
    check_name(name)?;
    let path = dir.join(name);
    let partial = dir.join(format!(".{name}.partial"));
    let mut file = crate::capacity::open_private(dir, &partial)?;
    use std::io::Write;
    file.set_len(0)
        .and_then(|()| file.write_all(token.expose().as_bytes()))
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", partial.display()))?;
    drop(file);
    std::fs::rename(&partial, &path).map_err(|_| format!("cannot write {}", path.display()))?;
    Ok(path)
}

/// The token saved at `path`, or `None` when the file is missing or holds
/// no token.
#[must_use]
pub fn read_token(path: &Path) -> Option<Token> {
    Token::find(&std::fs::read_to_string(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "sk-ant-oat01-AbCdEfGhIjKlMnOp_qrstu-vwxyz0123456789";

    #[test]
    fn a_token_is_found_in_setup_token_output_and_never_printed() {
        let printed = format!(
            "✓ Long-lived authentication token created successfully!\n\nYour OAuth token (valid for 1 year):\n\n{TOKEN}\n\nStore this token securely.\n"
        );
        let token = Token::find(&printed).unwrap();
        assert_eq!(token.expose(), TOKEN);
        assert_eq!(Token::find(TOKEN).unwrap().expose(), TOKEN);
        assert_eq!(
            Token::find(&format!("  {TOKEN}\n")).unwrap().expose(),
            TOKEN
        );
        assert!(Token::find("no token here").is_none());
        assert!(Token::find("sk-ant-api03-key").is_none());
        assert!(Token::find("sk-ant-oat").is_none());
        let chosen = Chosen {
            name: "alt".into(),
            token,
        };
        assert!(!format!("{chosen:?}").contains("AbCd"));
    }

    #[test]
    fn names_are_plain_words_and_never_the_signed_in_login() {
        for good in ["alt", "work-2", "a.b_c", "X9"] {
            assert!(check_name(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "default",
            "../x",
            "a/b",
            ".hidden",
            "-x",
            "a b",
            &"x".repeat(65),
        ] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_saved_token_is_private_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let logins = dir.path().join("claude-logins");
        let token = Token::find(TOKEN).unwrap();
        let path = save_token(&logins, "alt", &token).unwrap();
        assert_eq!(path, logins.join("alt"));
        assert_eq!(read_token(&path), Some(token.clone()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600);
            assert_eq!(mode(&logins), 0o700);
        }
        assert!(save_token(&logins, "../escape", &token).is_err());
        assert_eq!(read_token(&logins.join("missing")), None);
    }
}
