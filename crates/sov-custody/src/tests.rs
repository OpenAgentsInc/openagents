use crate::{
    custodian::{Bundle, Config, Custodian, envelope},
    signer::{Exchange, InProcessSigner, Nip46, SignerTransport},
};
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::Event;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const CUSTODIAN: u8 = 1;
const AUTHORITY: u8 = 2;
const AGENT: u8 = 3;
const REMOTE: u8 = 4;
const TRANSPORT: u8 = 5;
const CALLER: u8 = 6;
const G1: u8 = 7;
const G2: u8 = 8;
const G3: u8 = 9;
const STRANGER: u8 = 10;
const PEER: u8 = 11;

#[derive(Clone)]
struct Shared(Rc<RefCell<InProcessSigner>>);
impl SignerTransport for Shared {
    fn exchange(&mut self, request: &Event, now: u64) -> Result<Event, Exchange> {
        self.0.borrow_mut().exchange(request, now)
    }
}

fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut v =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        v["schema"] = json!(schema);
    }
    v
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(map, &jcs(body).unwrap(), body["v"].as_str())
}
fn reference(body: &Value) -> Value {
    let bytes = jcs(body).unwrap();
    json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":body["v"]})
}

struct Fx {
    _temp: tempfile::TempDir,
    custodian: Custodian,
    signer: Rc<RefCell<InProcessSigner>>,
    artifacts: BTreeMap<String, Vec<u8>>,
    profile: Value,
    now: u64,
    custody: Value,
    guardian: Value,
    pay: Value,
    counter: u8,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("custody");
        let mut artifacts = BTreeMap::new();
        let profile = put(
            &mut artifacts,
            b"sovereign profile bytes",
            Some("openagents.sovereign-profile.v1"),
        );
        Custodian::initialize(
            root.clone(),
            secret(CUSTODIAN),
            Config {
                authorities: BTreeMap::from([(pk(AGENT), pk(AUTHORITY))]),
                profiles: BTreeMap::from([(pk(AGENT), profile.clone())]),
            },
        )
        .unwrap();
        let custodian = Custodian::open(root, secret(CUSTODIAN)).unwrap();
        let signer = Rc::new(RefCell::new(InProcessSigner::new(
            secret(REMOTE),
            secret(AGENT),
            Some("bunker-secret".into()),
        )));
        custodian
            .mount(
                &pk(AGENT),
                Nip46::new(
                    secret(TRANSPORT),
                    &pk(REMOTE),
                    &pk(AGENT),
                    Box::new(Shared(signer.clone())),
                    Some("bunker-secret".into()),
                )
                .unwrap(),
            )
            .unwrap();
        let pay = json!({"id":format!("{}:sov/pay",pk(AUTHORITY)),"artifact":put(&mut artifacts,b"pay operation",None)});
        Self {
            _temp: temp,
            custodian,
            signer,
            artifacts,
            profile,
            now: crate::now(),
            custody: Value::Null,
            guardian: Value::Null,
            pay,
            counter: 0x40,
        }
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        format!("{:02x}", self.counter).repeat(32)
    }
    fn send(&self, signer: u8, body: &Value, declarations: Vec<Event>) -> Value {
        self.custodian
            .submit(
                &Bundle {
                    event: envelope(&secret(signer), &pk(CUSTODIAN), body, self.now).unwrap(),
                    artifacts: self.artifacts.clone(),
                    declarations,
                },
                self.now,
            )
            .unwrap()
    }
    fn custody_policy(&self, mechanism: &str) -> Value {
        json!({"v":"openagents.sovereign-custody-policy.v1","requires":[],"agent":pk(AGENT),"authority":pk(AUTHORITY),
            "revision":0,"previous":null,"mechanism":mechanism,"signer":pk(REMOTE),"transport":pk(TRANSPORT),
            "purposes":["sign","nip44_encrypt","nip44_decrypt"],"callers":[pk(CALLER)],"recovery":null,
            "exportable":false,"starts_at":self.now-10,"expires_at":self.now+3600})
    }
    fn admit_custody(&mut self) {
        let body = self.custody_policy("nip46");
        assert_eq!(self.send(AUTHORITY, &body, vec![])["status"], "accepted");
        self.custody = reference(&body);
    }
    fn admit_guardians(&mut self, previous: Value, revision: u64) {
        let body = json!({"v":"openagents.sovereign-guardian-policy.v1","requires":[],"agent":pk(AGENT),
            "authority":pk(AUTHORITY),"revision":revision,"previous":previous,"members":[pk(G1),pk(G2),pk(G3)],
            "threshold":2,"operations":[self.pay],"starts_at":self.now-10,"expires_at":self.now+3600});
        let result = self.send(AUTHORITY, &body, vec![]);
        assert_eq!(result["status"], "accepted", "{result}");
        self.guardian = reference(&body);
    }
    fn unsigned(&mut self, content: &str) -> Value {
        let event =
            json!({"pubkey":pk(AGENT),"created_at":self.now,"kind":1,"tags":[],"content":content});
        put(
            &mut self.artifacts,
            &serde_json::to_vec(&event).unwrap(),
            None,
        )
    }
    fn request(
        &mut self,
        purpose: &str,
        input: &Value,
        operation: Value,
        action: Value,
        nonce: Option<String>,
    ) -> Value {
        let nonce = nonce.unwrap_or_else(|| self.next());
        json!({"v":"openagents.sovereign-custody-request.v1","requires":[],"agent":pk(AGENT),"profile":self.profile,
            "policy":self.custody,"guardian":if self.guardian.is_null() {Value::Null} else {self.guardian.clone()},
            "requester":pk(CALLER),"generation":1,"purpose":purpose,"operation":operation,"input":input,
            "action":action,"nonce":nonce,"deadline":self.now+300})
    }
    fn other_op(&mut self) -> Value {
        json!({"id":format!("{}:sov/post",pk(AUTHORITY)),"artifact":put(&mut self.artifacts,b"post operation",None)})
    }
    fn action(&mut self, input: &Value) -> Value {
        let mut a = std::mem::take(&mut self.artifacts);
        let preconditions = put_json(
            &mut a,
            &json!({"v":"openagents.sovereign-action-preconditions.v1","requires":[],
            "profile":self.profile,"guardian_policy":self.guardian,"generation":1,"input":input}),
        );
        let body = json!({"v":"openagents.action.v1","requires":[],"task_frame":put(&mut a,b"frame",None),
            "snapshot":put(&mut a,b"snapshot",None),"operation":self.pay,"binding":put(&mut a,b"binding",None),
            "input":input,"context":put(&mut a,b"context",None),
            "effects":{"reads":[],"writes":["payments"],"network":[],"process":false,"delegates":false,"spend":true},
            "bounds":[],"recipient":null,"preconditions":preconditions,"nonce":self.next()});
        self.artifacts = a;
        body
    }
    fn requests(&mut self, action: &Value) -> Value {
        let result = self.send(CALLER, action, vec![]);
        assert!(
            matches!(result["status"].as_str(), Some("accepted" | "duplicate")),
            "{result}"
        );
        result["requests"].clone()
    }
    fn decide(&mut self, guardian: u8, request: &Value, action: &Value, decision: &str) -> Value {
        let body = json!({"v":"openagents.approval-decision.v1","requires":[],"request":request,"action":reference(action),
            "approver":pk(guardian),"decision":decision,"expires_at":self.now+600,"reason":""});
        self.send(guardian, &body, vec![])
    }
}

