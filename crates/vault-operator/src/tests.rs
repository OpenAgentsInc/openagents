use crate::service::{Config, Service};
use oa_vault::{
    Dek, Error as VaultError,
    object::{self, New, Tier},
    operator::{LocalOperatorKms, OperatorKms, wrap_operator},
};
use serde_json::json;
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use vault_sealed::Signed;
use zeroize::Zeroizing;

const VAULT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OBJ_A: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const OBJ_B: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const VERSION: &str =
    "projects/fixture/locations/global/keyRings/vault/cryptoKeys/operator/cryptoKeyVersions/1";
const KEY: [u8; 32] = [0x6a; 32];
const OPERATOR: u8 = 1;
const ALICE: u8 = 2;
const BOB: u8 = 3;

fn secret(n: u8) -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n)
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}

/// A key service that may use the key and then lose the reply.
struct Faulty {
    inner: LocalOperatorKms,
    lose: Rc<Cell<bool>>,
    decrypts: Rc<Cell<usize>>,
}
impl OperatorKms for Faulty {
    fn key_version(&self) -> String {
        self.inner.key_version()
    }
    fn encrypt(&mut self, aad: &[u8], plaintext: &[u8]) -> oa_vault::Result<Vec<u8>> {
        self.inner.encrypt(aad, plaintext)
    }
    fn decrypt(
        &mut self,
        version: &str,
        aad: &[u8],
        ciphertext: &[u8],
    ) -> oa_vault::Result<Zeroizing<Vec<u8>>> {
        self.decrypts.set(self.decrypts.get() + 1);
        let out = self.inner.decrypt(version, aad, ciphertext);
        if self.lose.get() {
            return Err(VaultError::Refused("The key service timed out."));
        }
        out
    }
}

struct Fx {
    _temp: tempfile::TempDir,
    dir: std::path::PathBuf,
    service: Service,
    lose: Rc<Cell<bool>>,
    decrypts: Rc<Cell<usize>>,
    now: u64,
    counter: u8,
}

fn object_bytes(object: &str, text: &[u8], key: [u8; 32]) -> Vec<u8> {
    let new = New {
        object,
        vault: VAULT,
        tier: Tier::Operator,
        media: None,
        created_at: 1,
    };
    let draft = object::seal_new(&new, text, Vec::new()).unwrap();
    let digest = draft.header.core_digest().unwrap();
    let mut kms = LocalOperatorKms::new(VERSION, key);
    let wrap = wrap_operator(&mut kms, &digest, &draft.dek).unwrap();
    let nonce = oa_vault::unb64_n::<7>(&draft.header.core.content.nonce_prefix).unwrap();
    let _ = Dek::from_bytes([0; 32]);
    object::seal_with(
        &new,
        text,
        vec![wrap],
        draft.dek,
        nonce,
        draft.header.core.content.chunk_bytes,
    )
    .unwrap()
    .bytes
}

impl Fx {
    fn new(key: [u8; 32]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("operator");
        Service::initialize(
            dir.clone(),
            Config {
                operators: BTreeSet::from([pk(OPERATOR)]),
                recipients: BTreeSet::from([pk(ALICE), pk(BOB)]),
            },
        )
        .unwrap();
        let lose = Rc::new(Cell::new(false));
        let decrypts = Rc::new(Cell::new(0));
        let kms = Faulty {
            inner: LocalOperatorKms::new(VERSION, key),
            lose: lose.clone(),
            decrypts: decrypts.clone(),
        };
        let service = Service::open(dir.clone(), Box::new(kms)).unwrap();
        Self {
            _temp: temp,
            dir,
            service,
            lose,
            decrypts,
            now: 1_800_000_000,
            counter: 0x40,
        }
    }
    fn id(&mut self) -> String {
        self.counter += 1;
        format!("{:02x}", self.counter).repeat(32)
    }
    fn grant(
        &mut self,
        signer: u8,
        recipient: u8,
        objects: &[&str],
        ttl: u64,
        uses: u64,
    ) -> Result<String, String> {
        let body = json!({"v":"openagents.vault-operator-grant.v1","requires":[],"grant":self.id(),"vault":VAULT,
            "objects":objects,"recipient":pk(recipient),"purpose":"support","expires_at":self.now+ttl,"max_uses":uses,"issued_at":self.now});
        self.service
            .grant(&Signed::sign(body, &secret(signer)).unwrap(), self.now)
    }
    fn request(
        &mut self,
        signer: u8,
        grant: &str,
        object: &str,
        request: Option<String>,
    ) -> Signed {
        let request = request.unwrap_or_else(|| self.id());
        let body = json!({"v":"openagents.vault-operator-use.v1","requires":[],"request":request,"grant":grant,
            "object":object,"purpose":"support","issued_at":self.now});
        Signed::sign(body, &secret(signer)).unwrap()
    }
}

#[test]
fn a_granted_use_is_audited_before_the_key_is_used() {
    let mut fx = Fx::new(KEY);
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 1).unwrap();
    let bytes = object_bytes(OBJ_A, b"ticket transcript", KEY);
    let request = fx.request(ALICE, &grant, OBJ_A, None);
    let outcome = fx.service.use_object(&request, &bytes, fx.now).unwrap();
    assert_eq!(outcome.status, "completed");
    assert_eq!(outcome.plaintext.unwrap().as_slice(), b"ticket transcript");
    let kinds: Vec<String> = fx
        .service
        .audit()
        .entries()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["grant", "intent", "used"]);
    // A new request after the one allowed use refuses.
    let again = fx.request(ALICE, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&again, &bytes, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("conflict")
    );
    let log = std::fs::read_to_string(fx.dir.join("audit.jsonl")).unwrap();
    assert!(!log.contains("ticket transcript"));
    assert_eq!(fx.service.audit().verify().unwrap(), 3);
}

