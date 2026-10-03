## Prepared source evidence

Source commit: `09f4e0915150f503e332c2b213d24f793c433f3b`. Selections may omit requirements, dependencies, fixtures, and callers. Partial units are labeled; inspect their full spans before relying on completeness. Complete applicable instructions are supplied separately.

### c03 nips/openagents/NIP-HOST.md:22-48 — nips/openagents/NIP-HOST.md

Role: document; partial_file. Full unit: 1-710. Blob: `0f37d829075fd0eec50bce2156becdfffb274679`. File SHA-256: `3da5fd0161b7096a4aeada96bdccd3fc3444734f205eda3919f85e1890c18210`.

```text
| [CJ](NIP-CJ.md) | The CAP binding below invokes HOST operations through CJ execution v1. A CJ `completed` result means the HOST operation answered; the embedded reply states whether it was admitted. |
| [POL](NIP-POL.md) | Disclosure, action approvals, spending, and publication stay POL decisions. No HOST right approves a POL action, and `operate` does not raise a budget. |
| [REACH](NIP-REACH.md) | Owner host directory, presence, reachability hints, and direct channels. A direct channel binds to one HOST grant ID and epoch. A route, address, or tailnet membership never grants access. The opt-in [tailnet admission](#tailnet-admission) uses Tailscale identity to hand out an invitation, never a grant. |
| [TERM](NIP-TERM.md) | Terminal sessions, streams, and replay. TERM requires the HOST `terminal` right and defines no grant of its own. |
| [RUN](NIP-RUN.md) and [ENV](NIP-ENV.md) | A task admitted through `task.create` is recorded and executed under the host's task owner, RUN, and ENV. HOST admission is not execution evidence. |
| Official [NIP-46](../official/46.md) | A remote signer can hold the owner key. A permission to sign is not a HOST right; the host still checks the signer against its locally established owner. |
| Block [NIP-OA](../block/NIP-OA.md) and [NIP-AA](../block/NIP-AA.md) | Relay admission for agent keys. Relay membership never admits a HOST operation. |

## Encoding, principals, and limits

Every artifact defined below contains `v`, `requires: []`, and optional inert
`meta` only where specified; this version specifies none. Reject unknown
fields, enum values, versions, duplicate keys, and nonempty `requires`.
Common IDs are random 64-hex values. Pubkeys are 64-hex x-only keys.
Timestamps are Unix seconds. Host time, not event `created_at`, enforces
expiry. Bodies are at most 128 KiB and use the common nesting limit.

There are three principals:

- The **owner** is the key whose authority the host serves. The host
  establishes the owner relationship locally, for example with an operator
  command on that machine. An invitation, approval, relay event, or display
  name cannot establish or change it. The owner holds every right at its host
  without a grant. The host refuses a request signed by any other key without
  a grant.
- The **host** key signs grants, replies, and enrollment requests. It is
  distinct from the owner key.
```

### c06 crates/coder-access/README.md:1-55 — crates/coder-access/README.md

Role: document; partial_file. Full unit: 1-187. Blob: `365d3f3cc4754796439c66632fe2a9e5a637af3e`. File SHA-256: `0b2d7ddf3063bbc5136326a62f210c3275ba8eb66b406d4885935d7c73af47ac`.

````text
# Coder Access

Coder Access admits devices to one Coder host with host-wide, scoped rights.
It implements the [NIP-HOST draft](../../nips/openagents/NIP-HOST.md): host
invitations, reverse enrollment for a headless host, host-signed grants with
revocation epochs, delegation, device listing and revocation, and a typed
`task.create` operation. Requests and replies are original signed private
`3188` artifacts over NIP-42 authenticated relay connections. The resident
host in [`coder-host`](../coder-host/README.md) also carries them over direct
channels and inside NIP-CJ execution requests.

The host is the only issuer of access. An invitation, an approval, or relay
delivery introduces a device. Only the host's current grant record admits an
operation, and every operation checks it again.

