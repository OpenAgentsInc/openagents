//! Consent is checked for the bytes retained, independently of capture purpose.
use super::*;

impl Controller {
    pub(super) fn retention_consent(
        &self,
        state: &State,
        session: &Session,
        track: &Value,
        rights: &[&str],
        now: u64,
    ) -> Result<()> {
        if track["retention"]["mode"] != "bounded"
            || !track["retention"]["until"]
                .as_u64()
                .is_some_and(|until| until > now)
        {
            return Err("not_admitted: track retention is unavailable".into());
        }
        let track_id = track["id"].as_str().ok_or("malformed: track id")?;
        let rights: Vec<String> = rights.iter().map(|r| (*r).into()).collect();
        for (participant, member) in session.members.iter().filter(|(_, m)| {
            m.tracks.iter().any(|t| t == track_id) && m.rights.iter().any(|r| r == "publish")
        }) {
            if member.expires_at <= now {
                return Err("stale: source participant expired".into());
            }
            self.consented(
                state,
                session,
                participant,
                &rights,
                &[track_id.into()],
                &member.consent,
                now,
            )?;
        }
        Ok(())
    }
}

impl Controller {
    /// Record expiry before writing media, so a crash cannot orphan an
    /// untracked capture. Equal bytes remain until the latest admitted expiry.
    pub(super) fn retain_media(
        &self,
        state: &mut State,
        bytes: &[u8],
        media: &str,
        until: u64,
    ) -> Result<Value> {
        let reference = workspace_host::artifact::reference(bytes, media, None);
        let digest = digest_of(&reference).to_owned();
        let mut durable = self.state()?;
        let previous = durable
            .media_retention
            .get(&digest)
            .map_or(0, |(_, expiry)| *expiry);
        let expiry = until.max(previous);
        durable
            .media_retention
            .insert(digest.clone(), (reference.clone(), expiry));
        self.save(&durable)?;
        state
            .media_retention
            .insert(digest, (reference.clone(), expiry));
        self.store.put(bytes, media, None)
    }
    pub(super) fn expire_media_in(&self, state: &mut State, now: u64) -> Result<()> {
        let expired: Vec<_> = state
            .media_retention
            .iter()
            .filter(|(_, (_, until))| *until <= now)
            .map(|(digest, (reference, _))| (digest.clone(), reference.clone()))
            .collect();
        if expired.is_empty() {
            return Ok(());
        }
        for (digest, reference) in expired {
            let parsed = contracts::parse_artifact(&reference).map_err(contract)?;
            let path = self
                .store
                .root()
                .join("artifacts")
                .join(&parsed.digest[7..]);
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("unavailable: expired media cleanup: {e}")),
            }
            state.media_retention.remove(&digest);
        }
        self.save(state)
    }
    /// The resident calls this while idle and before reads and new operations.
    /// Observation and transcript references remain; expired content is absent.
    pub fn expire_media(&self, now: u64) -> Result<()> {
        let _guard = self.store.lock()?;
        self.expire_media_in(&mut self.state()?, now)?;
        if let Some(sink) = self.sink.borrow_mut().as_mut() {
            sink.expire(now)?;
        }
        Ok(())
    }
}