#[test]
fn wrong_users_wrong_keys_and_unknown_recipients_open_nothing() {
    let mut fx = Fx::new(KEY);
    assert!(
        fx.grant(BOB, ALICE, &[OBJ_A], 900, 1)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    assert!(
        fx.grant(OPERATOR, 9, &[OBJ_A], 900, 1)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    assert!(
        fx.grant(OPERATOR, ALICE, &[OBJ_A], 86_401, 1)
            .unwrap_err()
            .starts_with("limit_exceeded")
    );
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 2).unwrap();
    let bytes = object_bytes(OBJ_A, b"secret", KEY);
    let bob = fx.request(BOB, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&bob, &bytes, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("not_admitted")
    );
    let overbroad = fx.request(ALICE, &grant, OBJ_B, None);
    let other = object_bytes(OBJ_B, b"other", KEY);
    assert_eq!(
        fx.service
            .use_object(&overbroad, &other, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("limit_exceeded")
    );
    // An object wrapped under another key fails, and that failure is audited.
    let foreign = object_bytes(OBJ_A, b"secret", [0x6c; 32]);
    let request = fx.request(ALICE, &grant, OBJ_A, None);
    let failed = fx.service.use_object(&request, &foreign, fx.now).unwrap();
    assert_eq!(failed.status, "failed");
    assert!(
        fx.service
            .audit()
            .entries()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "failed")
    );
}

#[test]
fn replays_redeliver_with_a_new_audited_use_and_conflicts_refuse() {
    let mut fx = Fx::new(KEY);
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 1).unwrap();
    let bytes = object_bytes(OBJ_A, b"secret", KEY);
    let id = fx.id();
    let request = fx.request(ALICE, &grant, OBJ_A, Some(id.clone()));
    fx.service.use_object(&request, &bytes, fx.now).unwrap();
    // The reply was lost: the exact request re-delivers, audited again, and
    // does not spend another use.
    let again = fx.service.use_object(&request, &bytes, fx.now).unwrap();
    assert_eq!(again.status, "completed");
    assert!(
        fx.service
            .audit()
            .entries()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "redelivered")
    );
    // The same request ID with other bytes refuses.
    let mut changed = request.body.clone();
    changed["purpose"] = json!("support ");
    let changed = Signed::sign(changed, &secret(ALICE)).unwrap();
    assert_eq!(
        fx.service
            .use_object(&changed, &bytes, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("idempotency_conflict")
    );
}

#[test]
fn an_unknown_key_service_outcome_reconciles_without_spending_twice() {
    let mut fx = Fx::new(KEY);
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 1).unwrap();
    let bytes = object_bytes(OBJ_A, b"secret", KEY);
    fx.lose.set(true);
    let request = fx.request(ALICE, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&request, &bytes, fx.now)
            .unwrap()
            .status,
        "unknown"
    );
    fx.lose.set(false);
    let retried = fx.service.use_object(&request, &bytes, fx.now).unwrap();
    assert_eq!(retried.status, "completed");
    let kinds: Vec<String> = fx
        .service
        .audit()
        .entries()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["grant", "intent", "unknown", "retry", "used"]);
    // The one allowed use was spent once.
    let next = fx.request(ALICE, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&next, &bytes, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("conflict")
    );
}

#[test]
fn a_use_whose_audit_cannot_be_written_never_touches_the_key() {
    let mut fx = Fx::new(KEY);
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 1).unwrap();
    let bytes = object_bytes(OBJ_A, b"secret", KEY);
    fx.service.audit().fail_next = true;
    let request = fx.request(ALICE, &grant, OBJ_A, None);
    let outcome = fx.service.use_object(&request, &bytes, fx.now).unwrap();
    assert_eq!(outcome.status, "refused");
    assert!(outcome.plaintext.is_none());
    assert_eq!(fx.decrypts.get(), 0);
}

#[test]
fn revocation_expiry_and_audit_tampering_stop_use() {
    let mut fx = Fx::new(KEY);
    let grant = fx.grant(OPERATOR, ALICE, &[OBJ_A], 900, 5).unwrap();
    let bytes = object_bytes(OBJ_A, b"secret", KEY);
    let expired = fx.request(ALICE, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&expired, &bytes, fx.now + 901)
            .unwrap()
            .reason
            .as_deref(),
        Some("stale")
    );
    let revoke = Signed::sign(
        json!({"v":"openagents.vault-operator-revoke.v1","requires":[],"grant":grant}),
        &secret(OPERATOR),
    )
    .unwrap();
    fx.service.revoke(&revoke, fx.now).unwrap();
    let after = fx.request(ALICE, &grant, OBJ_A, None);
    assert_eq!(
        fx.service
            .use_object(&after, &bytes, fx.now)
            .unwrap()
            .reason
            .as_deref(),
        Some("revoked")
    );
    // Editing any audit line breaks the chain, and the service will not open.
    let path = fx.dir.join("audit.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replacen("support", "nothing", 1);
    std::fs::write(&path, text).unwrap();
    assert!(
        Service::open(
            fx.dir.clone(),
            Box::new(LocalOperatorKms::new(VERSION, KEY))
        )
        .is_err()
    );
}
