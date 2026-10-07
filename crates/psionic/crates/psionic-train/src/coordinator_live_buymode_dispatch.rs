//! Live `BuyModeDispatch` over the real Pylon dispatch path (Khala M6, issue
//! #6014 / EPIC #6017).
//!
//! [`crate::coordinator_eval_verdict_source`] (#1137) defines the
//! [`BuyModeDispatch`] seam and a [`DispatchBackedVerdictSource`] that pre-checks
//! the [`DailySpendCap`] and yields the scalar terminal verdict the
//! [`crate::LiveCoordinatorFitness`] / [`crate::ShadowComparison`] consume. That
//! change shipped only a fixture dispatcher (`RecordingDispatch`); the "live
//! dispatcher that publishes to the real Pylon pool and reads the settled gateway
//! verdict" was named as the remaining **owner-gated** piece.
//!
//! This module lands that live dispatcher: [`LiveBuyModeDispatch`], a real
//! [`BuyModeDispatch`] that mirrors the transport-mode structure of the existing
//! Pylon training dispatcher
//! ([`crate::qwen_legal_pylon_dispatch`] — `Loopback` / `Tailnet` / `Production`):
//!
//! 1. it signs the priced [`BuyModeEvalJob`] into a [`SignedBuyModeEvalEnvelope`]
//!    with a deterministic scheduler key (the same ed25519 envelope pattern as
//!    the training dispatcher);
//! 2. it publishes that envelope to a coordinator-routed Pylon worker over TCP —
//!    in-process loopback (`127.0.0.1:0`) for the fixture lane, or a remote
//!    `tcp://` / `tailnet://` worker for the tailnet/production lanes;
//! 3. the worker runs the `training.verification_classes.v1` replay-validator
//!    check (NOT a prompted LLM judge) and returns a signed
//!    [`BuyModeEvalVerdictReceipt`];
//! 4. the dispatcher verifies the worker signature and yields a
//!    [`BuyModeEvalResult`] (verdict + settled spend) the
//!    [`DispatchBackedVerdictSource`] feeds into the coordinator lane.
//!
//! ## Default-off, fail-closed, inert until armed
//!
//! - **Disarmed by default.** [`LiveBuyModeDispatch::loopback`] /
//!   [`LiveBuyModeDispatch::remote`] both start [`BuyModeArmState::Disarmed`].
//!   A disarmed dispatcher refuses every job before any socket is opened —
//!   zero network, zero spend.
//! - **Production transport is unreachable without arming.** Even when a
//!   dispatcher is constructed in [`BuyModeDispatchMode::Production`], the
//!   `Production` and `Tailnet` lanes refuse to dial unless the dispatcher is
//!   armed. The owner arming step is the ONLY thing that makes the production
//!   transport reachable.
//! - **The `DailySpendCap` is still the spend authority.** This dispatcher does
//!   NOT debit the cap; the [`DispatchBackedVerdictSource`] runs its fail-closed
//!   cap pre-check *before* it ever calls [`BuyModeDispatch::dispatch_eval`], and
//!   the [`crate::CapDebitOwner`] split decides who debits the settled spend, so
//!   the spend is counted exactly once and never double-counted. The live
//!   dispatcher is reached only after the cap admitted the price.
//!
//! ## Loopback vs production
//!
//! - **Loopback** binds an ephemeral `127.0.0.1:0` listener, spawns an
//!   in-process worker thread that runs the injected replay-validator closure,
//!   and dials it over TCP. No external network, no real inference, no sats. This
//!   is the lane the tests exercise and the offline proof of the full
//!   sign → publish → verdict → verify round trip.
//! - **Tailnet / Production** dial a remote signed-envelope worker over TCP. This
//!   module ships the transport but it is **inert in tests** (never dialed) and
//!   **owner-gated** on the live lane: it needs the M4 real Pylon pool (#6012,
//!   merged), an armed dispatcher, an armed [`DispatchBackedVerdictSource`], and
//!   a spend-enabled buy-mode campaign row. This module provides the seam; it
//!   never fabricates a verdict and never dispatches in CI.

use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::coordinator_eval_verdict_source::{
    BuyModeDispatch, BuyModeEvalJob, BuyModeEvalResult, VerificationClass, VerificationClassVerdict,
};
use crate::coordinator_live_training::CoordinatorLiveTrainingError;

/// Schema version of the scheduler-signed buy-mode eval envelope.
pub const BUY_MODE_EVAL_ENVELOPE_SCHEMA_VERSION: &str =
    "psionic.coordinator_buy_mode_eval_envelope.v1";
/// Schema version of the worker-signed buy-mode eval verdict receipt.
pub const BUY_MODE_EVAL_VERDICT_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.coordinator_buy_mode_eval_verdict_receipt.v1";

// ---------------------------------------------------------------------------
// Arming + transport mode.
// ---------------------------------------------------------------------------

/// Whether the live dispatcher may open a socket and publish a job. **Default is
/// [`Disarmed`].** A disarmed dispatcher refuses every job before any network or
/// spend; arming is an owner decision, never a default.
///
/// This is a second, independent fail-closed gate below the
/// [`crate::DispatchBackedVerdictSource`]'s own
/// [`crate::CoordinatorArmState`]: even an armed verdict source cannot reach the
/// production transport through a disarmed dispatcher.
///
/// [`Disarmed`]: BuyModeArmState::Disarmed
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyModeArmState {
    /// The dispatcher is OFF (default). Every job refuses cleanly before any
    /// socket is opened.
    #[default]
    Disarmed,
    /// The dispatcher is armed: jobs publish over the configured transport.
    Armed,
}

