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
