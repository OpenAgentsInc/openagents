//! Fresh page-memory enrollment for one explicitly disclosed native host.
use super::{Admission, Error, Result, terminal_generation};
use coder_access::{
    RelayPolicy, Right,
    client::{Pending, finish_redeem, prepare_redeem},
    protocol::{Access, HostInvitation},
};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

/// Public route and presentation pins. These values supply no native rights.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pins {
    pub host: String,
    pub relay: String,
    pub generation: u64,
    pub workspace: String,
    #[serde(default)]
    pub route: Option<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub loopback: bool,
    #[serde(default)]
    pub terminal: Option<coder_pty::wire::TerminalRef>,
    #[serde(default)]
    pub session: Option<String>,
    /// An engine sign-in to run in a fresh host terminal once enrolled.
    #[serde(default)]
    pub sign_in: Option<SignIn>,
}
/// Run one engine's own sign-in program, unmodified and with no arguments,
/// in a new terminal on the user's computer. The user completes the
/// engine vendor's flow there; the page never sees or keeps the login.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignIn {
    /// The absolute program path on the host.
    pub program: String,
    /// The host workspace common ID the terminal opens in.
    pub workspace: String,
}
impl SignIn {
    /// The exact terminal open request: the program, no arguments, and no
    /// environment, so every sign-in method the program offers stays.
    #[must_use]
    pub fn open(&self, request: String) -> coder_pty::wire::Open {
        coder_pty::wire::Open::new(
            request,
            self.workspace.clone(),
            "",
            coder_pty::wire::Launch::Command {
                program: self.program.clone(),
                args: vec![],
            },
            coder_pty::wire::Size::new(24, 80),
        )
    }
}
impl Pins {
    /// Require secure WebSockets, except for an explicitly opted-in same-host loopback fixture.
    pub fn validate(&self, page_origin: &str) -> Result<RelayPolicy> {
        if !id(&self.host) || self.host.parse::<secp256k1::XOnlyPublicKey>().is_err() {
            return Err(Error::Malformed);
        }
        if self.generation == 0
            || self.workspace.is_empty()
            || self.workspace.len() > 128
            || self.workspace.chars().any(char::is_control)
            || self.capabilities.len() > 64
            || self
                .capabilities
                .iter()
                .any(|s| s.is_empty() || s.len() > 128 || !s.bytes().all(|b| b.is_ascii_graphic()))
        {
            return Err(Error::Malformed);
        }
        if page_origin.len() > 2048
            || self.relay.len() > 2048
            || self.route.as_ref().is_some_and(|s| s.len() > 2048)
        {
            return Err(Error::Limit);
        }
        let page = url::Url::parse(page_origin).map_err(|_| Error::Malformed)?;
        if !matches!(page.scheme(), "http" | "https")
            || page.host_str().is_none()
            || !page.username().is_empty()
            || page.password().is_some()
            || page.fragment().is_some()
            || page.query().is_some()
            || page.path() != "/"
        {
            return Err(Error::Malformed);
        }
        let local = self.loopback && loopback(&page) && page.scheme() == "http";
        if page.scheme() != "https" && !local {
            return Err(Error::NotAdmitted);
        }
        for route in std::iter::once(self.relay.as_str()).chain(self.route.as_deref()) {
            let url = url::Url::parse(route).map_err(|_| Error::Malformed)?;
            if !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
                || url.query().is_some()
                || url.host_str().is_none()
                || !(url.scheme() == "wss"
                    || (local
                        && url.scheme() == "ws"
                        && loopback(&url)
                        && url.host_str() == page.host_str()))
            {
                return Err(Error::NotAdmitted);
            }
        }
        if let Some(terminal) = &self.terminal {
            if !id(&terminal.terminal) || !id(&terminal.generation) {
                return Err(Error::Malformed);
            }
            if terminal.generation != terminal_generation(&self.host, self.generation) {
                return Err(Error::Stale);
            }
        }
        if self.session.as_ref().is_some_and(|s| {
            s.len() != 64
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(Error::Malformed);
        }
        if let Some(sign_in) = &self.sign_in {
            if !id(&sign_in.workspace)
                || !sign_in.program.starts_with('/')
                || sign_in.program.len() > 256
                || sign_in.program.chars().any(char::is_control)
                || self.terminal.is_some()
                || self.session.is_some()
            {
                return Err(Error::Malformed);
            }
            sign_in
                .open(format!("{:064x}", 0))
                .check()
                .map_err(|_| Error::Malformed)?;
        }
        Ok(if local {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        })
    }
}
fn id(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(_)) => false,
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}
/// A fresh device key held by this page. It cannot be imported or serialized.
pub struct Pairing {
    pub(crate) pins: Pins,
    pub(crate) secret: SecretKey,
    pub(crate) policy: RelayPolicy,
}
impl Pairing {
    pub fn new(pins: Pins, page_origin: &str, _now: u64) -> Result<Self> {
        let policy = pins.validate(page_origin)?;
        Ok(Self {
            pins,
            secret: SecretKey::new(&mut secp256k1::rand::rng()),
            policy,
        })
    }
    pub fn pins(&self) -> &Pins {
        &self.pins
    }
    pub(crate) fn prepare(&self, code: &str, now: u64) -> Result<(HostInvitation, Pending)> {
        if code.len() > 16 * 1024 {
            return Err(Error::Limit);
        }
        let invitation =
            HostInvitation::parse(code, now, self.policy).map_err(|_| Error::NotAdmitted)?;
        if invitation.host() != self.pins.host || invitation.relay() != self.pins.relay {
            return Err(Error::NotAdmitted);
        }
        let pending = prepare_redeem(&invitation, &self.secret, now, self.policy)
            .map_err(|_| Error::NotAdmitted)?;
        Ok((invitation, pending))
    }
    pub(crate) fn finish(
        self,
        invitation: &HostInvitation,
        pending: &Pending,
        event: &Event,
        now: u64,
    ) -> Result<Enrolled> {
        let access: Access =
            finish_redeem(invitation, pending, event, &self.secret, now, self.policy)
                .map_err(|_| Error::NotAdmitted)?;
        if !access.grant.rights.contains(Right::Terminal) {
            return Err(Error::NotAdmitted);
        }
        let mut admission =
            Admission::new(self.secret, access, self.policy, self.pins.generation, now)?;
        admission.negotiate(&self.pins.capabilities);
        Ok(Enrolled { admission })
    }
}
impl Drop for Pairing {
    fn drop(&mut self) {
        self.secret.non_secure_erase();
    }
}
/// Verified host admission. No secret, grant, or access serializer is exposed.
pub struct Enrolled {
    admission: Admission,
}
impl Enrolled {
    pub fn into_admission(self) -> Admission {
        self.admission
    }
    pub fn expires_at(&self) -> u64 {
        self.admission.expires_at()
    }
    pub fn device(&self) -> &str {
        self.admission.device()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins() -> Pins {
        Pins {
            host: coder_access::protocol::pubkey(&SecretKey::from_byte_array([1; 32]).unwrap()),
            relay: "wss://relay.example".into(),
            generation: 1,
            workspace: "checkout".into(),
            route: Some("wss://host.example/reach".into()),
            capabilities: vec![],
            loopback: false,
            terminal: None,
            session: None,
            sign_in: None,
        }
    }
    fn redemption(pairing: &Pairing, terminal: bool) -> (HostInvitation, Pending, Event) {
        use coder_access::protocol::*;
        let host = SecretKey::from_byte_array([1; 32]).unwrap();
        let invitation = HostInvitation::from_parts(
            &pubkey(&host),
            &"a".repeat(64),
            &"b".repeat(64),
            &pairing.pins.relay,
            100,
            100 + INVITATION_LIFETIME,
            100,
            pairing.policy,
        )
        .unwrap();
        let pending = prepare_redeem(&invitation, &pairing.secret, 100, pairing.policy).unwrap();
        let grant = Grant {
            v: GRANT.into(),
            requires: vec![],
            grant: "c".repeat(64),
            host: pubkey(&host),
            owner: pubkey(&SecretKey::from_byte_array([2; 32]).unwrap()),
            device: pubkey(&pairing.secret),
            relay: pairing.pins.relay.clone(),
            rights: coder_access::Rights::new(if terminal {
                vec![Right::Terminal, Right::Observe]
            } else {
                vec![Right::Observe]
            })
            .unwrap(),
            epoch: 1,
            origin: Origin {
                kind: OriginKind::Invitation,
                id: invitation.id().into(),
                issuer: pubkey(&SecretKey::from_byte_array([2; 32]).unwrap()),
            },
            issued_at: 100,
            expires_at: 1000,
        };
        let authorization = coder_reach::artifact::seal(
            &grant,
            GRANT,
            &host,
            &grant.device,
            &grant.grant,
            100,
            1000,
        )
        .unwrap();
        let reply = Reply {
            v: REPLY.into(),
            requires: vec![],
            request: pending.request.request.clone(),
            request_event: pending.event.id.clone(),
            host: pubkey(&host),
            issued_at: 100,
            expires_at: pending.request.expires_at,
            result: ReplyResult::Ok {
                outcome: Outcome::Granted {
                    authorization: Box::new(authorization),
                },
            },
        };
        let event = coder_reach::artifact::seal(
            &reply,
            REPLY,
            &host,
            &grant.device,
            &reply.request,
            100,
            reply.expires_at,
        )
        .unwrap();
        (invitation, pending, event)
    }
    #[test]
    fn enrollment_requires_original_reply_and_explicit_terminal_right() {
        let pairing = Pairing::new(pins(), "https://openagents.com", 100).unwrap();
        let (invitation, pending, event) = redemption(&pairing, true);
        let mut changed = pending.clone();
        changed.request.request = "d".repeat(64);
        assert!(
            finish_redeem(
                &invitation,
                &changed,
                &event,
                &pairing.secret,
                100,
                pairing.policy
            )
            .is_err()
        );
        let enrolled = pairing.finish(&invitation, &pending, &event, 100).unwrap();
        assert!(enrolled.admission.current(100));
        assert!(enrolled.admission.can_observe(100));
        assert_eq!(enrolled.expires_at(), 1000);
        assert!(
            enrolled
                .admission
                .prepare_thread(&"1".repeat(32), None, 100)
                .is_ok()
        );
        assert!(matches!(
            enrolled.admission.prepare_thread("bad", None, 100),
            Err(Error::Malformed)
        ));
        let pairing = Pairing::new(pins(), "https://openagents.com", 100).unwrap();
        let (invitation, pending, event) = redemption(&pairing, false);
        assert!(matches!(
            pairing.finish(&invitation, &pending, &event, 100),
            Err(Error::NotAdmitted)
        ));
    }
    #[test]
    fn original_thread_reply_cannot_change_host_identity_or_page_boundary() {
        use coder_access::protocol::*;
        use coder_access::thread::*;
        let pairing = Pairing::new(pins(), "https://openagents.com", 100).unwrap();
        let (invitation, pending, event) = redemption(&pairing, true);
        let a = pairing
            .finish(&invitation, &pending, &event, 100)
            .unwrap()
            .into_admission();
        let read = a.prepare_thread(&"1".repeat(32), Some(1), 100).unwrap();
        let page = ThreadPage {
            thread: "1".repeat(32),
            title: "Synthetic native thread".into(),
            start: 0,
            total: 1,
            turns: vec![ThreadTurn {
                role: ThreadRole::User,
                text: "Original retained turn".into(),
                at: Some(100),
                stopped: false,
                model: None,
                request: None,
                extras: ThreadExtras::default(),
            }],
            busy: false,
            partial: String::new(),
            failure: None,
            coder: None,
            outside: None,
        };
        let answer = |page: ThreadPage, key: u8| {
            let host = SecretKey::from_byte_array([key; 32]).unwrap();
            let reply = Reply {
                v: REPLY.into(),
                requires: vec![],
                request: read.pending.request.request.clone(),
                request_event: read.pending.event.id.clone(),
                host: read.admission.host().into(),
                issued_at: 100,
                expires_at: read.pending.request.expires_at,
                result: ReplyResult::Ok {
                    outcome: Outcome::Thread {
                        thread: Box::new(page),
                    },
                },
            };
            coder_reach::artifact::seal(
                &reply,
                REPLY,
                &host,
                read.admission.device(),
                &reply.request,
                100,
                reply.expires_at,
            )
            .unwrap()
        };
        assert_eq!(read.verify(&answer(page.clone(), 1), 100).unwrap(), page);
        assert!(read.verify(&answer(page.clone(), 3), 100).is_err());
        let mut replaced = page.clone();
        replaced.thread = "2".repeat(32);
        assert!(read.verify(&answer(replaced, 1), 100).is_err());
        let mut replaced = page;
        replaced.start = 1;
        replaced.total = 2;
        assert_eq!(read.verify(&answer(replaced, 1), 100), Err(Error::Stale));
        assert!(
            read.verify(
                &answer(
                    ThreadPage {
                        thread: "1".repeat(32),
                        title: "Synthetic".into(),
                        start: 0,
                        total: 0,
                        turns: vec![],
                        busy: false,
                        partial: String::new(),
                        failure: None,
                        coder: None,
                        outside: None
                    },
                    1
                ),
                1000
            )
            .is_err()
        );
    }
    #[test]
    fn routes_require_secure_or_explicit_same_host_loopback() {
        let mut p = pins();
        assert!(p.validate("https://openagents.com").is_ok());
        p.relay = "ws://127.0.0.1:9999".into();
        assert!(p.validate("https://openagents.com").is_err());
        p.loopback = true;
        p.route = Some("ws://127.0.0.1:8888/reach".into());
        assert!(p.validate("http://127.0.0.1:7777").is_ok());
        assert!(p.validate("http://localhost:7777").is_err());
        assert!(p.validate("http://host.example").is_err());
        p.route = Some("ws://127.0.0.1:8888/reach?credential=no".into());
        assert!(p.validate("http://127.0.0.1:7777").is_err());
    }
    #[test]
    fn engine_sign_in_opens_the_exact_program_with_nothing_added() {
        let mut p = pins();
        let sign_in = SignIn {
            program: "/usr/local/bin/claude".into(),
            workspace: "a".repeat(64),
        };
        p.sign_in = Some(sign_in.clone());
        assert!(p.validate("https://openagents.com").is_ok());
        let open = sign_in.open("b".repeat(64));
        assert_eq!(
            open.launch,
            coder_pty::wire::Launch::Command {
                program: "/usr/local/bin/claude".into(),
                args: vec![]
            }
        );
        assert!(open.env.is_empty() && open.dir.is_empty());
        for bad in [
            SignIn {
                program: "claude".into(),
                workspace: "a".repeat(64),
            },
            SignIn {
                program: "/usr/local/bin/claude".into(),
                workspace: "checkout".into(),
            },
        ] {
            p.sign_in = Some(bad);
            assert!(p.validate("https://openagents.com").is_err());
        }
        p.sign_in = Some(sign_in);
        p.session = Some("c".repeat(64));
        assert!(p.validate("https://openagents.com").is_err());
    }
    #[test]
    fn each_page_generates_a_different_device() {
        let a = Pairing::new(pins(), "https://openagents.com", 1).unwrap();
        let b = Pairing::new(pins(), "https://openagents.com", 1).unwrap();
        assert_ne!(
            coder_access::protocol::pubkey(&a.secret),
            coder_access::protocol::pubkey(&b.secret)
        );
    }
}
