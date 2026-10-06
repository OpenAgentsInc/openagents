//! Private terminal sharing controls. Only an exact host reply confirms a change.
use crate::{
    Application,
    input::{KeyCode, KeyIn, Logical, NamedKey},
};
use coder_pty::{
    share::{ShareMode, Viewers},
    wire::Value,
};
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Issue {
        grantee: String,
        mode: ShareMode,
        expires_at: u64,
    },
    Pause(bool),
    Revoke(Option<String>),
}

impl Action {
    fn confirmation(&self) -> String {
        match self {
            Self::Read => "Refresh the private viewer list.".into(),
            Self::Issue {
                grantee,
                mode,
                expires_at,
            } => format!(
                "Share future output with {grantee} in {} mode until {expires_at}.",
                if *mode == ShareMode::Watch {
                    "watch"
                } else {
                    "drive"
                }
            ),
            Self::Pause(true) => "Pause sharing and blank every recipient pane.".into(),
            Self::Pause(false) => "Resume sharing with a gap; paused output stays hidden.".into(),
            Self::Revoke(Some(share)) => format!("Revoke {share} and detach its recipients."),
            Self::Revoke(None) => "Revoke all shares and detach their recipients.".into(),
        }
    }
}

#[derive(Default)]
pub struct Page {
    pub open: bool,
    pub pane: Option<u64>,
    pub view: Option<Viewers>,
    pub notice: Option<String>,
    pub confirmation: Option<Action>,
    pub scroll: usize,
    pub authorization: Option<String>,
    pending: Option<Receiver<Result<Value, String>>>,
    last_read: Option<Instant>,
    last_seen: Option<Instant>,
    supported: bool,
    enter_down: bool,
}

impl Page {
    pub fn active(&self) -> bool {
        self.view.as_ref().is_some_and(|view| {
            view.paused || !view.shares.is_empty() || view.viewers.len() > 1 || view.agent.is_some()
        })
    }
    pub fn marker(&self) -> String {
        match &self.view {
            None => "SHARING unknown (F17)".into(),
            Some(view) => format!(
                "SHARING {} | {} viewers | {} shares | typist {} (F17)",
                if self
                    .last_seen
                    .is_some_and(|time| time.elapsed() > Duration::from_secs(5))
                {
                    "STALE"
                } else if view.paused {
                    "PAUSED"
                } else if view.shares.is_empty() {
                    "off"
                } else {
                    "LIVE"
                },
                view.viewers.len(),
                view.shares.len(),
                view.agent
                    .as_ref()
                    .map(|agent| agent.agent.as_str())
                    .or_else(|| view
                        .viewers
                        .iter()
                        .find(|v| v.typist)
                        .map(|v| v.device.as_str()))
                    .unwrap_or("none")
            ),
        }
    }
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            "PRIVATE TERMINAL SHARING".into(),
            self.marker(),
            "New shares disclose future output only. Drive also permits input and resize.".into(),
            "Viewer input stays private except when the program echoes it.".into(),
            "F2 refresh; P pause/resume; R revoke all; Enter confirms; Escape cancels.".into(),
            "Issue: /watch DEVICE EXPIRY_UNIX or /drive DEVICE EXPIRY_UNIX".into(),
            "Revoke one: /revoke SHARE_ID. Shares expire within seven days.".into(),
            "C copies the last sealed share authorization. PgUp/PgDn scroll.".into(),
        ];
        if let Some(view) = &self.view {
            for viewer in &view.viewers {
                lines.push(format!(
                    "Viewer {} {:?} {}",
                    viewer.device,
                    viewer.mode,
                    if viewer.typist { "[TYPIST]" } else { "" }
                ));
            }
            for grant in &view.shares {
                lines.push(format!(
                    "Share {} {:?} to {} until {}",
                    grant.share, grant.mode, grant.grantee, grant.expires_at
                ));
            }
        }
        if let Some(action) = &self.confirmation {
            lines.insert(
                2,
                format!(
                    "CONFIRM {} Enter sends once; Escape cancels.",
                    action.confirmation()
                ),
            );
        }
        if self.pending.is_some() {
            lines.insert(
                2,
                "Waiting for the host. No change is confirmed yet.".into(),
            );
        }
        if let Some(notice) = &self.notice {
            lines.insert(2, notice.clone());
        }
        lines
    }
}