impl BuyModeArmState {
    /// Whether the dispatcher is armed for publishing.
    #[must_use]
    pub const fn is_armed(self) -> bool {
        matches!(self, Self::Armed)
    }
}

/// The transport a [`LiveBuyModeDispatch`] publishes over, mirroring the
/// [`crate::qwen_legal_pylon_dispatch`] training-dispatch modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyModeDispatchMode {
    /// In-process worker over an ephemeral `127.0.0.1:0` TCP listener. No
    /// external network, no real inference, no sats. The fixture / proof lane.
    Loopback,
    /// A remote signed-envelope worker reached over the Tailnet (`tailnet://` or
    /// `tcp://`). Owner-gated; inert in tests (never dialed) and refused unless
    /// the dispatcher is armed.
    Tailnet,
    /// A remote signed-envelope worker on the live Pylon pool (`tcp://`).
    /// Owner-gated; **unreachable without arming**.
    Production,
}

impl BuyModeDispatchMode {
    /// Whether this mode dials an external (non-loopback) worker. External modes
    /// are unreachable unless the dispatcher is armed.
    #[must_use]
    pub const fn is_external(self) -> bool {
        matches!(self, Self::Tailnet | Self::Production)
    }

    /// The transport label recorded for a published job.
    #[must_use]
    pub const fn transport_label(self) -> &'static str {
        match self {
            Self::Loopback => "loopback_tcp",
            Self::Tailnet => "tailnet_tcp_signed_envelope",
            Self::Production => "production_tcp_signed_envelope",
        }
    }
}

// ---------------------------------------------------------------------------
// Refusal: why the live dispatcher refused without publishing.
// ---------------------------------------------------------------------------

/// Why a [`LiveBuyModeDispatch`] refused a job without opening a socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyModeDispatchRefusal {
    /// The dispatcher was disarmed (default). No network, no spend.
    Disarmed,
    /// The transport was an external (tailnet/production) lane on a dispatcher
    /// that was not armed. The production transport is unreachable without
    /// arming.
    ExternalTransportNotArmed,
}

// ---------------------------------------------------------------------------
// Signed envelope + verdict receipt (ed25519, mirrors the training dispatcher).
// ---------------------------------------------------------------------------

/// One scheduler-signed buy-mode eval job published to a Pylon worker. The
/// worker verifies the scheduler signature before running the replay-validator
/// check, mirroring [`crate::qwen_legal_pylon_dispatch`]'s signed job envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedBuyModeEvalEnvelope {
    /// Schema version.
    pub schema_version: String,
    /// Stable envelope id.
    pub envelope_id: String,
    /// The transport mode the scheduler dispatched under.
    pub dispatch_mode: BuyModeDispatchMode,
    /// The priced eval job (worker, role, sample, amount).
    pub job: BuyModeEvalJob,
    /// Digest of the job, bound into the signed payload.
    pub job_digest: String,
    /// Digest the scheduler signed.
    pub signed_payload_digest: String,
    /// Scheduler public key (hex).
    pub scheduler_pubkey_hex: String,
    /// Scheduler signature over `signed_payload_digest` (hex).
    pub signature_hex: String,
    /// Digest of the whole envelope.
    pub envelope_digest: String,
}

impl SignedBuyModeEvalEnvelope {
    /// Digest of the envelope with `envelope_digest` cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.envelope_digest.clear();
        stable_json_digest(b"psionic_coordinator_buy_mode_eval_envelope|", &clone)
    }

    /// Digest of the signable payload (signature fields cleared).
    #[must_use]
    pub fn signable_payload_digest(&self) -> String {
        let mut clone = self.clone();
        clone.signed_payload_digest.clear();
        clone.scheduler_pubkey_hex.clear();
        clone.signature_hex.clear();
        clone.envelope_digest.clear();
        stable_json_digest(
            b"psionic_coordinator_buy_mode_eval_envelope_payload|",
            &clone,
        )
    }
}

/// One worker-signed buy-mode eval verdict: the `training.verification_classes.v1`
/// outcome and the spend that actually settled. The dispatcher verifies the
/// worker signature before trusting it, mirroring the training worker receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyModeEvalVerdictReceipt {
    /// Schema version.
    pub schema_version: String,
    /// The envelope id this verdict answers.
    pub envelope_id: String,
    /// The sample the verdict is for.
    pub sample_id: String,
    /// The named verification class the work was checked under.
    pub verification_class: VerificationClass,
    /// Whether the replay-validator / verification-command check passed.
    pub passed: bool,
    /// The spend that actually settled, in msats.
    pub settled_msats: u64,
    /// Digest the worker signed.
    pub signed_payload_digest: String,
    /// Worker public key (hex).
    pub worker_pubkey_hex: String,
    /// Worker signature over `signed_payload_digest` (hex).
    pub signature_hex: String,
    /// Digest of the whole receipt.
    pub receipt_digest: String,
}