## Rights

| Right | Permits |
| --- | --- |
| `observe` | Session, task, and file reads under the disclosure policy. |
| `operate` | Create, steer, and cancel tasks and sessions. |
| `terminal` | Open and drive terminals. |
| `review` | Write reviews and diffs. |
| `access_read` | List enrolled devices. |
| `access_admin` | Invite, approve, deny, cancel, and revoke, within held rights. |

No right implies another. The `standard` preset is `observe`, `operate`,
`terminal`, and `review`. The `admin` preset is `access_read` and
`access_admin`. A refusal for a missing right names that right.

## Set up a host

Establish the owner on the host itself. Nothing received over a relay can do
this.

```sh
cargo run -p coder-access -- init --owner <owner-public-key>
```

The owner key accepts lower-case hex or an `npub`. The placeholder is not a
real key. The command creates a host key and a private store under
`~/.openagents/coder-access/`, or the directory passed with `--state` before
the command. The directory has mode `0700`; the key, lock, and state files
have mode `0600`. Initializing again with the same owner does nothing;
another owner is refused. To change the owner, create a new store.

## Enroll a device with an invitation

```sh
cargo run -p coder-access -- invite --relay wss://relay.example/ --rights standard
```

The host saves the invitation, then prints its ID and a `coder-host:` paste
string, and draws a QR code in the terminal. `--no-qr` skips the QR code.
The invitation admits one device and expires after five minutes. Show it
````

### c01 crates/coder-access/src/host/enroll.rs:102-176 — Host::request_enrollment

Role: implementation; complete_declaration. Full unit: 102-176. Blob: `6bb26ba6f5647afaf4565595755ab4f79cdfdf31`. File SHA-256: `dc07b3a1d4a939660bee8c3b3b8d19539c38f06fee17c57b351cbb1a80f13f98`.

```text
    /// Start reverse enrollment. The request is persisted before the code is
    /// returned for display. Publish the returned events to the relay.
    pub fn request_enrollment(
        &self,
        relay: &str,
        rights: Rights,
        now: u64,
    ) -> Result<PendingEnrollment> {
        self.policy.validate(relay).map_err(Error::from)?;
        let (mut store, secret, mut book) = self.open()?;
        book.prune(now);
        if book.enrollments.len() >= MAX_ENROLLMENTS {
            return fail(Code::Bounds, "enrollment request retention limit reached");
        }
        let enrollment = Enrollment {
            v: ENROLLMENT.into(),
            requires: vec![],
            enrollment: random_id(),
            host: book.host.clone(),
            owner: book.owner.clone(),
            relay: relay.into(),
            rights: rights.clone(),
            issued_at: now,
            expires_at: now + ENROLLMENT_LIFETIME,
        };
        enrollment.validate(self.policy)?;
        let code = short_code();
        let mut recipients = vec![book.owner.clone()];
        for record in book.grants.values() {
            let g = &record.grant;
            if record.revoked_at.is_none()
                && g.expires_at > now
                && g.epoch == book.epoch(&g.device)
                && g.rights.contains(Right::AccessAdmin)
                && !recipients.contains(&g.device)
                && recipients.len() < MAX_ENROLLMENT_RECIPIENTS
            {
                recipients.push(g.device.clone());
            }
        }
        let events = recipients
            .iter()
            .map(|recipient| {
                seal(
                    &enrollment,
                    ENROLLMENT,
                    &secret,
                    recipient,
                    &enrollment.enrollment,
                    now,
                    enrollment.expires_at,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        book.enrollments.insert(
            enrollment.enrollment.clone(),
            EnrollmentRecord {
                artifact_digest: enrollment.digest()?,
                code_digest: code_digest(&enrollment.enrollment, &code)?,
                relay: relay.into(),
                rights,
                issued_at: now,
                expires_at: enrollment.expires_at,
                attempts: 0,
                state: EnrollmentState::Pending {},
            },
        );
        store.save(&book)?;
        Ok(PendingEnrollment {
            id: enrollment.enrollment,
            code,
            expires_at: enrollment.expires_at,
            events,
        })
    }
```

