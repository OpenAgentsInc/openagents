//! Native-bridge transport (ADR 0010).
//!
//! A phone shell loads a zone's release top-level in a platform web view
//! (`WKWebView`, `android.webkit.WebView`) and plays the trusted page's role.
//! Instead of `postMessage` between frames plus a transferred `MessagePort`,
//! messages travel over the web view's own channel, exposed to the page as
//! `window.aethyrHost` with `postMessage(string)` and an `onmessage` handler
//! (Android's `WebViewCompat.addWebMessageListener` object has exactly this
//! shape; the iOS shell injects the same object over a
//! `WKScriptMessageHandler`).
//!
//! The envelopes, schemas, sequence rules, and byte bounds are the same as the
//! browser transport's. What changes is the peer check: instead of "this
//! origin and this iframe window", the shell checks that each frame came from
//! the zone's exact origin and from the main frame, as the platform reports
//! them (`WKScriptMessage.frameInfo`, Android's `sourceOrigin` and
//! `isMainFrame`).
use crate::*;

/// The page-visible name of the native channel and of the iOS script handler.
pub const NATIVE_CHANNEL: &str = "aethyrHost";

/// Where a native frame came from, as the web view reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePeer<'a> {
    /// The serialized origin of the document that sent the frame.
    pub origin: &'a str,
    /// Frames from subframes are never accepted.
    pub main_frame: bool,
}

/// What the shell should do with an accepted frame.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeEvent {
    /// The engine announced itself. Deliver this `hello` envelope to it.
    Hello(String),
    /// A validated message on the session.
    Message(Box<Message>),
}

/// Bounds every native frame before any parsing, as `wire` does for ports.
pub fn native_frame(text: &str) -> Result<&str, Error> {
    if text.len() > MAX_BYTES {
        Err(Error::Invalid("transport size"))
    } else {
        Ok(text)
    }
}

/// Host-side state for one zone document in one web view.
pub struct NativeHost {
    host: Host,
    url: String,
    release: String,
    abi: String,
    format: u16,
    project: Project,
    listened: bool,
}
impl NativeHost {
    pub fn new(
        engine_url: &str,
        session: String,
        release: String,
        abi: String,
        format: u16,
        project: Project,
    ) -> Result<Self, Error> {
        let host = Host::new(
            engine_url,
            session,
            release.clone(),
            abi.clone(),
            format,
            project.clone(),
        )?;
        Ok(Self {
            host,
            url: engine_url.into(),
            release,
            abi,
            format,
            project,
            listened: false,
        })
    }
    /// The exact origin every frame must come from.
    pub fn origin(&self) -> &str {
        &self.host.origin
    }
    pub fn host(&self) -> &Host {
        &self.host
    }
    /// Check the peer and bounds, then answer the bootstrap once per document
    /// or validate an envelope on the session.
    pub fn receive(&mut self, peer: NativePeer<'_>, text: &str) -> Result<NativeEvent, Error> {
        let text = native_frame(text)?;
        if !peer.main_frame || peer.origin != self.host.origin {
            return Err(Error::Peer);
        }
        if text == BOOTSTRAP {
            if self.listened {
                return Err(Error::Replay);
            }
            let hello = self.host.hello()?.encode()?;
            self.listened = true;
            return Ok(NativeEvent::Hello(hello));
        }
        if !self.listened {
            return Err(Error::Phase);
        }
        self.host
            .receive(text)
            .map(|m| NativeEvent::Message(Box::new(m)))
    }
    /// Encode a request for the engine; the shell delivers the string.
    pub fn request(&mut self, action: Action, now: u64) -> Result<String, Error> {
        self.host.request(action, now)?.encode()
    }
    /// A new document loaded in the web view: start a new session, as the
    /// browser host does on a frame load.
    pub fn reload(&mut self, session: String) -> Result<(), Error> {
        let next = Host::new(
            &self.url,
            session,
            self.release.clone(),
            self.abi.clone(),
            self.format,
            self.project.clone(),
        )?;
        self.host.shutdown();
        self.host = next;
        self.listened = false;
        Ok(())
    }
    pub fn expire(&mut self, now: u64) {
        self.host.expire(now)
    }
    pub fn shutdown(&mut self) {
        self.host.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://engine.example";
    fn project() -> Project {
        Project {
            id: "haunted-house".into(),
            branch: "main".into(),
            revision: 0,
        }
    }
    fn peer() -> NativePeer<'static> {
        NativePeer {
            origin: ORIGIN,
            main_frame: true,
        }
    }
    fn host() -> NativeHost {
        NativeHost::new(
            "https://engine.example/",
            "native-session-1".into(),
            "release-1".into(),
            "abi-1".into(),
            1,
            project(),
        )
        .unwrap()
    }
    fn status() -> Status {
        Status {
            project: project(),
            generation: "generation-1".into(),
            frame: 1,
            save: SaveState::Unsaved,
            checkpoint: None,
            selection: None,
            authoring: None,
            view: None,
            npc: None,
        }
    }