impl BuyModeEvalVerdictReceipt {
    /// Digest of the receipt with `receipt_digest` cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(
            b"psionic_coordinator_buy_mode_eval_verdict_receipt|",
            &clone,
        )
    }

    /// Digest of the signable payload (signature fields cleared).
    #[must_use]
    pub fn signable_payload_digest(&self) -> String {
        let mut clone = self.clone();
        clone.signed_payload_digest.clear();
        clone.worker_pubkey_hex.clear();
        clone.signature_hex.clear();
        clone.receipt_digest.clear();
        stable_json_digest(
            b"psionic_coordinator_buy_mode_eval_verdict_receipt_payload|",
            &clone,
        )
    }

    /// The verification-class verdict the coordinator lane consumes.
    #[must_use]
    pub const fn class_verdict(&self) -> VerificationClassVerdict {
        VerificationClassVerdict {
            class: self.verification_class,
            passed: self.passed,
        }
    }
}

// ---------------------------------------------------------------------------
// Worker-side replay-validator hook.
// ---------------------------------------------------------------------------

/// The worker-side replay-validator that decides the
/// `training.verification_classes.v1` verdict for a published eval job. The live
/// lane binds the real replay validator over the worker's recomputed output; the
/// loopback proof binds a deterministic closure. It MUST be the
/// replay-validator / verification-command outcome, never a prompted LLM judge.
pub trait BuyModeReplayValidator {
    /// Recomputes the verdict + settled spend for `job`. The returned spend is
    /// the real settled amount (which may differ from the quoted price).
    fn validate(
        &self,
        job: &BuyModeEvalJob,
    ) -> Result<(VerificationClassVerdict, u64), CoordinatorLiveTrainingError>;
}

/// A deterministic replay validator for the loopback proof lane: returns a fixed
/// verdict and settles a fixed amount, with no real inference and no external
/// network. This stands in for the worker's real recompute in the round-trip
/// proof; it never moves sats (the loopback worker only records).
#[derive(Clone, Copy, Debug)]
pub struct FixtureReplayValidator {
    verdict: VerificationClassVerdict,
    settled_msats: u64,
}

impl FixtureReplayValidator {
    /// A validator that always returns `verdict` and settles `settled_msats`.
    #[must_use]
    pub const fn new(verdict: VerificationClassVerdict, settled_msats: u64) -> Self {
        Self {
            verdict,
            settled_msats,
        }
    }

    /// A validator that always passes the exact-trace class and settles
    /// `settled_msats`.
    #[must_use]
    pub const fn exact_trace_pass(settled_msats: u64) -> Self {
        Self::new(VerificationClassVerdict::exact_trace_pass(), settled_msats)
    }
}

impl BuyModeReplayValidator for FixtureReplayValidator {
    fn validate(
        &self,
        _job: &BuyModeEvalJob,
    ) -> Result<(VerificationClassVerdict, u64), CoordinatorLiveTrainingError> {
        Ok((self.verdict, self.settled_msats))
    }
}

// ---------------------------------------------------------------------------
// Live dispatcher.
// ---------------------------------------------------------------------------

/// A live [`BuyModeDispatch`] over the Pylon dispatch path. Mirrors the
/// transport-mode structure of [`crate::qwen_legal_pylon_dispatch`]:
/// `Loopback` runs an in-process signed-envelope worker; `Tailnet` / `Production`
/// dial a remote signed-envelope worker.
///
/// **Default-off, fail-closed:** disarmed dispatchers refuse before any socket;
/// external (tailnet/production) transports are unreachable unless armed. The
/// dispatcher never touches the [`crate::DailySpendCap`] — the
/// [`crate::DispatchBackedVerdictSource`] runs the fail-closed cap pre-check
/// before it ever calls [`BuyModeDispatch::dispatch_eval`].
pub struct LiveBuyModeDispatch<V: BuyModeReplayValidator> {
    mode: BuyModeDispatchMode,
    arm: BuyModeArmState,
    /// The remote worker address for `Tailnet` / `Production` (`tcp://host:port`
    /// or `tailnet://host:port`). Unused for `Loopback`.
    remote_addr: Option<String>,
    /// Deterministic scheduler key derived from `run_id`, used to sign envelopes.
    run_id: String,
    /// The worker-side replay validator (loopback worker only). For the
    /// remote lanes the worker runs its own validator; this is bound only on the
    /// in-process loopback lane.
    validator: V,
    /// Records the last refusal reason for the driver's report.
    last_refusal: std::cell::RefCell<Option<BuyModeDispatchRefusal>>,
}

impl<V: BuyModeReplayValidator> LiveBuyModeDispatch<V> {
    /// Builds a **disarmed** loopback dispatcher (the safe default). The
    /// `validator` runs in the in-process worker thread; no external network.
    #[must_use]
    pub fn loopback(run_id: impl Into<String>, validator: V) -> Self {
        Self {
            mode: BuyModeDispatchMode::Loopback,
            arm: BuyModeArmState::Disarmed,
            remote_addr: None,
            run_id: run_id.into(),
            validator,
            last_refusal: std::cell::RefCell::new(None),
        }
    }

    /// Builds a **disarmed** remote dispatcher for `mode` (`Tailnet` /
    /// `Production`) dialing `remote_addr`. The remote lane is unreachable until
    /// [`arm`](Self::arm) is called — this is the owner-gated production
    /// transport. `validator` is unused on the remote lane (the remote worker
    /// runs its own) but kept for type uniformity.
    #[must_use]
    pub fn remote(
        mode: BuyModeDispatchMode,
        remote_addr: impl Into<String>,
        run_id: impl Into<String>,
        validator: V,
    ) -> Self {
        Self {
            mode,
            arm: BuyModeArmState::Disarmed,
            remote_addr: Some(remote_addr.into()),
            run_id: run_id.into(),
            validator,
            last_refusal: std::cell::RefCell::new(None),
        }
    }