### c04 crates/coder-access/src/host/mod.rs:425-472 — Host::execute

Role: implementation; partial_declaration. Full unit: 401-533. Blob: `a341758ce82a21cc1b45aebf8d233288c9a1e6a5`. File SHA-256: `7b0f8ed5041f0bc2929a03632e0a5245188416c1dbb2029924b5600ff001bced`.

```text
            }),
            Operation::Revoke { device } => {
                revoke(book, device, now).map(|(epoch, grants)| Outcome::Revoked {
                    device: device.clone(),
                    epoch,
                    grants,
                })
            }
            Operation::ListWorkspaces {} => match dispatch.workspaces() {
                Ok(mut workspaces) => {
                    workspaces.sort();
                    workspaces.dedup();
                    let outcome = Outcome::Workspaces { workspaces };
                    outcome.validate().map(|()| outcome)
                }
                Err(code) => Err(Error::new(code, "the host lists no workspaces")),
            },
            // Queue edits are idempotent: an exact retry after an uncertain
            // save sets the same text, order, or lease again.
            Operation::QueueTask { task, edit } => {
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.queue(&p.key, grant, task, edit) {
                    Ok(queue) => {
                        let outcome = Outcome::Queue { queue };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(
                                Code::Unavailable,
                                "the task owner's queue is invalid",
                            )),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the task owner refused the queue edit")),
                }
            }
            Operation::ListSpends { grant } => {
                if grant.issuer != p.key || grant.grantee != book.host {
                    Err(Error::new(
                        Code::Forbidden,
                        "a spend grant must be the sender's own and name this host",
                    ))
                } else {
                    match dispatch.spends().map(|s| s.list(&p.key, grant, now)) {
                        Some(Ok(spends)) => {
                            let outcome = Outcome::Spends { spends };
                            outcome.validate().map(|()| outcome)
                        }
                        Some(Err(code)) => Err(Error::new(code, "the host refused the spend list")),
```

### c02 crates/coder-access/src/tests/flows.rs:574-610 — a_task_command_reaches_the_owner_with_its_grant_and_epoch

Role: test; complete_declaration. Full unit: 574-610. Blob: `80f26ad3064f83c24aec3241bbd87d07ef4c79d0`. File SHA-256: `47c553ce162ae0362cf481230ecb3e6c7c1a56acf25010773164b1ca3c0b3a5b`.

```text
#[tokio::test]
async fn a_task_command_reaches_the_owner_with_its_grant_and_epoch() {
    let f = Fixture::served(0, false).await;
    let (phone, operator) = f.enroll("standard").await;
    let Outcome::Dispatched { receipt } = operator.call(command()).await.unwrap() else {
        panic!("dispatch expected")
    };
    assert_eq!(receipt.operation, "task.command");
    let entry = f.host().devices(now()).unwrap().remove(0);
    let seen = f.recorder.seen();
    assert_eq!(
        seen[0].1,
        format!("{} {} {}", pubkey(&phone), entry.grant, entry.epoch)
    );
    // Emulation belongs to a steer only, and text stays bounded.
    let Operation::CommandTask { mut command } = command() else {
        unreachable!()
    };
    command.emulate = true;
    assert!(
        Operation::CommandTask {
            command: command.clone()
        }
        .validate()
        .is_err()
    );
    command.action = crate::CommandAction::Steer;
    assert!(
        Operation::CommandTask {
            command: command.clone()
        }
        .validate()
        .is_ok()
    );
    command.text = "x".repeat(16 * 1024 + 1);
    assert!(Operation::CommandTask { command }.validate().is_err());
}
```

### c07 crates/coder-access/src/spend.rs:130-165 — Grant

