use super::sso::{self, Audience, FixtureVerifier, Jwk, Linking, Outcome, SsoRefusal, Terms};
use super::{Accounts, Refusal, Role, WorkspaceKind};
use std::collections::BTreeSet;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn terms(linking: Linking) -> Terms {
    Terms {
        issuer: "https://accounts.google.com".into(),
        tenant: "example.com".into(),
        audience: "client-1".into(),
        linking,
        keys: vec![Jwk {
            kid: "k1".into(),
            n: "AQAB".into(),
            e: "AQAB".into(),
        }],
        audit_fields: ["sub", "email"].into_iter().map(String::from).collect(),
    }
}
fn token(v: &FixtureVerifier, kid: &str, claims: serde_json::Value) -> String {
    let h = sso::b64url_encode(
        serde_json::json!({"alg":"fixture","kid":kid})
            .to_string()
            .as_bytes(),
    );
    let c = sso::b64url_encode(claims.to_string().as_bytes());
    let input = format!("{h}.{c}");
    let sig = sso::b64url_encode(&v.sign(kid, input.as_bytes()));
    format!("{input}.{sig}")
}
fn claims(sub: &str, email: &str) -> serde_json::Value {
    serde_json::json!({
        "iss": "https://accounts.google.com", "aud": "client-1", "sub": sub,
        "hd": "example.com", "email": email, "email_verified": true,
        "iat": now() - 5, "exp": now() + 600, "nonce": "n1"
    })
}