#[test]
fn remote_signing_binds_the_exact_request_and_never_repeats_it() {
    let mut fx = Fx::new();
    // A forged authority and unsupported custody mechanisms refuse.
    let body = fx.custody_policy("nip46");
    assert_eq!(fx.send(STRANGER, &body, vec![])["reason"], "not_admitted");
    for mechanism in ["hardware", "threshold"] {
        assert_eq!(
            fx.send(AUTHORITY, &fx.custody_policy(mechanism), vec![])["reason"],
            "cannot_enforce"
        );
    }
    fx.admit_custody();
    let input = fx.unsigned("hello");
    let op = fx.other_op();
    let request = fx.request("sign", &input, op.clone(), Value::Null, None);
    let signed = fx.send(CALLER, &request, vec![]);
    assert_eq!(signed["status"], "completed", "{signed}");
    let receipt = fx.custodian.store().json(&signed["receipt"]).unwrap();
    let event: Event =
        serde_json::from_slice(&fx.custodian.store().get(&receipt["result"]).unwrap()).unwrap();
    assert_eq!(event.pubkey, pk(AGENT));
    event.validate_crypto().unwrap();
    let served = fx.signer.borrow().served;
    assert_eq!(
        fx.send(CALLER, &request, vec![])["receipt"],
        signed["receipt"]
    );
    assert_eq!(fx.signer.borrow().served, served);
    let mut reused = request.clone();
    reused["input"] = fx.unsigned("other");
    assert_eq!(
        fx.send(CALLER, &reused, vec![])["reason"],
        "idempotency_conflict"
    );
    let mut stranger = fx.request("sign", &input, op.clone(), Value::Null, None);
    stranger["requester"] = json!(pk(STRANGER));
    assert_eq!(
        fx.send(STRANGER, &stranger, vec![])["reason"],
        "not_admitted"
    );
    // An unsigned event for another key refuses before the signer is called.
    let wrong = put(
        &mut fx.artifacts,
        &serde_json::to_vec(
            &json!({"pubkey":pk(STRANGER),"created_at":fx.now,"kind":1,"tags":[],"content":"x"}),
        )
        .unwrap(),
        None,
    );
    let request = fx.request("sign", &wrong, op, Value::Null, None);
    assert_eq!(
        fx.send(CALLER, &request, vec![])["reason"],
        "identity_mismatch"
    );
    assert_eq!(fx.signer.borrow().served, served);
}

