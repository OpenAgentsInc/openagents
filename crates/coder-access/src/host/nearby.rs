//! Nearby approval (NIP-HOST): after the person at the host clicks
//! **Connect** while both screens show the same confirmation code, the host
//! signs an ordinary grant for the device key the nearby exchange named.
//!
//! The exchange, its code, and the click live in the host's serving code;
//! this is only the grant. Its origin is `approval` with a fresh enrollment
//! ID and the host as issuer, its rights are the pairing rights
//! ([`Rights::pairing`]), and it lasts the 30 days a connect-code grant lasts.
use super::*;

impl Host {
    /// Issues and stores a nearby-approval grant for `device` on `relay`
    /// with the pairing `rights`, and returns the grant envelope, encrypted
    /// to `device`. Call it only after the person's click.
    ///
    /// # Errors
    /// Refuses rights other than [`Rights::pairing`], a relay the policy
    /// forbids, a malformed device key, and the owner's or host's own key.
    pub fn approve_nearby(
        &self,
        device: &str,
        relay: &str,
        rights: Rights,
        now: u64,
    ) -> Result<Event> {
        if !is_connect_code_rights(&rights) {
            return fail(
                Code::Forbidden,
                "nearby approval grants only the pairing rights",
            );
        }
        public(device)?;
        self.policy.validate(relay).map_err(Error::from)?;
        let (mut store, secret, mut book) = self.open()?;
        if device == book.owner || device == book.host {
            return fail(Code::Forbidden, "owner and host keys cannot enroll");
        }
        let origin = Origin {
            kind: OriginKind::Approval,
            id: random_id(),
            issuer: book.host.clone(),
        };
        let authorization = issue(
            &mut book,
            &secret,
            device,
            relay,
            rights,
            origin,
            now,
            now + MAX_GRANT_LIFETIME,
        )?;
        store.save(&book)?;
        Ok(authorization)
    }
}

/// [`Rights::pairing`]: what a connect code or a nearby approval grants.
#[must_use]
pub fn is_connect_code_rights(rights: &Rights) -> bool {
    *rights == Rights::pairing()
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::SecretKey;

    const RELAY: &str = "ws://127.0.0.1:7777";

    fn key() -> SecretKey {
        SecretKey::new(&mut secp256k1::rand::rng())
    }

    fn host() -> (tempfile::TempDir, Host, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::new(dir.path().join("host"), RelayPolicy::LoopbackTest);
        let owner = pubkey(&key());
        let host_key = host.init(&owner).unwrap();
        (dir, host, owner, host_key)
    }

    #[test]
    fn a_click_signs_an_approval_grant_the_device_accepts() {
        let (_dir, host, owner, host_key) = host();
        let device = key();
        let now = crate::unix_time().unwrap();
        let rights = Rights::pairing();
        let envelope = host
            .approve_nearby(&pubkey(&device), RELAY, rights.clone(), now)
            .unwrap();
        let access = Access::from_authorization(
            envelope,
            &device,
            &host_key,
            now,
            RelayPolicy::LoopbackTest,
        )
        .unwrap();
        let grant = access.grant;
        assert_eq!(grant.origin.kind, OriginKind::Approval);
        assert_eq!(grant.origin.issuer, host_key);
        assert_eq!(grant.owner, owner);
        assert_eq!(grant.rights, rights);
        assert_eq!(grant.relay, RELAY);
        assert_eq!(grant.expires_at, now + MAX_GRANT_LIFETIME);
        assert_eq!(host.devices(now).unwrap().len(), 1);
        // Another device key cannot open it.
        let other = Access::from_authorization(
            host.approve_nearby(&pubkey(&device), RELAY, Rights::pairing(), now)
                .unwrap(),
            &key(),
            &host_key,
            now,
            RelayPolicy::LoopbackTest,
        );
        assert!(other.is_err());
    }

    #[test]
    fn only_the_pairing_rights_and_only_a_device_key() {
        let (_dir, host, owner, host_key) = host();
        let now = crate::unix_time().unwrap();
        let device = pubkey(&key());
        for other in [
            "standard",
            "admin",
            "observe",
            "observe,operate",
            "observe,operate,terminal",
        ] {
            let error = host
                .approve_nearby(&device, RELAY, Rights::parse_list(other).unwrap(), now)
                .unwrap_err();
            assert_eq!(error.code, Code::Forbidden, "{other}");
        }
        let rights = Rights::pairing();
        for principal in [&owner, &host_key] {
            let error = host
                .approve_nearby(principal, RELAY, rights.clone(), now)
                .unwrap_err();
            assert_eq!(error.code, Code::Forbidden);
        }
        assert!(host.approve_nearby("npub", RELAY, rights, now).is_err());
        assert!(host.devices(now).unwrap().is_empty());
    }
}