    /// Arms the dispatcher (owner decision). Consumes and returns `self` so an
    /// armed dispatcher is an explicit, deliberate construction step.
    #[must_use]
    pub fn arm(mut self) -> Self {
        self.arm = BuyModeArmState::Armed;
        self
    }

    /// Whether the dispatcher is armed.
    #[must_use]
    pub const fn is_armed(&self) -> bool {
        self.arm.is_armed()
    }

    /// The transport mode.
    #[must_use]
    pub const fn mode(&self) -> BuyModeDispatchMode {
        self.mode
    }

    /// The last refusal reason, if the last job refused without publishing.
    #[must_use]
    pub fn last_refusal(&self) -> Option<BuyModeDispatchRefusal> {
        *self.last_refusal.borrow()
    }

    fn refuse(
        &self,
        refusal: BuyModeDispatchRefusal,
        detail: String,
    ) -> CoordinatorLiveTrainingError {
        *self.last_refusal.borrow_mut() = Some(refusal);
        CoordinatorLiveTrainingError::VerdictSource { detail }
    }

    /// Signs the priced job into a scheduler-signed envelope.
    fn signed_envelope(&self, job: &BuyModeEvalJob) -> SignedBuyModeEvalEnvelope {
        let signing_key = scheduler_signing_key(&self.run_id);
        let mut envelope = SignedBuyModeEvalEnvelope {
            schema_version: String::from(BUY_MODE_EVAL_ENVELOPE_SCHEMA_VERSION),
            envelope_id: format!("buymode.{}.{}", self.run_id, job.sample_id),
            dispatch_mode: self.mode,
            job_digest: job_digest(job),
            job: job.clone(),
            signed_payload_digest: String::new(),
            scheduler_pubkey_hex: String::new(),
            signature_hex: String::new(),
            envelope_digest: String::new(),
        };
        envelope.signed_payload_digest = envelope.signable_payload_digest();
        envelope.scheduler_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
        envelope.signature_hex = hex::encode(
            signing_key
                .sign(envelope.signed_payload_digest.as_bytes())
                .to_bytes(),
        );
        envelope.envelope_digest = envelope.stable_digest();
        envelope
    }
}

impl<V: BuyModeReplayValidator> BuyModeDispatch for LiveBuyModeDispatch<V> {
    fn dispatch_eval(
        &self,
        job: &BuyModeEvalJob,
    ) -> Result<BuyModeEvalResult, CoordinatorLiveTrainingError> {
        // Default-off arming gate. Disarmed -> refuse before any socket is
        // opened. This is what makes the production transport unreachable without
        // arming: an external (tailnet/production) lane can only be dialed below,
        // and that path is reached only past this gate. The refusal reason
        // distinguishes an external transport (a deliberate production-lane
        // attempt) from a loopback attempt, so the driver's report is honest
        // about WHAT was blocked.
        if !self.arm.is_armed() {
            let refusal = if self.mode.is_external() {
                BuyModeDispatchRefusal::ExternalTransportNotArmed
            } else {
                BuyModeDispatchRefusal::Disarmed
            };
            return Err(self.refuse(
                refusal,
                format!(
                    "live buy-mode {} transport is DISARMED (default) and unreachable without arming; refusing job for sample `{}` with no network and zero spend",
                    self.mode.transport_label(),
                    job.sample_id
                ),
            ));
        }

        *self.last_refusal.borrow_mut() = None;
        let envelope = self.signed_envelope(job);

        let receipt = match self.mode {
            BuyModeDispatchMode::Loopback => {
                run_loopback_buy_mode_worker(&envelope, job, &self.validator)?
            }
            BuyModeDispatchMode::Tailnet | BuyModeDispatchMode::Production => {
                let addr = self.remote_addr.as_deref().ok_or_else(|| {
                    CoordinatorLiveTrainingError::VerdictSource {
                        detail: String::from(
                            "live buy-mode remote dispatch is missing a worker address",
                        ),
                    }
                })?;
                run_remote_buy_mode_worker(addr, &envelope)?
            }
        };

        verify_worker_verdict_receipt(&receipt, &envelope)?;
        Ok(BuyModeEvalResult {
            verdict: receipt.class_verdict(),
            settled_msats: receipt.settled_msats,
        })
    }
}

// ---------------------------------------------------------------------------
// Loopback transport: in-process signed-envelope worker over 127.0.0.1:0.
// ---------------------------------------------------------------------------