#[test]
fn decryption_goes_only_to_its_recipient_and_stops_after_revocation() {
    let mut fx = Fx::new();
    fx.admit_custody();
    let key = nostr::nip44::conversation_key(&secret(PEER), &pk(AGENT).parse().unwrap());
    let ciphertext = nostr::nip44::encrypt("private memory", &key, [9; 32]).unwrap();
    let mine = put(
        &mut fx.artifacts,
        &serde_json::to_vec(&json!({"peer":pk(PEER),"text":ciphertext,"recipient":pk(CALLER)}))
            .unwrap(),
        None,
    );
    let theirs = put(
        &mut fx.artifacts,
        &serde_json::to_vec(&json!({"peer":pk(PEER),"text":ciphertext,"recipient":pk(STRANGER)}))
            .unwrap(),
        None,
    );
    let op = fx.other_op();
    let request = fx.request("nip44_decrypt", &theirs, op.clone(), Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "not_admitted");
    let request = fx.request("nip44_decrypt", &mine, op, Value::Null, None);
    let opened = fx.send(CALLER, &request, vec![]);
    assert_eq!(opened["plaintext"], "private memory");
    // The receipt itself carries no plaintext.
    let receipt = fx.custodian.store().json(&opened["receipt"]).unwrap();
    assert!(receipt["result"].is_null());
    fx.custodian.revoke(&pk(CALLER)).unwrap();
    let retry = fx.send(CALLER, &request, vec![]);
    assert_eq!(retry["status"], "completed");
    assert!(retry["plaintext"].is_null());
}