impl Application {
    fn sharing_send(&mut self, action: Action) {
        if self.sharing.pending.is_some() {
            return;
        }
        if action == Action::Read {
            self.sharing.last_read = Some(Instant::now());
        }
        let reply = self
            .sharing
            .pane
            .and_then(|id| self.panes.get(&id))
            .and_then(|pane| pane.session.sharing(action));
        match reply {
            Some(reply) => {
                self.sharing.supported = true;
                self.sharing.pending = Some(reply);
            }
            None => {
                self.sharing.notice =
                    Some("This attachment does not support owner sharing controls.".into())
            }
        }
    }
    pub fn sharing_tick(&mut self) {
        if !self.sharing.open
            && self.sharing.pending.is_none()
            && self.sharing.pane != self.paper_pane()
        {
            self.sharing = Page {
                pane: self.paper_pane(),
                ..Page::default()
            };
            if self.sharing.pane.is_some() {
                self.sharing_send(Action::Read);
            }
        }
        let Some(reply) = &self.sharing.pending else {
            if self.sharing.pane == self.paper_pane()
                && self.sharing.pane.is_some()
                && self.sharing.supported
                && self.sharing.confirmation.is_none()
                && self
                    .sharing
                    .last_read
                    .is_some_and(|time| time.elapsed() >= Duration::from_secs(2))
            {
                self.sharing_send(Action::Read);
            }
            return;
        };
        let result = match reply.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(
                "The reply was lost; the change is unknown. Refresh before another action.".into(),
            ),
        };
        self.sharing.pending = None;
        match result {
            Ok(Value::Viewers { viewers }) => {
                self.sharing.view = Some(viewers);
                self.sharing.last_seen = Some(Instant::now());
                self.sharing.notice = None;
            }
            Ok(Value::Shared { authorization, .. }) => {
                self.sharing.authorization = Some(authorization.to_string());
                self.sharing.notice = Some("Share confirmed. C copies its sealed authorization for private delivery to the recipient.".into());
                self.sharing_send(Action::Read);
            }
            Ok(Value::Done) => {
                self.sharing.notice = Some("The host confirmed the change.".into());
                self.sharing_send(Action::Read);
            }
            Ok(_) => {
                self.sharing.notice = Some("Unexpected host reply; refresh to reconcile.".into())
            }
            Err(why) => self.sharing.notice = Some(why),
        }
    }
    pub fn sharing_key(&mut self, key: &KeyIn) -> bool {
        let named = match key.logical {
            Logical::Named(named) => Some(named),
            _ => None,
        };
        if named == Some(NamedKey::F17) && key.pressed && !key.repeat && !key.synthetic {
            if self.sharing.open {
                self.sharing.open = false;
                self.sharing.confirmation = None;
            } else {
                if self.sharing.pending.is_some() {
                    self.sharing.open = true;
                    return true;
                }
                self.paper.on = true;
                self.mouse = crate::mouse::Mouse::default();
                self.sharing = Page {
                    open: true,
                    pane: self.paper_pane(),
                    ..Page::default()
                };
                self.sharing_send(Action::Read);
            }
            return true;
        }
        if !self.sharing.open {
            return false;
        }
        if matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if !key.pressed {
                self.sharing.enter_down = false;
                return true;
            }
            if key.repeat || key.synthetic || self.sharing.enter_down {
                return true;
            }
            self.sharing.enter_down = true;
        }
        if !key.pressed || key.repeat || key.synthetic {
            return true;
        }
        if key.code == KeyCode::Escape {
            if self.sharing.confirmation.take().is_none() {
                self.sharing.open = false;
            }
            return true;
        }
        if named == Some(NamedKey::PageUp) {
            self.sharing.scroll = self.sharing.scroll.saturating_sub(10);
            return true;
        }
        if named == Some(NamedKey::PageDown) {
            self.sharing.scroll = self.sharing.scroll.saturating_add(10);
            return true;
        }
        if named == Some(NamedKey::F2) {
            self.sharing_send(Action::Read);
            return true;
        }
        if matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if self.sharing.pending.is_some() {
                return true;
            }
            if let Some(action) = self.sharing.confirmation.take() {
                self.sharing_send(action);
            } else {
                let input = std::mem::take(&mut self.paper.input);
                self.paper.cursor = 0;
                match parse(&input) {
                    Ok(action) => self.sharing.confirmation = Some(action),
                    Err(why) => self.sharing.notice = Some(why.into()),
                }
            }
            return true;
        }
        if self.sharing.confirmation.is_some() || self.sharing.pending.is_some() {
            return true;
        }
        if let Logical::Character(text) = &key.logical {
            if self.paper.input.is_empty() && text.eq_ignore_ascii_case("c") {
                if let Some(authorization) = self.sharing.authorization.clone() {
                    self.set_clipboard(authorization);
                }
            } else if self.paper.input.is_empty() && text.eq_ignore_ascii_case("p") {
                if let Some(view) = &self.sharing.view {
                    self.sharing.confirmation = Some(Action::Pause(!view.paused));
                }
            } else if self.paper.input.is_empty() && text.eq_ignore_ascii_case("r") {
                self.sharing.confirmation = Some(Action::Revoke(None));
            } else if self.paper.input.len() + text.len() <= 256 {
                self.paper.input.push_str(text);
                self.paper.cursor = self.paper.input.chars().count();
            }
        } else if key.code == KeyCode::Backspace {
            self.paper.input.pop();
            self.paper.cursor = self.paper.input.chars().count();
        }
        true
    }
}

fn parse(input: &str) -> Result<Action, &'static str> {
    let words: Vec<_> = input.split_whitespace().collect();
    let id = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    match words.as_slice() {
        ["/watch" | "/drive", device, expiry] if id(device) => Ok(Action::Issue {
            grantee: device.to_string(),
            mode: if words[0] == "/watch" {
                ShareMode::Watch
            } else {
                ShareMode::Drive
            },
            expires_at: expiry
                .parse()
                .map_err(|_| "Expiry must be a Unix timestamp.")?,
        }),
        ["/revoke", share] if id(share) => Ok(Action::Revoke(Some(share.to_string()))),
        _ => Err("Use /watch DEVICE EXPIRY_UNIX, /drive DEVICE EXPIRY_UNIX, or /revoke SHARE_ID."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issue_is_future_only_and_explicit() {
        let key = "a".repeat(64);
        assert_eq!(
            parse(&format!("/watch {key} 123")),
            Ok(Action::Issue {
                grantee: key,
                mode: ShareMode::Watch,
                expires_at: 123
            })
        );
        assert!(parse("/drive someone 123").is_err());
        assert!(parse(&format!("/watch {} 123 replay", "a".repeat(64))).is_err());
    }
}