    #[test]
    fn the_bootstrap_names_this_protocol_version() {
        assert_eq!(BOOTSTRAP, format!("{PROTOCOL}/{VERSION}/listening"));
    }

    #[test]
    fn handshake_open_and_status_run_over_the_native_channel() {
        let mut h = host();
        assert_eq!(h.origin(), ORIGIN);
        let NativeEvent::Hello(hello) = h.receive(peer(), BOOTSTRAP).unwrap() else {
            panic!("hello")
        };
        let (mut engine, ready) = Engine::accept(
            &hello,
            "release-1".into(),
            "abi-1".into(),
            1,
            vec![Capability::Inspect, Capability::Observe],
        )
        .unwrap();
        let ready = ready.encode().unwrap();
        assert!(matches!(
            h.receive(peer(), &ready).unwrap(),
            NativeEvent::Message(m) if matches!(*m, Message::Ready { .. })
        ));
        let open = h.request(Action::Open { checkpoint: None }, 0).unwrap();
        let Message::Request { request_id, .. } = engine.receive(&open).unwrap() else {
            panic!("request")
        };
        let reply = engine
            .reply(
                &request_id,
                Outcome {
                    state: ResultState::Applied,
                    message: "Opened.".into(),
                    status: Some(status()),
                    detail: None,
                },
            )
            .unwrap()
            .encode()
            .unwrap();
        h.receive(peer(), &reply).unwrap();
        assert_eq!(h.host().phase, Phase::Open);
        // Replaying the same envelope is refused.
        assert_eq!(h.receive(peer(), &reply), Err(Error::Replay));
    }

    #[test]
    fn frames_from_other_origins_or_subframes_are_refused() {
        let mut h = host();
        for bad in [
            NativePeer {
                origin: "https://evil.example",
                main_frame: true,
            },
            NativePeer {
                origin: "https://engine.example:444",
                main_frame: true,
            },
            NativePeer {
                origin: "http://engine.example",
                main_frame: true,
            },
            NativePeer {
                origin: ORIGIN,
                main_frame: false,
            },
        ] {
            assert_eq!(h.receive(bad, BOOTSTRAP), Err(Error::Peer));
        }
        // Nothing was consumed: the real peer can still bootstrap once.
        assert!(matches!(
            h.receive(peer(), BOOTSTRAP),
            Ok(NativeEvent::Hello(_))
        ));
        assert_eq!(h.receive(peer(), BOOTSTRAP), Err(Error::Replay));
    }

    #[test]
    fn envelopes_before_bootstrap_and_oversized_frames_fail_closed() {
        let mut h = host();
        assert_eq!(h.receive(peer(), "{}"), Err(Error::Phase));
        let big = "x".repeat(MAX_BYTES + 1);
        assert_eq!(
            h.receive(peer(), &big),
            Err(Error::Invalid("transport size"))
        );
        assert_eq!(
            native_frame(&"x".repeat(MAX_BYTES)).map(str::len),
            Ok(MAX_BYTES)
        );
    }

    #[test]
    fn a_new_document_gets_a_new_session_and_listens_again() {
        let mut h = host();
        let NativeEvent::Hello(first) = h.receive(peer(), BOOTSTRAP).unwrap() else {
            panic!()
        };
        h.reload("native-session-2".into()).unwrap();
        let NativeEvent::Hello(second) = h.receive(peer(), BOOTSTRAP).unwrap() else {
            panic!()
        };
        assert_ne!(
            Envelope::decode(&first).unwrap().session,
            Envelope::decode(&second).unwrap().session
        );
        assert!(h.reload("bad session!".into()).is_err());
    }
}