#[test]
fn a_guardian_quorum_is_exact_and_single_use() {
    let mut fx = Fx::new();
    fx.admit_custody();
    fx.admit_guardians(Value::Null, 0);
    let input = fx.unsigned("pay 10 sats");
    let pay = fx.pay.clone();
    let request = fx.request("sign", &input, pay.clone(), Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "not_admitted");
    let action = fx.action(&input);
    let requests = fx.requests(&action);
    assert_eq!(requests.as_object().unwrap().len(), 3);
    fx.decide(G1, &requests[pk(G1)], &action, "approve");
    // The same guardian twice counts once.
    assert_eq!(
        fx.decide(G1, &requests[pk(G1)], &action, "approve")["status"],
        "duplicate"
    );
    let early = fx.request("sign", &input, pay.clone(), reference(&action), None);
    assert_eq!(fx.send(CALLER, &early, vec![])["reason"], "not_admitted");
    fx.decide(G2, &requests[pk(G2)], &action, "approve");
    let request = fx.request("sign", &input, pay.clone(), reference(&action), None);
    let signed = fx.send(CALLER, &request, vec![]);
    assert_eq!(signed["status"], "completed", "{signed}");
    let receipt = fx.custodian.store().json(&signed["receipt"]).unwrap();
    let admission = fx.custodian.store().json(&receipt["quorum"]).unwrap();
    assert_eq!(admission["approvals"].as_array().unwrap().len(), 2);
    // A retry returns the retained outcome; another request cannot reuse it.
    assert_eq!(
        fx.send(CALLER, &request, vec![])["receipt"],
        signed["receipt"]
    );
    let again = fx.request("sign", &input, pay, reference(&action), None);
    assert_eq!(fx.send(CALLER, &again, vec![])["reason"], "conflict");
}

#[test]
fn denials_take_precedence_and_decisions_do_not_cross_actions() {
    let mut fx = Fx::new();
    fx.admit_custody();
    fx.admit_guardians(Value::Null, 0);
    let pay = fx.pay.clone();
    let input = fx.unsigned("pay 20 sats");
    let action = fx.action(&input);
    let requests = fx.requests(&action);
    fx.decide(G1, &requests[pk(G1)], &action, "approve");
    fx.decide(G2, &requests[pk(G2)], &action, "approve");
    fx.decide(G3, &requests[pk(G3)], &action, "deny");
    let request = fx.request("sign", &input, pay.clone(), reference(&action), None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "not_admitted");
    // A guardian that changes its decision blocks admission.
    let input = fx.unsigned("pay 30 sats");
    let second = fx.action(&input);
    let second_requests = fx.requests(&second);
    fx.decide(G1, &second_requests[pk(G1)], &second, "approve");
    fx.decide(G1, &second_requests[pk(G1)], &second, "deny");
    fx.decide(G2, &second_requests[pk(G2)], &second, "approve");
    fx.decide(G3, &second_requests[pk(G3)], &second, "approve");
    let request = fx.request("sign", &input, pay.clone(), reference(&second), None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "not_admitted");
    // A decision naming the first action's request cannot count for a third.
    let input = fx.unsigned("pay 40 sats");
    let third = fx.action(&input);
    fx.requests(&third);
    assert_eq!(
        fx.decide(G1, &requests[pk(G1)], &third, "approve")["reason"],
        "identity_mismatch"
    );
    // A stranger is not a guardian.
    assert_eq!(
        fx.decide(STRANGER, &requests[pk(G1)], &third, "approve")["reason"],
        "not_admitted"
    );
}

#[test]
fn policy_or_controller_changes_void_pending_quorums() {
    let mut fx = Fx::new();
    fx.admit_custody();
    fx.admit_guardians(Value::Null, 0);
    let pay = fx.pay.clone();
    let input = fx.unsigned("pay 50 sats");
    let action = fx.action(&input);
    let requests = fx.requests(&action);
    fx.decide(G1, &requests[pk(G1)], &action, "approve");
    fx.decide(G2, &requests[pk(G2)], &action, "approve");
    let previous = fx.guardian.clone();
    fx.admit_guardians(previous, 1);
    let request = fx.request("sign", &input, pay.clone(), reference(&action), None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "not_admitted");
    let action = fx.action(&input);
    let requests = fx.requests(&action);
    fx.decide(G1, &requests[pk(G1)], &action, "approve");
    fx.decide(G2, &requests[pk(G2)], &action, "approve");
    fx.custodian.advance_generation(&pk(AGENT)).unwrap();
    let request = fx.request("sign", &input, pay, reference(&action), None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "stale");
}

