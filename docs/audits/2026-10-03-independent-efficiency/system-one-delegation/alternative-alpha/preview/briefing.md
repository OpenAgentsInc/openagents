## Prepared source context

Source commit: `09f4e0915150f503e332c2b213d24f793c433f3b`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coder-access/src/host/enroll.rs:102-176 — Host::request_enrollment

File SHA-256: `dc07b3a1d4a939660bee8c3b3b8d19539c38f06fee17c57b351cbb1a80f13f98`

```rust
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

### crates/coder-access/src/host/enroll.rs:177-193 — Host::enrollment_status

File SHA-256: `dc07b3a1d4a939660bee8c3b3b8d19539c38f06fee17c57b351cbb1a80f13f98`

```rust
    pub fn enrollment_status(&self, id: &str, now: u64) -> Result<EnrollmentStatus> {
        let (_, _, book) = self.open()?;
        let record = book
            .enrollments
            .get(id)
            .ok_or_else(|| Error::new(Code::Unavailable, "enrollment request is not retained"))?;
        Ok(match &record.state {
            EnrollmentState::Pending {} if now >= record.expires_at => EnrollmentStatus::Expired,
            EnrollmentState::Pending {} => EnrollmentStatus::Pending,
            EnrollmentState::Approved { device, grant, .. } => EnrollmentStatus::Approved {
                device: device.clone(),
                grant: grant.clone(),
            },
            EnrollmentState::Denied { .. } => EnrollmentStatus::Denied,
            EnrollmentState::Closed {} => EnrollmentStatus::Closed,
        })
    }
```

### crates/coder-access/src/host/mod.rs:247-251 — Host::devices

File SHA-256: `7b0f8ed5041f0bc2929a03632e0a5245188416c1dbb2029924b5600ff001bced`

```rust
    /// Every enrolled grant with its current state.
    pub fn devices(&self, now: u64) -> Result<Vec<DeviceEntry>> {
        let (_, _, book) = self.open()?;
        Ok(devices(&book, now))
    }
```

### crates/coder-access/src/host/enroll.rs:96-101 — Host::cancel_invitation

File SHA-256: `dc07b3a1d4a939660bee8c3b3b8d19539c38f06fee17c57b351cbb1a80f13f98`

```rust
    /// Cancel an unused invitation. A redeemed one needs device revocation.
    pub fn cancel_invitation(&self, id: &str) -> Result<()> {
        let (mut store, _, mut book) = self.open()?;
        cancel(&mut book, id)?;
        Ok(store.save(&book)?)
    }