fn run_loopback_buy_mode_worker<V: BuyModeReplayValidator>(
    envelope: &SignedBuyModeEvalEnvelope,
    job: &BuyModeEvalJob,
    validator: &V,
) -> Result<BuyModeEvalVerdictReceipt, CoordinatorLiveTrainingError> {
    // Run the validator on the scheduler side of the loopback boundary so the
    // worker thread can stay `Send` without bounding `V: Send`. The loopback
    // worker proves the signed sign -> publish -> verdict -> verify round trip;
    // the verdict content is the deterministic validator's.
    let (verdict, settled_msats) = validator.validate(job)?;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(io_err("loopback tcp bind"))?;
    let endpoint = listener
        .local_addr()
        .map_err(io_err("loopback tcp local_addr"))?
        .to_string();

    let worker = thread::spawn(move || -> Result<Vec<u8>, CoordinatorLiveTrainingError> {
        let (mut inbound, _) = listener.accept().map_err(io_err("loopback tcp accept"))?;
        let mut request_bytes = Vec::new();
        inbound
            .read_to_end(&mut request_bytes)
            .map_err(io_err("loopback tcp read"))?;
        let envelope = serde_json::from_slice::<SignedBuyModeEvalEnvelope>(&request_bytes)
            .map_err(serde_err)?;
        // The worker verifies the scheduler signature before answering.
        verify_signed_envelope(&envelope)?;
        let receipt = signed_verdict_receipt(&envelope, verdict, settled_msats);
        serde_json::to_vec(&receipt).map_err(serde_err)
    });

    let mut stream =
        TcpStream::connect(endpoint.as_str()).map_err(io_err("loopback tcp connect"))?;
    let request_bytes = serde_json::to_vec(envelope).map_err(serde_err)?;
    stream
        .write_all(request_bytes.as_slice())
        .map_err(io_err("loopback tcp write"))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(io_err("loopback tcp shutdown"))?;
    let response_bytes =
        worker
            .join()
            .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
                detail: String::from("loopback buy-mode worker thread panicked"),
            })??;
    serde_json::from_slice::<BuyModeEvalVerdictReceipt>(&response_bytes).map_err(serde_err)
}

// ---------------------------------------------------------------------------
// Remote transport: tailnet / production signed-envelope worker over TCP.
// ---------------------------------------------------------------------------

fn run_remote_buy_mode_worker(
    network_addr: &str,
    envelope: &SignedBuyModeEvalEnvelope,
) -> Result<BuyModeEvalVerdictReceipt, CoordinatorLiveTrainingError> {
    let endpoint = tcp_endpoint(network_addr)?;
    let request_bytes = serde_json::to_vec(envelope).map_err(serde_err)?;
    let mut stream = TcpStream::connect(endpoint.as_str()).map_err(io_err("remote tcp connect"))?;
    stream
        .write_all(request_bytes.as_slice())
        .map_err(io_err("remote tcp write"))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(io_err("remote tcp shutdown"))?;
    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .map_err(io_err("remote tcp read"))?;
    serde_json::from_slice::<BuyModeEvalVerdictReceipt>(&response_bytes).map_err(serde_err)
}

/// Serves one signed buy-mode eval job over a bound TCP listener and exits. This
/// is the worker-side protocol the `Tailnet` / `Production` lanes dial: verify
/// the scheduler envelope, run the injected replay validator, sign and return the
/// verdict receipt. A real Pylon worker binds this (with the real replay
/// validator); the loopback lane runs an equivalent in-process worker thread.
pub fn serve_buy_mode_eval_worker_listener_once<V: BuyModeReplayValidator>(
    listener: TcpListener,
    validator: &V,
) -> Result<(), CoordinatorLiveTrainingError> {
    let (mut inbound, _) = listener.accept().map_err(io_err("worker tcp accept"))?;
    let mut request_bytes = Vec::new();
    inbound
        .read_to_end(&mut request_bytes)
        .map_err(io_err("worker tcp read"))?;
    let envelope =
        serde_json::from_slice::<SignedBuyModeEvalEnvelope>(&request_bytes).map_err(serde_err)?;
    verify_signed_envelope(&envelope)?;
    let (verdict, settled_msats) = validator.validate(&envelope.job)?;
    let receipt = signed_verdict_receipt(&envelope, verdict, settled_msats);
    let response_bytes = serde_json::to_vec(&receipt).map_err(serde_err)?;
    inbound
        .write_all(response_bytes.as_slice())
        .map_err(io_err("worker tcp write"))?;
    inbound
        .shutdown(Shutdown::Write)
        .map_err(io_err("worker tcp shutdown"))?;
    Ok(())
}

/// Serves one signed buy-mode eval job over `bind_addr` (`tcp://host:port` or
/// `host:port`) and exits.
pub fn serve_buy_mode_eval_worker_once<V: BuyModeReplayValidator>(
    bind_addr: &str,
    validator: &V,
) -> Result<(), CoordinatorLiveTrainingError> {
    let endpoint = tcp_endpoint(bind_addr)?;
    let listener = TcpListener::bind(endpoint.as_str()).map_err(io_err("worker tcp bind"))?;
    serve_buy_mode_eval_worker_listener_once(listener, validator)
}

// ---------------------------------------------------------------------------
// Signing / verification (ed25519, deterministic scheduler + worker keys).
// ---------------------------------------------------------------------------

fn signed_verdict_receipt(
    envelope: &SignedBuyModeEvalEnvelope,
    verdict: VerificationClassVerdict,
    settled_msats: u64,
) -> BuyModeEvalVerdictReceipt {
    let signing_key = worker_signing_key(&envelope.job.worker_id);
    let mut receipt = BuyModeEvalVerdictReceipt {
        schema_version: String::from(BUY_MODE_EVAL_VERDICT_RECEIPT_SCHEMA_VERSION),
        envelope_id: envelope.envelope_id.clone(),
        sample_id: envelope.job.sample_id.clone(),
        verification_class: verdict.class,
        passed: verdict.passed,
        settled_msats,
        signed_payload_digest: String::new(),
        worker_pubkey_hex: String::new(),
        signature_hex: String::new(),
        receipt_digest: String::new(),
    };
    receipt.signed_payload_digest = receipt.signable_payload_digest();
    receipt.worker_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
    receipt.signature_hex = hex::encode(
        signing_key
            .sign(receipt.signed_payload_digest.as_bytes())
            .to_bytes(),
    );
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn verify_signed_envelope(
    envelope: &SignedBuyModeEvalEnvelope,
) -> Result<(), CoordinatorLiveTrainingError> {
    if envelope.schema_version != BUY_MODE_EVAL_ENVELOPE_SCHEMA_VERSION {
        return verdict_err("buy-mode eval envelope schema version drifted");
    }
    if envelope.job_digest != job_digest(&envelope.job) {
        return verdict_err("buy-mode eval envelope job_digest drifted");
    }
    if envelope.signed_payload_digest != envelope.signable_payload_digest() {
        return verdict_err("buy-mode eval envelope signed_payload_digest drifted");
    }
    if envelope.envelope_digest != envelope.stable_digest() {
        return verdict_err("buy-mode eval envelope digest drifted");
    }
    let verifying_key = decode_verifying_key(&envelope.scheduler_pubkey_hex)?;
    let signature = decode_signature(&envelope.signature_hex)?;
    verifying_key
        .verify(envelope.signed_payload_digest.as_bytes(), &signature)
        .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
            detail: String::from("buy-mode eval envelope scheduler signature verification failed"),
        })
}