Role: implementation; complete_declaration. Full unit: 130-165. Blob: `3573cd45e1536169a06430172666095426a9044b`. File SHA-256: `974b7acd78b7183850972b6445b52207f6050e4e936211b646fe6c7b739e3b38`.

```text
/// `openagents.spend-grant.v1`: what a host may ask the phone to pay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub v: String,
    pub requires: Vec<String>,
    /// 32-byte ID, lowercase hex.
    pub grant: String,
    /// The phone's device key.
    pub issuer: String,
    /// The host's key: the agent side that asks.
    pub grantee: String,
    pub wallet: WalletRef,
    pub mode: Mode,
    pub unit: String,
    /// Ceiling for one payment, fees included.
    pub per_payment_max: u64,
    /// Rolling window in seconds and its ceiling, fees included.
    pub period: u64,
    pub period_max: u64,
    /// Lifetime ceiling, fees included.
    pub total_max: u64,
    pub fee_max: FeeMax,
    pub rails: Vec<Rail>,
    /// Allowed payees: Lightning node keys (compressed, hex). Empty means
    /// none, unless `any_payee` is set.
    pub payees: Vec<String>,
    /// Any payee may be asked for. Only in `request` mode, where the owner
    /// sees the payee decoded from the invoice and approves each payment.
    pub any_payee: bool,
    pub purposes: Vec<Purpose>,
    /// The grantee's epoch at the phone. Revocation advances it.
    pub epoch: u64,
    pub issued_at: u64,
    pub expires_at: u64,
}
```

### c05 crates/coder-access/tests/capability.rs:33-63 — checked_in_manifest_is_the_one_a_host_builds_and_validates

Role: test; complete_declaration. Full unit: 33-63. Blob: `fa6b5ff16578ccf8d9c09b3ea254d0f848854471`. File SHA-256: `dddd02369820cac2313da05782d41cd9ef55516847fae50d6112e2cf632105e5`.

```text
#[test]
fn checked_in_manifest_is_the_one_a_host_builds_and_validates() {
    let (_, host) = fixture_host();
    let fixture = checked_in();
    let built = definition(&host, &[RELAY.to_owned()]);
    assert_eq!(fixture["definition"], built, "regenerate the fixture");

    let parsed = cap::parse_definition(&fixture["definition"]).expect("NIP-CAP validator");
    assert_eq!(parsed.profile, Profile::Adapter);
    assert_eq!(parsed.transport, "nostr-cj");
    assert_eq!(parsed.component, SLUG);
    assert_eq!(parsed.idempotency, "request_attempt");
    assert_eq!(
        fixture["definition"]["binding_contract"]["operations"],
        json!(OPERATIONS)
    );

    // The schema references pin the checked-in schema documents, and the
    // shared evaluator accepts both.
    for (field, bytes) in [("input", CALL_SCHEMA), ("output", ANSWER_SCHEMA)] {
        let reference = &fixture["definition"][field];
        assert_eq!(reference["digest"], digest_bytes(bytes));
        assert_eq!(reference["size"], bytes.len());
        assert_eq!(reference["media_type"], "application/schema+json");
    }
    let documents = BTreeMap::from([
        (digest_bytes(CALL_SCHEMA), CALL_SCHEMA.to_vec()),
        (digest_bytes(ANSWER_SCHEMA), ANSWER_SCHEMA.to_vec()),
    ]);
    prepare_closure(&documents).expect("the schemas use supported keywords");
}
```

### c09 crates/coder-access/src/protocol.rs:27-28 — HostInvitation

Role: implementation; complete_declaration. Full unit: 27-28. Blob: `ab66f2bd00b62340b9d7292fba36ff300f4ddcd2`. File SHA-256: `bd52ee438da9f0fb08e86eeb228cb83ad8788b73dea9f2e555b4b46cb7dda041`.

```text
/// A parsed host invitation. It holds a temporary capability: never log it.
pub struct HostInvitation(pub(crate) Invitation);
```