#[test]
fn signer_outages_refuse_or_stay_unknown_without_a_second_call() {
    let mut fx = Fx::new();
    fx.admit_custody();
    let op = fx.other_op();
    fx.signer.borrow_mut().unreachable = true;
    let input = fx.unsigned("one");
    let request = fx.request("sign", &input, op.clone(), Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "unavailable");
    fx.signer.borrow_mut().unreachable = false;
    let input = fx.unsigned("two");
    let request = fx.request("sign", &input, op.clone(), Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["status"], "completed");
    fx.signer.borrow_mut().lose_reply = true;
    let input = fx.unsigned("three");
    let request = fx.request("sign", &input, op, Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["status"], "unknown");
    let served = fx.signer.borrow().served;
    fx.signer.borrow_mut().lose_reply = false;
    assert_eq!(fx.send(CALLER, &request, vec![])["status"], "unknown");
    assert_eq!(fx.signer.borrow().served, served);
}

struct ReplySubstitution {
    signer: nostr::domain::RemoteSigner,
    foreign_author: bool,
    invalid_ack: bool,
    foreign_on_operation: bool,
    methods: Rc<RefCell<Vec<String>>>,
}
impl SignerTransport for ReplySubstitution {
    fn exchange(&mut self, request: &Event, now: u64) -> Result<Event, Exchange> {
        use nostr::domain::{RelaySigner, RemoteRequest, Tag, open_response};
        let client = request.pubkey.parse().unwrap();
        let clear = nostr::nip44::decrypt(
            &request.content,
            &nostr::nip44::conversation_key(&secret(REMOTE), &client),
        )
        .unwrap();
        let request_body: RemoteRequest = serde_json::from_str(&clear).unwrap();
        self.methods.borrow_mut().push(request_body.method.clone());
        let event = self.signer.handle(request, now, [31; 32]).unwrap();
        let foreign = self.foreign_author
            && (!self.foreign_on_operation || request_body.method == "nip44_encrypt");
        if !foreign && !(self.invalid_ack && request_body.method == "connect") {
            return Ok(event);
        }
        let mut body = open_response(&event, &secret(TRANSPORT)).unwrap();
        if self.invalid_ack && request_body.method == "connect" {
            body.result = Some("not-an-acknowledgment".into());
        }
        let author = secret(if self.foreign_author {
            STRANGER
        } else {
            REMOTE
        });
        let content = nostr::nip44::encrypt(
            &serde_json::to_string(&body).unwrap(),
            &nostr::nip44::conversation_key(&author, &client),
            [32; 32],
        )
        .unwrap();
        Ok(
            RelaySigner::from_secret_hex(&author.display_secret().to_string())
                .unwrap()
                .sign(
                    now,
                    24133,
                    vec![Tag::new(vec!["p".into(), request.pubkey.clone()])],
                    content,
                ),
        )
    }
}

#[test]
fn custody_transport_rejects_foreign_authors_wrong_agents_and_invalid_acknowledgments() {
    for (foreign_author, wrong_agent, invalid_ack, foreign_on_operation) in [
        (true, false, false, false),
        (true, false, false, true),
        (false, true, false, false),
        (false, false, true, false),
    ] {
        let methods = Rc::new(RefCell::new(Vec::new()));
        let transport = ReplySubstitution {
            signer: nostr::domain::RemoteSigner::new(
                secret(REMOTE),
                secret(if wrong_agent { STRANGER } else { AGENT }),
                None,
                vec![],
            ),
            foreign_author,
            invalid_ack,
            foreign_on_operation,
            methods: methods.clone(),
        };
        let mut connection = Nip46::new(
            secret(TRANSPORT),
            &pk(REMOTE),
            &pk(AGENT),
            Box::new(transport),
            None,
        )
        .unwrap();
        let outcome = connection.nip44("nip44_encrypt", &pk(PEER), "private memory", crate::now());
        assert!(
            !matches!(outcome, crate::signer::Call::Result(_)),
            "untrusted connection was accepted"
        );
        assert_eq!(
            methods.borrow().iter().any(|m| m == "nip44_encrypt"),
            foreign_on_operation
        );
    }
}