fn verify_worker_verdict_receipt(
    receipt: &BuyModeEvalVerdictReceipt,
    envelope: &SignedBuyModeEvalEnvelope,
) -> Result<(), CoordinatorLiveTrainingError> {
    if receipt.schema_version != BUY_MODE_EVAL_VERDICT_RECEIPT_SCHEMA_VERSION {
        return verdict_err("buy-mode eval verdict receipt schema version drifted");
    }
    if receipt.envelope_id != envelope.envelope_id {
        return verdict_err("buy-mode eval verdict receipt envelope_id does not match the job");
    }
    if receipt.sample_id != envelope.job.sample_id {
        return verdict_err("buy-mode eval verdict receipt sample_id does not match the job");
    }
    if receipt.receipt_digest != receipt.stable_digest() {
        return verdict_err("buy-mode eval verdict receipt digest drifted");
    }
    if receipt.signed_payload_digest != receipt.signable_payload_digest() {
        return verdict_err("buy-mode eval verdict receipt signed_payload_digest drifted");
    }
    let verifying_key = decode_verifying_key(&receipt.worker_pubkey_hex)?;
    let signature = decode_signature(&receipt.signature_hex)?;
    verifying_key
        .verify(receipt.signed_payload_digest.as_bytes(), &signature)
        .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
            detail: String::from(
                "buy-mode eval verdict receipt worker signature verification failed",
            ),
        })
}

fn scheduler_signing_key(run_id: &str) -> SigningKey {
    deterministic_signing_key(format!("coordinator-buy-mode-scheduler|{run_id}").as_str())
}

fn worker_signing_key(worker_id: &str) -> SigningKey {
    deterministic_signing_key(format!("coordinator-buy-mode-worker|{worker_id}").as_str())
}

fn deterministic_signing_key(seed: &str) -> SigningKey {
    let digest = Sha256::digest(seed.as_bytes());
    let mut secret = [0_u8; 32];
    secret.copy_from_slice(&digest[..32]);
    SigningKey::from_bytes(&secret)
}

fn decode_verifying_key(
    public_key_hex: &str,
) -> Result<VerifyingKey, CoordinatorLiveTrainingError> {
    let bytes = hex::decode(public_key_hex).map_err(|error| {
        CoordinatorLiveTrainingError::VerdictSource {
            detail: format!("invalid public key hex: {error}"),
        }
    })?;
    let bytes: [u8; 32] =
        bytes
            .try_into()
            .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
                detail: String::from("public key must be 32 bytes"),
            })?;
    VerifyingKey::from_bytes(&bytes).map_err(|error| CoordinatorLiveTrainingError::VerdictSource {
        detail: format!("invalid public key: {error}"),
    })
}

fn decode_signature(signature_hex: &str) -> Result<Signature, CoordinatorLiveTrainingError> {
    let bytes = hex::decode(signature_hex).map_err(|error| {
        CoordinatorLiveTrainingError::VerdictSource {
            detail: format!("invalid signature hex: {error}"),
        }
    })?;
    let bytes: [u8; 64] =
        bytes
            .try_into()
            .map_err(|_| CoordinatorLiveTrainingError::VerdictSource {
                detail: String::from("signature must be 64 bytes"),
            })?;
    Ok(Signature::from_bytes(&bytes))
}

// ---------------------------------------------------------------------------
// Small helpers (digests, address parsing, error mapping).
// ---------------------------------------------------------------------------