#[test]
fn sso_signs_in_only_admitted_linked_members_and_exports_scoped_audit() {
    let dir = std::env::temp_dir().join(format!("sso-test-{}-{}", std::process::id(), now()));
    std::fs::create_dir_all(&dir).unwrap();
    let accounts = Accounts::install(&dir).unwrap();
    let owner = accounts.create_account("owner@example.com", &[]).unwrap();
    let member = accounts.create_account("pat@example.com", &[]).unwrap();
    let outsider = accounts.create_account("pat@other.com", &[]).unwrap();
    let ws = accounts
        .create_workspace(&owner.id, "Team", WorkspaceKind::Organization, "team", None)
        .unwrap();
    let inv = accounts
        .invite(&owner.id, &ws.id, Role::Member, 3600)
        .unwrap();
    accounts.accept(&member.id, &inv.token).unwrap();
    let other = accounts
        .create_workspace(
            &outsider.id,
            "Other",
            WorkspaceKind::Organization,
            "other",
            None,
        )
        .unwrap();
    let v = FixtureVerifier {
        secret: b"fixture".to_vec(),
    };
    let forged = FixtureVerifier {
        secret: b"forged".to_vec(),
    };

    // No provider yet: nothing signs in. A member cannot configure one.
    assert!(matches!(
        accounts.sso_sign_in(
            &ws.id,
            &token(&v, "k1", claims("s1", "pat@example.com")),
            &v
        ),
        Err(Refusal::Sso(SsoRefusal::NoProvider(_)))
    ));
    assert!(matches!(
        accounts.sso_configure(&member.id, &ws.id, &owner.id, terms(Linking::LinkedOnly)),
        Err(Refusal::Forbidden { .. })
    ));
    let mut bad = terms(Linking::LinkedOnly);
    bad.issuer = "http://accounts.google.com".into();
    assert!(
        accounts
            .sso_configure(&owner.id, &ws.id, &owner.id, bad)
            .is_err()
    );
    let rev = accounts
        .sso_configure(&owner.id, &ws.id, &owner.id, terms(Linking::LinkedOnly))
        .unwrap();
    assert_eq!(rev.version, 1);

    // Verified but unlinked: refused. Forged, wrong tenant, wrong audience,
    // expired, unknown kid: all one denial.
    let good = token(&v, "k1", claims("s1", "pat@example.com"));
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, &good, &v),
        Err(Refusal::Sso(SsoRefusal::Unlinked))
    ));
    accounts
        .sso_link(&owner.id, &ws.id, &member.id, "s1")
        .unwrap();
    assert!(matches!(
        accounts.sso_link(&owner.id, &ws.id, &owner.id, "s1"),
        Err(Refusal::PrincipalTaken { .. })
    ));
    assert!(matches!(
        accounts.sso_link(&member.id, &ws.id, &member.id, "s2"),
        Err(Refusal::Forbidden { .. })
    ));
    for (t, why) in [
        (
            token(&forged, "k1", claims("s1", "pat@example.com")),
            "forged",
        ),
        (
            token(&v, "k9", claims("s1", "pat@example.com")),
            "unknown kid",
        ),
        (
            token(&v, "k1", {
                let mut c = claims("s1", "pat@example.com");
                c["hd"] = "evil.com".into();
                c
            }),
            "tenant",
        ),
        (
            token(&v, "k1", {
                let mut c = claims("s1", "pat@example.com");
                c["aud"] = "client-2".into();
                c
            }),
            "audience",
        ),
        (
            token(&v, "k1", {
                let mut c = claims("s1", "pat@example.com");
                c["exp"] = (now() - 1).into();
                c
            }),
            "expired",
        ),
    ] {
        assert!(
            matches!(
                accounts.sso_sign_in(&ws.id, &t, &v),
                Err(Refusal::Sso(SsoRefusal::Denied))
            ),
            "{why}"
        );
    }
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, "not.a.jws.at.all", &v),
        Err(Refusal::Sso(SsoRefusal::Malformed))
    ));
    // Cross-tenant: the other workspace has no provider; the same token never
    // signs in there.
    assert!(accounts.sso_sign_in(&other.id, &good, &v).is_err());

    let admitted = accounts.sso_sign_in(&ws.id, &good, &v).unwrap();
    assert_eq!(admitted.account, member.id);
    assert_eq!(admitted.role, Role::Member);
    assert_eq!(admitted.provider_digest, rev.digest);
    // Replay of the same token refuses.
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, &good, &v),
        Err(Refusal::Sso(SsoRefusal::Denied))
    ));
    // The principal resolves through the existing account path.
    assert_eq!(
        accounts
            .account_of_principal(&sso::principal("https://accounts.google.com", "s1"))
            .unwrap()
            .as_deref(),
        Some(member.id.as_str())
    );

    // Verified-domain linking: a fresh subject maps to the one member whose
    // label is the verified email; an unverified or foreign email refuses.
    let rev2 = accounts
        .sso_configure(
            &owner.id,
            &ws.id,
            &owner.id,
            terms(Linking::VerifiedEmailDomain),
        )
        .unwrap();
    assert_eq!(rev2.version, 2);
    assert_eq!(rev2.supersedes.as_deref(), Some(rev.digest.as_str()));
    let mut unverified = claims("s3", "owner@example.com");
    unverified["email_verified"] = false.into();
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, &token(&v, "k1", unverified), &v),
        Err(Refusal::Sso(SsoRefusal::Unlinked))
    ));
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, &token(&v, "k1", claims("s4", "pat@other.com")), &v),
        Err(Refusal::Sso(SsoRefusal::Unlinked))
    ));
    let o = accounts
        .sso_sign_in(
            &ws.id,
            &token(&v, "k1", claims("s3", "owner@example.com")),
            &v,
        )
        .unwrap();
    assert_eq!(
        (o.account.as_str(), o.role),
        (owner.id.as_str(), Role::Owner)
    );

    // Removed membership blocks a still-valid, still-linked subject.
    accounts
        .remove_member(&owner.id, &ws.id, &member.id)
        .unwrap();
    let mut fresh = claims("s1", "pat@example.com");
    fresh["nonce"] = "n2".into();
    assert!(matches!(
        accounts.sso_sign_in(&ws.id, &token(&v, "k1", fresh), &v),
        Err(Refusal::Sso(SsoRefusal::NotMember { .. }))
    ));

    // Audit: admin/owner only, this workspace only, references and the
    // reviewed fields, no token material.
    assert!(matches!(
        accounts.sso_audit(&outsider.id, &ws.id, 0),
        Err(Refusal::NotMember { .. })
    ));
    let audit = accounts.sso_audit(&owner.id, &ws.id, 0).unwrap();
    assert!(audit.iter().all(|a| a.workspace == ws.id));
    let signed: Vec<_> = audit
        .iter()
        .filter(|a| a.outcome == Outcome::SignedIn)
        .collect();
    assert_eq!(signed.len(), 2);
    assert_eq!(
        signed[0].fields.get("email").map(String::as_str),
        Some("pat@example.com")
    );
    assert!(
        signed[0].fields.get("nonce").is_none(),
        "unreviewed claims are not exported"
    );
    let dumped = serde_json::to_string(&audit).unwrap();
    assert!(!dumped.contains(&good) && !dumped.contains("fixture"));
    assert!(
        audit
            .iter()
            .any(|a| a.outcome == Outcome::Refused && a.reason == "replay")
    );
    assert!(
        audit
            .iter()
            .filter(|a| a.outcome == Outcome::Configured)
            .count()
            == 2
    );
    let _ = (Audience::One(String::new()), BTreeSet::<String>::new());
    std::fs::remove_dir_all(&dir).ok();
}