#[test]
fn retained_plaintext_requires_current_policy_generation_and_request_deadline() {
    for change in [
        "policy",
        "generation",
        "deadline",
        "guardian",
        "policy-expiry",
    ] {
        let mut fx = Fx::new();
        fx.admit_custody();
        let key = nostr::nip44::conversation_key(&secret(PEER), &pk(AGENT).parse().unwrap());
        let ciphertext = nostr::nip44::encrypt("private memory", &key, [35; 32]).unwrap();
        let operand = put(
            &mut fx.artifacts,
            &serde_json::to_vec(&json!({"peer":pk(PEER),"text":ciphertext,"recipient":pk(CALLER)}))
                .unwrap(),
            None,
        );
        let operation = fx.other_op();
        let mut request = fx.request("nip44_decrypt", &operand, operation, Value::Null, None);
        if change == "policy-expiry" {
            request["deadline"] = json!(fx.now + 7200);
        }
        let opened = fx.send(CALLER, &request, vec![]);
        assert_eq!(opened["plaintext"], "private memory");
        let served = fx.signer.borrow().served;
        match change {
            "policy" => {
                let mut policy = fx.custody_policy("nip46");
                policy["revision"] = json!(1);
                policy["previous"] = fx.custody.clone();
                policy["callers"] = json!([pk(STRANGER)]);
                assert_eq!(fx.send(AUTHORITY, &policy, vec![])["status"], "accepted");
            }
            "generation" => {
                fx.custodian.advance_generation(&pk(AGENT)).unwrap();
            }
            "deadline" => {
                fx.now += 301;
            }
            "policy-expiry" => {
                fx.now += 3601;
            }
            "guardian" => {
                fx.admit_guardians(Value::Null, 0);
            }
            _ => unreachable!(),
        }
        let replay = fx.send(CALLER, &request, vec![]);
        assert_eq!(replay["status"], "completed");
        assert!(
            replay["plaintext"].is_null(),
            "plaintext was redisclosed after {change}"
        );
        assert_eq!(fx.signer.borrow().served, served);
    }
}

#[test]
fn custody_policy_cannot_authorize_before_its_start_time() {
    let mut fx = Fx::new();
    let mut policy = fx.custody_policy("nip46");
    policy["starts_at"] = json!(fx.now + 100);
    assert_eq!(fx.send(AUTHORITY, &policy, vec![])["status"], "accepted");
    fx.custody = reference(&policy);
    let input = fx.unsigned("future policy");
    let op = fx.other_op();
    let request = fx.request("sign", &input, op, Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["reason"], "stale");
    assert_eq!(fx.signer.borrow().served, 0);
}

#[test]
fn remount_cannot_replace_an_admitted_signer_transport_or_agent() {
    let mut fx = Fx::new();
    fx.admit_custody();
    for changed in ["signer", "transport", "agent"] {
        let remote = if changed == "signer" {
            STRANGER
        } else {
            REMOTE
        };
        let client = if changed == "transport" {
            STRANGER
        } else {
            TRANSPORT
        };
        let agent = if changed == "agent" { STRANGER } else { AGENT };
        let connection = Nip46::new(
            secret(client),
            &pk(remote),
            &pk(agent),
            Box::new(InProcessSigner::new(secret(remote), secret(agent), None)),
            None,
        )
        .unwrap();
        let error = fx.custodian.mount(&pk(AGENT), connection).unwrap_err();
        assert_eq!(crate::custodian::code(&error), "identity_mismatch");
    }
    let input = fx.unsigned("original connection");
    let op = fx.other_op();
    let request = fx.request("sign", &input, op, Value::Null, None);
    assert_eq!(fx.send(CALLER, &request, vec![])["status"], "completed");
    assert!(fx.signer.borrow().served > 0);
}