fn job_digest(job: &BuyModeEvalJob) -> String {
    stable_json_digest(b"psionic_coordinator_buy_mode_eval_job|", job)
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    if let Ok(bytes) = serde_json::to_vec(value) {
        hasher.update(bytes);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn tcp_endpoint(network_addr: &str) -> Result<String, CoordinatorLiveTrainingError> {
    let trimmed = network_addr.trim();
    if let Some(endpoint) = trimmed.strip_prefix("tcp://") {
        return non_empty_endpoint(endpoint, network_addr);
    }
    if let Some(endpoint) = trimmed.strip_prefix("tailnet://") {
        return non_empty_endpoint(endpoint, network_addr);
    }
    if !trimmed.contains("://") && trimmed.contains(':') {
        return non_empty_endpoint(trimmed, network_addr);
    }
    verdict_err(format!(
        "remote Pylon worker address `{network_addr}` must be tcp://host:port, tailnet://host:port, or host:port"
    ))
}

fn non_empty_endpoint(
    endpoint: &str,
    original: &str,
) -> Result<String, CoordinatorLiveTrainingError> {
    if endpoint.trim().is_empty() {
        return verdict_err(format!(
            "remote Pylon worker address `{original}` is missing host:port"
        ));
    }
    Ok(endpoint.trim().to_string())
}

fn io_err(path: &'static str) -> impl Fn(std::io::Error) -> CoordinatorLiveTrainingError {
    move |error| CoordinatorLiveTrainingError::VerdictSource {
        detail: format!("buy-mode dispatch I/O failed at `{path}`: {error}"),
    }
}

fn serde_err(error: serde_json::Error) -> CoordinatorLiveTrainingError {
    CoordinatorLiveTrainingError::VerdictSource {
        detail: format!("buy-mode dispatch serialization failed: {error}"),
    }
}

fn verdict_err<T>(detail: impl Into<String>) -> Result<T, CoordinatorLiveTrainingError> {
    Err(CoordinatorLiveTrainingError::VerdictSource {
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_eval_verdict_source::{
        BuyModeEvalJob, CapDebitOwner, CoordinatorArmState, DispatchBackedVerdictSource,
        COORDINATOR_OWNER_DAILY_CAP_MSATS,
    };
    use crate::coordinator_evolution::VerificationVerdict;
    use crate::coordinator_live_training::{DailySpendCap, EvalVerdictSource, TrajectoryRequest};

    fn request(sample: &str) -> TrajectoryRequest {
        TrajectoryRequest {
            worker_index: 0,
            worker_id: "open-pylon-a".to_string(),
            role_index: 1,
            sample_id: sample.to_string(),
        }
    }

    fn job(sample: &str, amount_msats: u64) -> BuyModeEvalJob {
        BuyModeEvalJob {
            worker_id: "open-pylon-a".to_string(),
            role_index: 1,
            sample_id: sample.to_string(),
            amount_msats,
        }
    }

    // ---- Default-off ------------------------------------------------------

    #[test]
    fn arm_state_defaults_to_disarmed() {
        assert_eq!(BuyModeArmState::default(), BuyModeArmState::Disarmed);
        assert!(!BuyModeArmState::default().is_armed());
        assert!(BuyModeArmState::Armed.is_armed());
    }

    #[test]
    fn loopback_dispatcher_is_disarmed_by_default() {
        let dispatch =
            LiveBuyModeDispatch::loopback("run.test", FixtureReplayValidator::exact_trace_pass(0));
        assert!(!dispatch.is_armed());
        assert_eq!(dispatch.mode(), BuyModeDispatchMode::Loopback);
    }

    #[test]
    fn remote_dispatcher_is_disarmed_by_default() {
        let dispatch = LiveBuyModeDispatch::remote(
            BuyModeDispatchMode::Production,
            "tcp://10.0.0.1:9999",
            "run.test",
            FixtureReplayValidator::exact_trace_pass(0),
        );
        assert!(!dispatch.is_armed());
        assert_eq!(dispatch.mode(), BuyModeDispatchMode::Production);
    }

    // ---- Disarmed: refuse, NEVER publish ----------------------------------

    #[test]
    fn disarmed_loopback_refuses_without_publishing() {
        // settled_msats is large; if this ever published it would "spend", so a
        // clean refusal proves no network and no spend.
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(9_999_999),
        );
        let error = dispatch.dispatch_eval(&job("s0", 1_000)).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
        assert_eq!(
            dispatch.last_refusal(),
            Some(BuyModeDispatchRefusal::Disarmed)
        );
    }

    #[test]
    fn disarmed_production_transport_is_unreachable() {
        // A production-mode dispatcher pointed at a bogus address must refuse
        // BEFORE attempting any connection. If it tried to dial it would error on
        // connect, not on the disarmed gate — so asserting the refusal reason
        // proves the transport is unreachable without arming.
        let dispatch = LiveBuyModeDispatch::remote(
            BuyModeDispatchMode::Production,
            // Unroutable TEST-NET-1 address: a dial attempt would fail differently.
            "tcp://192.0.2.1:9",
            "run.test",
            FixtureReplayValidator::exact_trace_pass(0),
        );
        let error = dispatch.dispatch_eval(&job("s0", 1_000)).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
        // Disarmed gate fires first: never dialed. The production lane refuses
        // with the external-transport reason, proving it is unreachable here.
        assert_eq!(
            dispatch.last_refusal(),
            Some(BuyModeDispatchRefusal::ExternalTransportNotArmed)
        );
    }

    // ---- Armed loopback: full signed round trip, real verdict --------------

    #[test]
    fn armed_loopback_round_trips_a_signed_pass_verdict() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(1_000_000),
        )
        .arm();
        assert!(dispatch.is_armed());

        let result = dispatch
            .dispatch_eval(&job("s0", 1_000_000))
            .expect("dispatch");
        assert_eq!(result.verdict, VerificationClassVerdict::exact_trace_pass());
        assert_eq!(result.verdict.class, VerificationClass::ExactTraceReplay);
        assert!(result.verdict.passed);
        assert_eq!(result.settled_msats, 1_000_000);
        assert_eq!(dispatch.last_refusal(), None);
    }

    #[test]
    fn armed_loopback_round_trips_a_signed_fail_verdict() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::new(VerificationClassVerdict::exact_trace_fail(), 500_000),
        )
        .arm();
        let result = dispatch
            .dispatch_eval(&job("s1", 500_000))
            .expect("dispatch");
        assert!(!result.verdict.passed);
        assert_eq!(result.verdict.verdict(), VerificationVerdict::Rejected);
        assert_eq!(result.settled_msats, 500_000);
    }

    // ---- Worker-side protocol: signed envelope verified before answering ----

    #[test]
    fn worker_rejects_a_tampered_envelope_signature() {
        // Build a valid envelope, then tamper the signed amount AFTER signing.
        let dispatch =
            LiveBuyModeDispatch::loopback("run.test", FixtureReplayValidator::exact_trace_pass(0));
        let mut envelope = dispatch.signed_envelope(&job("s0", 1_000));
        // Tamper: bump the amount; the signature and job_digest no longer match.
        envelope.job.amount_msats += 1;
        let error = verify_signed_envelope(&envelope).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
    }

    #[test]
    fn dispatcher_rejects_a_tampered_verdict_receipt() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(7_000),
        );
        let envelope = dispatch.signed_envelope(&job("s0", 7_000));
        let mut receipt = signed_verdict_receipt(
            &envelope,
            VerificationClassVerdict::exact_trace_pass(),
            7_000,
        );
        // Tamper: flip the verdict to a pass-from-fail without re-signing.
        receipt.settled_msats = 0;
        let error = verify_worker_verdict_receipt(&receipt, &envelope).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
    }

    // ---- Address parsing for the remote lanes ------------------------------

    #[test]
    fn tcp_endpoint_accepts_tcp_tailnet_and_bare_hostport() {
        assert_eq!(tcp_endpoint("tcp://1.2.3.4:5").unwrap(), "1.2.3.4:5");
        assert_eq!(tcp_endpoint("tailnet://host:9").unwrap(), "host:9");
        assert_eq!(tcp_endpoint("127.0.0.1:7").unwrap(), "127.0.0.1:7");
        assert!(tcp_endpoint("https://nope").is_err());
        assert!(tcp_endpoint("tcp://").is_err());
    }

    // ---- Remote worker-side protocol over loopback TCP (proves the seam) ----

    #[test]
    fn remote_worker_protocol_serves_one_signed_job() {
        // Bind a worker on loopback and arm a remote (tailnet) dispatcher at it.
        // This exercises the SAME remote transport path the production lane uses,
        // without any external network: the worker runs its own validator.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let endpoint = listener.local_addr().expect("addr").to_string();
        let worker = thread::spawn(move || {
            let validator = FixtureReplayValidator::exact_trace_pass(2_500);
            serve_buy_mode_eval_worker_listener_once(listener, &validator)
        });

        let dispatch = LiveBuyModeDispatch::remote(
            BuyModeDispatchMode::Tailnet,
            format!("tcp://{endpoint}"),
            "run.test",
            FixtureReplayValidator::exact_trace_pass(0),
        )
        .arm();
        let result = dispatch.dispatch_eval(&job("s0", 2_500)).expect("dispatch");
        assert!(result.verdict.passed);
        assert_eq!(result.settled_msats, 2_500);
        worker.join().expect("worker join").expect("worker serve");
    }

    // ---- End-to-end: live dispatcher -> DispatchBackedVerdictSource ---------

    /// Proves the live loopback dispatcher plugs into the merged
    /// [`DispatchBackedVerdictSource`] seam (#1137): an armed source over an armed
    /// loopback dispatcher yields a [`VerdictOutcome`] the coordinator lane
    /// consumes, with the cap pre-check still in front. No external network, no
    /// real inference; the loopback worker only records.
    #[test]
    fn live_loopback_feeds_dispatch_backed_verdict_source() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(1_000_000),
        )
        .arm();
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            1_000_000,
            CapDebitOwner::Source,
            cap,
        );

        let outcome = source.verdict_for(&request("s0")).expect("verdict");
        assert_eq!(outcome.verdict, VerificationVerdict::Verified);
        // Source owns the cap: reports 0 to the fitness, debits the cap itself.
        assert_eq!(outcome.spend_msats, 0);
        assert_eq!(source.cap_snapshot().spent_today_msats(), 1_000_000);
        assert_eq!(source.last_refusal(), None);
    }

    /// Proves the DEFAULT-OFF + fail-closed composition end to end: a DISARMED
    /// verdict source over an armed live dispatcher dispatches NOTHING and moves
    /// no sats. The source's disarmed gate fires before the dispatcher is ever
    /// called.
    #[test]
    fn disarmed_source_over_live_dispatch_moves_no_sats() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(1_000_000),
        )
        .arm();
        let cap = DailySpendCap::owner_default("2026-06-22");
        // Source DISARMED (default) even though the dispatcher is armed.
        let source = DispatchBackedVerdictSource::disarmed(dispatch, 1_000_000, cap);
        assert!(source.verdict_for(&request("s0")).is_err());
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }

    /// Proves the over-cap fail-closed path through the live dispatcher: an armed
    /// source with a per-eval price above the whole cap refuses before the live
    /// dispatcher is reached, so no job is ever published and no sats move.
    #[test]
    fn over_cap_source_over_live_dispatch_never_publishes() {
        let dispatch = LiveBuyModeDispatch::loopback(
            "run.test",
            FixtureReplayValidator::exact_trace_pass(1_000_000),
        )
        .arm();
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            COORDINATOR_OWNER_DAILY_CAP_MSATS + 1, // larger than the whole cap.
            CapDebitOwner::Source,
            cap,
        );
        assert!(source.verdict_for(&request("s0")).is_err());
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }
}