```

### crates/coder-access/src/host/mod.rs:19-19 — MAX_GRANTS

File SHA-256: `7b0f8ed5041f0bc2929a03632e0a5245188416c1dbb2029924b5600ff001bced`

```rust
const MAX_GRANTS: usize = 128;
```

### crates/coder-computers/tests/live.rs:469-591 — add_a_computer_over_ssh_enrolls_through_its_invitation

File SHA-256: `d1deebc71b4a1231129ea19da73fca58718957d4dd95e0c9cdf274b275139ffa`

```rust
#[test]
fn add_a_computer_over_ssh_enrolls_through_its_invitation() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host) = serve_host(&dir.join("devbox-host"), &owner, &relay);

    // The fake remote machine: its home holds the invitation its host
    // prints, and every ssh invocation asks for a password.
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let remote = Remote { home: home.clone() };
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    std::fs::write(home.join("invitation"), format!("{}\n", invitation.code)).unwrap();
    std::fs::write(dir.join("password"), PASSWORD).unwrap();
    // The tunnel route is covered in `tests/edits.rs`; here port forwarding
    // is refused, and the host stays on its other routes.
    std::fs::write(dir.join("no-tunnel"), "").unwrap();
    let archive = fake_ssh::archive(&dir, FAKE_CODER, "coder.tar.gz");
    let program = fake_ssh::shim(&dir, &home, "sh");
    let (os, arch) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => (coder_ssh::Os::Macos, coder_ssh::Arch::Aarch64),
        ("macos", _) => (coder_ssh::Os::Macos, coder_ssh::Arch::X86_64),
        (_, "aarch64") => (coder_ssh::Os::Linux, coder_ssh::Arch::Aarch64),
        _ => (coder_ssh::Os::Linux, coder_ssh::Arch::X86_64),
    };
    let release = coder_ssh::Release::new(vec![coder_ssh::Artifact {
        os,
        arch,
        sha256: fake_ssh::sha256_file(&archive),
        archive,
    }])
    .unwrap();
    let mut setup = SshSetup::coder(release, &pubkey(&owner), &relay, true).unwrap();
    setup.program = Some(program);
    let mut settings = settings();
    settings.ssh = Some(setup);

    let mut computers = open(settings, &runtime);
    // First run offers SSH on a terminal client.
    press(&mut computers, "ssh-connect");
    submit(&mut computers, InputPurpose::SshDestination, "devbox");
    // Each ssh invocation asks; the terminal answers through an input
    // request that it masks.
    let deadline = Instant::now() + WAIT;
    let mut answered = 0;
    loop {
        computers.refresh().unwrap();
        let stage = computers.snapshot().ssh.clone().unwrap().stage;
        match stage {
            SshStage::Added { .. } => break,
            SshStage::Failed { reason } => panic!("SSH setup failed: {reason}"),
            SshStage::Prompt { text, .. } => {
                assert!(text.contains("password"), "{text}");
                let input = computers.input().unwrap().clone();
                assert_eq!(input.purpose, InputPurpose::SshPassword);
                assert!(input.secret);
                assert_eq!(input.prompt, text);
                computers.submit(&input.token, PASSWORD).unwrap();
                answered += 1;
            }
            SshStage::Starting | SshStage::Enrolling => {}
            stage => panic!("a setup never reaches {stage:?}"),
        }
        assert!(
            Instant::now() < deadline,
            "timed out in the SSH setup:\n{}",
            all_text(&computers)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Install asks for the archive, uploads it, installs, then invites.
    assert!(answered >= 3, "answered {answered} prompts");
    // The password never reached an environment, an argument, or a file.
    let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
    assert!(!calls.contains(PASSWORD));

    let snapshot = computers.snapshot().clone();
    let index = row(&snapshot, &host).expect("the SSH host in the list");
    assert_eq!(snapshot.hosts[index].label, "devbox");
    assert_eq!(snapshot.hosts[index].ssh.as_deref(), Some("devbox"));
    assert_eq!(
        text(&computers, "ssh-status").as_deref(),
        Some("Added devbox over SSH.")
    );
    // The SSH host satisfies first run.
    press(&mut computers, "first-run-continue");
    assert_eq!(
        text(&computers, &format!("host-{index}-ssh")).as_deref(),
        Some("Set up over SSH on devbox.")
    );
    until(&mut computers, "the SSH host online", |c| {
        text(c, &format!("host-{index}-status")).is_some_and(|s| s.starts_with("Online"))
    });
    // The host's grant, not the SSH login, now decides access.
    // The serving host can hold the store lock for a moment; retry as
    // `coder host list` does.
    let deadline = Instant::now() + WAIT;
    let device = loop {
        match store.devices(now()) {
            Ok(devices) => break devices,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error:?}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    assert_eq!(device.len(), 1);
    assert_eq!(device[0].rights, Rights::all());

    // Forgetting the computer leaves the managed remote host running: only
    // an explicit remove stops it.
    let pid = remote.pid().expect("the remote host's runtime record");
    press(&mut computers, &format!("host-{index}-forget"));
    press(&mut computers, &format!("host-{index}-forget-yes"));
    assert!(row(computers.snapshot(), &host).is_none());
    assert!(alive(pid));
}
```

### crates/coder-access/src/tests/flows.rs:4-24 — redeem_then_same_device_retry_after_restart_and_other_device_refused

File SHA-256: `47c553ce162ae0362cf481230ecb3e6c7c1a56acf25010773164b1ca3c0b3a5b`

```rust
#[tokio::test]
async fn redeem_then_same_device_retry_after_restart_and_other_device_refused() {
    let mut f = Fixture::served(0, false).await;
    let code = f.invite("standard");
    let phone = key();
    let access = client::redeem(&code, &phone, POLICY).await.unwrap();
    assert_eq!(f.next_handled().await.map(|_| ()), Ok(()));
    assert_eq!(access.grant.rights, Rights::standard());
    assert_eq!(access.grant.owner, pubkey(&f.owner));
    assert_eq!(access.grant.epoch, 0);
    assert!(access.verify(&phone, now(), POLICY).is_ok());

    // Every request opens a new Host over the private store: a restart.
    let again = client::redeem(&code, &phone, POLICY).await.unwrap();
    assert_eq!(again.grant.grant, access.grant.grant);
    assert_eq!(again.grant.expires_at, access.grant.expires_at);

    let other = client::redeem(&code, &key(), POLICY).await.unwrap_err();
    assert_eq!(other.code, Code::Forbidden);
    assert_eq!(f.host().devices(now()).unwrap().len(), 1);
}
```

### crates/coder-access/src/host/mod.rs:102-112 — GrantRecord

File SHA-256: `7b0f8ed5041f0bc2929a03632e0a5245188416c1dbb2029924b5600ff001bced`

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRecord {
    grant: Grant,
    authorization: Event,
    revoked_at: Option<u64>,
    /// Host time of the last authenticated request or direct channel from
    /// the grant's device under this grant. Older stores lack it.
    #[serde(default)]
    seen_at: Option<u64>,
}
```

### crates/coder-access/src/host/mod.rs:564-617 — Host::book

File SHA-256: `7b0f8ed5041f0bc2929a03632e0a5245188416c1dbb2029924b5600ff001bced`

```rust
    fn book(&self, store: &Store, secret: &SecretKey) -> Result<Book> {
        let book: Book = store.load()?.ok_or_else(|| {
            Error::new(Code::Unavailable, "initialize the host with `init` first")
        })?;
        let host = pubkey(secret);
        if book.v != STORE_VERSION
            || book.host != host
            || book.grants.len() > MAX_GRANTS
            || book.invitations.len() > MAX_INVITATIONS
            || book.enrollments.len() > MAX_ENROLLMENTS
            || book.replies.len() > MAX_REPLIES
            || book.epochs.len() > MAX_EPOCHS
        {
            return fail(
                Code::Malformed,
                "host access store identity or bounds differ",
            );
        }
        public(&book.owner)?;
        for (id, record) in &book.grants {
            record.grant.validate(self.policy)?;
            let signed: Grant = open(
                &record.authorization,
                secret,
                &host,
                &record.grant.device,
                GRANT,
            )?;
            if id != &record.grant.grant
                || encoded(&signed)? != encoded(&record.grant)?
                || record.grant.owner != book.owner
                || record.grant.host != host
            {
                return fail(
                    Code::Malformed,
                    "retained grant differs from its signed bytes",
                );
            }
        }
        for (id, retained) in &book.replies {
            if let Some(reply) = &retained.reply
                && (reply.pubkey != host
                    || reply.tag_values("h").collect::<Vec<_>>() != [id.as_str()]
                    || nostr::private_artifact::admit(reply).is_err())
            {
                return fail(
                    Code::Malformed,
                    "retained reply differs from its signed bytes",
                );
            }
        }
        enroll::validate(&book, self.policy)?;
        Ok(book)
    }
```

### crates/coder-access/src/host/enroll.rs:63-68 — IssuedInvitation

File SHA-256: `dc07b3a1d4a939660bee8c3b3b8d19539c38f06fee17c57b351cbb1a80f13f98`

```rust
/// Contains the invitation's capability. Show it only to the enrolling device.
pub struct IssuedInvitation {
    pub id: String,
    pub code: String,
    pub expires_at: u64,
}
```

### crates/coder-computers/src/live.rs:517-521 — Live::device

File SHA-256: `bc748dd9c7586fa970cb544216891fe5b292b3a0d66402810814be1484fba79e`

```rust
    /// This device's public key.
    #[must_use]
    pub fn device(&self) -> &str {
        &self.shared.key
    }
```
