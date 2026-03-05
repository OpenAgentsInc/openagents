# OpenAgents Economy Kernel — System Diagrams

These diagrams are intended to be **comprehensive** and to cover:

* The **normative kernel spec** (Sections 1–7) + proto-first posture
* The **proto plan** (common/outcomes/policy/snapshot) plus the **missing-but-required ecosystem primitives** from the arXiv recommendations and the gap analysis:

  * insurability preconditions (incident reporting, disclosure, proof-of-coverage hooks)
  * interoperable audit/incident formats + outcome registries
  * privacy-preserving safety signals (public aggregate + restricted feed)
  * identity assurance levels + credential references (incl. personhood where required)
  * certification + safe-harbor / “digital borders” policy gates
  * white-hat audit / bounty workflows as kernel-native WorkUnit lanes
  * explicit rollback / compensating-action semantics
  * exportable synthetic practice packages
  * correlation-adjusted “effective verified share” metrics
  * continuous monitoring + drift detection as a policy input
  * prediction markets as bounded liability coverage instruments
  * proof-of-cost signals for compute integrity and underwriting inputs

Where a lane is *optional*, it is labeled as such; the diagrams still show how it plugs into the same receipts/snapshot/policy substrate.

---

## 1) System Architecture and Trust Boundaries

```mermaid
flowchart LR
  subgraph Z0_PUBLIC[Public Zone]
    STATS[stats public<br/>minute snapshot view]
    OUTREG[(Public Outcome Registry<br/>aggregated redacted)]
  end

  subgraph Z1_RESTRICTED[Restricted Zone Certified Parties]
    SAFEF[Safety Signals Feed<br/>restricted policy gated]
    AUDITDL[AuditPackage Export Feed<br/>restricted modes]
  end

  subgraph Z2_CLIENTS[Clients and Product Surfaces]
    AP[Autopilot Desktop]
    MP[Marketplace and other surfaces]
    VUI[Verifier and adjudicator UI]
    UWI[Underwriter tools]
    AUI[Auditor tools]
    LPU[LP portal optional]
  end

  subgraph Z3_PROJ[Projection and Coordination Plane<br/>non authoritative]
    WS[WebSocket / Nostr / Spacetime<br/>progress and coordination only]
  end

  subgraph Z4_AUTH[Authority Plane HTTP only]
    TR[TreasuryRouter<br/>policy driven planner]
    K[Kernel Authority API<br/>idempotent mutations and receipts]
    SIG[Signer Set<br/>threshold approval for high impact actions]
  end

  subgraph MODS[Kernel Modules]
    SE[Settlement Engine<br/>quote to execute plus proofs]
    CE[Credit Envelopes<br/>bounded credit]
    ABP[Bonds and Collateral<br/>reserve draw release]
    VE[Verification Engine<br/>plan to evidence to verdict]
    LE[Liability Engine<br/>warranty claims remedies]
    MKT[Liability Markets optional<br/>coverage auction and belief signals]
    FX[FX RFQ and Settlement<br/>rfq quote select settle]
    RR[Routing and Risk<br/>breakers and autonomy modes]
    GT[GroundTruth and Synthetic Practice<br/>cases to scenarios]
    RB[Rollback and Comp Actions<br/>machine legible remedies]
    MD[Monitoring and Drift<br/>drift receipts]
    CI[Cost Integrity<br/>cost proof checks and anomalies]
    SI[Safety Certification Interop<br/>identity certs exports]
    RI[Reputation Index<br/>receipt derived priors]
  end

  subgraph DATA[Canonical Substrate Append only Deterministic]
    RS[(Receipt Stream append only)]
    EV[(Evidence Store<br/>artifacts bundles attestations)]
    PB[(PolicyBundle Store<br/>versioned)]
    CR[(Certification Registry<br/>issued revoked receipts)]
    IR[(Incident and GroundTruth Registry<br/>taxonomy coded)]
    OR[(Outcome Registry Store<br/>linkable entries)]
    SS[(EconomySnapshot Store<br/>snapshot_id snapshot_hash)]
    EXP[(AuditPackage Store<br/>export bundles redaction mode)]
  end

  subgraph SNAPLANE[Deterministic Minute Snapshot Lane]
    SCH[Scheduler once per minute]
    SNAP[ComputeSnapshot<br/>idempotent and receipted]
  end

  subgraph CUSTODY[Custody Boundary]
    WE[Wallet Executor<br/>canonical custody]
    RAILS[(LN Onchain FX Optional Solver Rails)]
  end

  subgraph ANCHOR[Optional Anchoring Privacy Preserving]
    ANC[Anchor Service<br/>publish hashes only]
    CHAIN[(Neutral Public Ledger<br/>optional)]
  end

  AP -->|Authenticated HTTP| TR
  MP -->|Authenticated HTTP| TR
  VUI -->|Authenticated HTTP| TR
  UWI -->|Authenticated HTTP| TR
  LPU -->|Authenticated HTTP optional| TR

  TR -->|Policy bounded commands| K
  SIG -->|Threshold approval| K

  K --> SE
  K --> CE
  K --> ABP
  K --> VE
  K --> LE
  K --> MKT
  K --> FX
  K --> RR
  K --> GT
  K --> RB
  K --> MD
  K --> CI
  K --> SI
  K --> RI

  SE --> WE
  FX --> WE
  WE --> RAILS

  K --> RS
  K --> EV
  K --> PB
  K --> CR
  K --> IR
  K --> OR
  K --> EXP

  SCH --> SNAP
  RS --> SNAP
  SNAP --> SS

  SS --> STATS
  OR --> OUTREG
  SS --> SAFEF
  EXP --> AUDITDL

  K -. projection only .-> WS
  AP <-->|progress only| WS
  MP <-->|progress only| WS
  VUI <-->|progress only| WS

  SS -. hashes only .-> ANC
  RS -. hashes only .-> ANC
  ANC --> CHAIN
```

---

## 2) Canonical Lifecycle with Preconditions, Verification, Liability, Monitoring, and Remedies

This is the “full-fidelity” lifecycle including the paper’s recommended primitives: disclosure/coverage hooks, identity assurance gates, incident reporting, drift, rollback/compensation, and exportability.

```mermaid
sequenceDiagram
  autonumber
  participant Buyer as Buyer/Operator
  participant TR as TreasuryRouter
  participant Kernel as Kernel Authority API
  participant Policy as PolicyBundle Eval
  participant ID as Identity/Assurance Gate
  participant UW as Underwriter (optional)
  participant Market as Liability Markets optional
  participant Worker as Worker/Provider
  participant Verifier as Verifier/Adjudicator
  participant Wallet as Wallet Executor
  participant Monitor as Monitoring/Drift
  participant Remedy as Rollback/Compensating Actions
  participant Claims as Claims/Disputes
  participant Incident as Incident/GroundTruth Lane
  participant Snap as Snapshot Compute
  participant Export as Audit/Interop Export
  participant Receipts as Receipt Stream

  %% --- Work definition ---
  Buyer->>TR: CreateWorkUnit(category, tfb, severity, B, trace, policy_bundle_id, idempotency_key)
  TR->>Kernel: Authority request (policy-bounded)
  Kernel-->>Receipts: work_unit.created (receipt + hints)

  %% --- Preconditions: identity, disclosure, coverage, certification ---
  Buyer->>TR: CreateContract(verification plan, provenance reqs, warranty/terms, bonds)
  TR->>Policy: Evaluate slice rules (category×tfb×severity precedence)
  Policy->>ID: Require assurance? personhood? org-vetted?
  ID-->>Policy: Assurance OK / WITHHELD(reason_code)
  Policy->>Kernel: Enforce preconditions (DisclosureBundle / ProofOfCoverage / Certification if required)
  alt Preconditions satisfied
    Kernel-->>Receipts: contract.created (terms hash, required tier, provenance, identity gates)
  else Preconditions fail
    Kernel-->>Receipts: contract.withheld (reason_code + policy rule id + snapshot binding if applicable)
  end

  %% --- Funding / bounded credit / bonds ---
  Buyer->>TR: Fund escrow / Issue envelope / Reserve bonds (bounded)
  TR->>Kernel: Authority commands (idempotent)
  Kernel->>Wallet: quote→execute / reserve (expiry-bound)
  Wallet-->>Kernel: proof refs (preimage/txid/etc.)
  Kernel-->>Receipts: funding + envelope + bond receipts (typed reason_codes)

  %% --- Optional underwriting lane ---
  opt Underwriter provides coverage / warranty backing
    UW->>TR: Post underwriter bond / ProofOfCoverageRef (optional)
    TR->>Kernel: Reserve bond / record coverage evidence
    Kernel-->>Receipts: coverage/bond receipts (policy-linked)
  end

  %% --- Optional market coverage binding lane ---
  opt Coverage market enabled for slice
    UW->>Market: PlaceCoverageOffer collateralized
    Market->>Kernel: BindCoverage contract and resolution ref
    Kernel->>Policy: Check safe harbor thresholds for this slice
    Policy-->>Kernel: Allow or deny relaxation
    Kernel-->>Receipts: market.coverage_offer.placed / market.coverage.bound
    Kernel-->>Receipts: policy.relaxation.applied or policy.relaxation.denied
  end

  %% --- Submission / provenance ---
  Worker->>TR: Submit outputs + evidence + provenance ref + cost proof ref
  TR->>Kernel: Authority request
  Kernel-->>Receipts: submission.received (provenance refs recorded)

  %% --- Verification & verdict ---
  Kernel->>Verifier: Execute verification plan (tier + independence constraints)
  Verifier-->>Kernel: Evidence + IndependenceReport + adjudication output (if needed)
  Kernel-->>Receipts: verdict.finalized (tier, Pgrade, correlation flags)

  %% --- Settlement & warranty issuance ---
  alt Verdict unlocks settlement under policy
    Kernel->>Wallet: Settle payment/refund (quote-linked, expiry-bound)
    Wallet-->>Kernel: settlement proof
    Kernel-->>Receipts: settlement receipt (PAID/WITHHELD/FAILED + reason_code)
    opt Warranty enabled
      Kernel-->>Receipts: warranty.issued / warranty.active (terms hash, window)
    end
  else Verdict recorded but settlement withheld
    Kernel-->>Receipts: settlement.withheld (reason_code = VERIFICATION_INSUFFICIENT or PROVENANCE_TOO_LOW or COVERAGE_INSUFFICIENT or COST_PROOF_MISSING)
  end

  %% --- Monitoring / drift detection (continuous) ---
  par Monitoring runs during warranty window / long feedback loops
    Monitor->>Kernel: Emit drift signals (policy-gated)
    Kernel-->>Receipts: drift.signal / drift.alert (typed + trace-linked)
    Monitor->>Kernel: Emit cost anomaly signal if variance breach
    Kernel-->>Receipts: economy.cost.anomaly_detected
  and Snapshot loop
    Snap->>Kernel: ComputeSnapshot(as_of_ms minute boundary UTC, idempotency_key)
    Kernel-->>Receipts: snapshot.emitted (receipt)
  end

  %% --- Claims / disputes / remedies ---
  alt Claim or incident triggered (failure, drift, dispute)
    Buyer->>Claims: OpenClaim(evidence refs, reason)
    Claims->>Kernel: Resolve under adjudication policy
    Kernel-->>Receipts: claim.resolution (APPROVED/DENIED/PARTIAL)

    opt Remedy requires rollback or compensating action
      Kernel->>Remedy: Execute rollback/comp action (policy-bounded)
      Remedy-->>Kernel: outcome + proofs
      Kernel-->>Receipts: rollback.executed / compensating_action.executed (or FAILED + reason_code)
    end

    Kernel->>Wallet: Execute remedy settlement (refund/damages/rework credit)
    Wallet-->>Kernel: proof refs
    Kernel-->>Receipts: bond draws/releases + remedy settlement receipts
  end

  %% --- Incident / ground truth & synthetic practice ---
  opt Major incident / near miss above threshold
    Incident->>Kernel: File IncidentReport / GroundTruthCase (taxonomy-coded, receipt-linked)
    Kernel-->>Receipts: incident.recorded / ground_truth.created
    Kernel-->>Receipts: simulation_scenario.derived (refers to evidence digests)
  end

  %% --- Export / interoperability (policy-gated) ---
  Export->>Kernel: Build AuditPackage(export mode: public/redacted/restricted)
  Kernel-->>Receipts: audit_package.emitted (hashes only)
```

---

## 3) Canonical Receipt & Evidence Graph (Navigability + Interop Exports)

This diagram is the “what happened / why / evidence” graph your spec requires, plus the missing interoperability objects (incident reports, outcome registry entries, certifications, audit packages).

```mermaid
flowchart TB
  WU[WorkUnit<br/>category tfb severity B] --> CT[Contract<br/>terms hash]
  CT --> SUB[Submission<br/>outputs evidence provenance ref]

  SUB --> PBNDL[ProvenanceBundle<br/>toolchain lineage attestations correlation]
  SUB --> CPB[CostProofBundle<br/>attestation level usage metering]
  SUB --> EVD[Evidence Bundles<br/>harness rubric adjudication drift]

  SUB --> VR[Verdict Receipt<br/>tier Pgrade independence correlation]
  VR --> EVD

  CT --> INT[Intent Envelope Bond intents]
  INT --> SET[Settlement Receipt<br/>paid withheld failed<br/>reason_code proofs]
  INT --> BOND[Bond Receipts<br/>reserve draw release]
  CT --> COV[CoverageBinding<br/>cap premium window resolution]
  COV --> MKS[MarketSignal<br/>implied fail calibration disagreement]
  COV --> BOND

  VR --> WRY[Warranty Receipts<br/>issued active expired]
  WRY --> CLM[Claim Receipts<br/>open review resolution]
  CLM --> REM[Remedy Receipts<br/>refund damages rework<br/>rollback comp action]
  REM --> SET
  REM --> BOND

  CT --> DR[Drift Receipts<br/>signal alert resolved]
  DR --> CLM

  CLM --> IRPT[IncidentReport or GroundTruthCase<br/>taxonomy coded]
  IRPT --> GTLINK[Required Linkage<br/>replay digests verdict receipts<br/>settlement receipts policy version]
  IRPT --> SIM[SimulationScenario<br/>derived redacted package]
  SIM --> VCAP[Verifier Capacity<br/>quals and performance]

  VR --> ORG[OutcomeRegistryEntry<br/>aggregated linkable]
  CT --> DISC[DisclosureBundleRef<br/>risk card coverage exclusions]
  CT --> POC[ProofOfCoverageRef<br/>insurance captive reserve]
  CT --> CERT[SafetyCertification Ref<br/>issued revoked receipts]

  RS[(Receipt Stream)] --> SNAP[EconomySnapshot<br/>sv sv_effective Δm_hat XA_hat<br/>coverage calibration concentration<br/>cost integrity anomalies<br/>loss ratio auth cert shares]
  MKS --> SNAP
  CPB --> SNAP
  SNAP --> STATS[stats public<br/>redacted]

  VR --> APKG[AuditPackage<br/>versioned export schema<br/>redaction mode]
  SET --> APKG
  CLM --> APKG
  IRPT --> APKG
  CERT --> APKG
  SNAP --> APKG
```

---

## 4) Normative State Machines Overview (Extended)

This expands beyond the base spec state machines to include: warranty, rollback, drift, incidents, snapshots, and certification.

```mermaid
flowchart TB
  subgraph Settlement_SM[Settlement]
    S_QUOTED[QUOTED] --> S_PAID[PAID]
    S_QUOTED --> S_WITHHELD[WITHHELD]
    S_QUOTED --> S_FAILED[FAILED]
  end

  subgraph Envelope_SM[Credit Envelope]
    E_INTENT[INTENT_CREATED] --> E_OFFERED[OFFERED]
    E_OFFERED --> E_ISSUED[ENVELOPE_ISSUED]
    E_ISSUED --> E_COMMITTED[COMMITTED]
    E_COMMITTED --> E_SETTLED[SETTLED]
    E_ISSUED --> E_EXPIRED[EXPIRED]
    E_COMMITTED --> E_REVOKED[REVOKED]
  end

  subgraph Contract_SM[Contract]
    C_CREATED[CREATED] --> C_FUNDED[FUNDED]
    C_FUNDED --> C_BONDED[BONDED]
    C_BONDED --> C_SUBMITTED[SUBMITTED]
    C_SUBMITTED --> C_VERIFYING[VERIFYING]
    C_VERIFYING --> C_PASS[VERDICT_PASS]
    C_VERIFYING --> C_FAIL[VERDICT_FAIL]
    C_PASS --> C_SETTLED[SETTLED]
    C_FAIL --> C_SETTLED
    C_SETTLED --> C_WARRANTY[WARRANTY_ACTIVE optional]
    C_SETTLED --> C_FINAL[FINALIZED]
    C_WARRANTY --> C_FINAL
    C_CREATED --> C_CANCEL[CANCELLED policy gated]
  end

  subgraph Claims_SM[Claims]
    CL_OPEN[OPEN] --> CL_REVIEW[UNDER_REVIEW]
    CL_REVIEW --> CL_APP[APPROVED]
    CL_REVIEW --> CL_DEN[DENIED]
    CL_REVIEW --> CL_PART[PARTIALLY_APPROVED]
    CL_APP --> CL_PAID[PAID]
    CL_PART --> CL_PAID
    CL_DEN --> CL_CLOSED[CLOSED]
    CL_PAID --> CL_CLOSED
  end

  subgraph Solver_SM[Optional Solver Cross Rail]
    X_INTENT[INTENT_CREATED] --> X_MATCH[MATCHED]
    X_MATCH --> X_INIT[INITIATED]
    X_INIT --> X_RED[REDEEMED]
    X_INIT --> X_REF[REFUNDED]
    X_INIT --> X_EXP[EXPIRED]
    X_INIT --> X_FAIL[FAILED]
  end

  subgraph MarketCoverage_SM[Optional Markets Coverage Binding]
    M_OPEN[OFFERING_OPEN] --> M_PROP[BINDING_PROPOSED]
    M_PROP --> M_BOUND[BOUND]
    M_BOUND --> M_ACTIVE[ACTIVE]
    M_ACTIVE --> M_CLAIM[CLAIM_TRIGGERED]
    M_ACTIVE --> M_EXP[EXPIRED]
    M_CLAIM --> M_SET[SETTLED]
    M_EXP --> M_SET
  end

  subgraph BeliefPos_SM[Optional Markets Belief Position]
    BP_OPEN[OPEN] --> BP_LOCK[MARGIN_LOCKED]
    BP_LOCK --> BP_REDUCED[REDUCED]
    BP_LOCK --> BP_CLOSED[CLOSED]
    BP_REDUCED --> BP_CLOSED
    BP_CLOSED --> BP_SETTLED[SETTLED]
  end

  %% Warranty
  subgraph Warranty_SM[Warranty]
    W_ISSUED[ISSUED] --> W_ACTIVE[ACTIVE]
    W_ACTIVE --> W_EXPIRED[EXPIRED]
    W_ACTIVE --> W_CLAIMED[CLAIM_OPEN]
    W_CLAIMED --> W_ACTIVE
    W_CLAIMED --> W_CLOSED[CLOSED]
  end

  subgraph Remedy_SM[Rollback Comp Action]
    R_PLANNED[PLANNED] --> R_INIT[INITIATED]
    R_INIT --> R_OK[EXECUTED]
    R_INIT --> R_BAD[FAILED]
    R_OK --> R_DONE[COMPLETE]
    R_BAD --> R_DONE
  end

  subgraph Drift_SM[Monitoring Drift]
    D_BASE[BASELINE] --> D_MON[MONITORING]
    D_MON --> D_ALERT[ALERT_RAISED]
    D_ALERT --> D_ACK[ACKNOWLEDGED]
    D_ACK --> D_RES[RESOLVED]
    D_RES --> D_MON
  end

  subgraph CostProof_SM[Cost Integrity]
    CP_SUB[SUBMITTED] --> CP_VER[VERIFIED]
    CP_VER --> CP_OK[ACCEPTED]
    CP_VER --> CP_FLAG[ANOMALY_FLAGGED]
  end

  subgraph Incident_SM[Incident GroundTruth]
    I_OPEN[OPEN] --> I_TRIAGE[TRIAGED]
    I_TRIAGE --> I_CONF[CONFIRMED]
    I_CONF --> I_ROOT[ROOT_CAUSED]
    I_ROOT --> I_CASE[GROUND_TRUTH_CASE]
    I_CASE --> I_SCEN[SIMULATION_DERIVED]
    I_SCEN --> I_CLOSED[CLOSED]
  end

  subgraph Snapshot_SM[EconomySnapshot Compute]
    N_SCHED[SCHEDULED] --> N_COMP[COMPUTED]
    N_COMP --> N_PUB[PUBLISHED]
    N_COMP --> N_WITHH[WITHHELD_INPUT_MISSING]
  end

  subgraph Cert_SM[Safety Certification]
    C_REQ[REQUESTED] --> C_REVIEW[EVIDENCE_REVIEW]
    C_REVIEW --> C_ISS[ISSUED]
    C_ISS --> C_SUSP[SUSPENDED]
    C_ISS --> C_REV[REVOKED]
    C_ISS --> C_EXP[EXPIRED]
  end
```

---

## 5) Control Loop: Receipts → Snapshot → Deterministic Policy → Actions → Receipts

This is the core “governor” loop, extended to include: identity/cert/coverage gates, incident & drift signals, and effective verified scale.

```mermaid
flowchart TD
  R[(Receipt Stream canonical append only)] --> SNAP[Compute EconomySnapshot<br/>UTC minute boundary<br/>idempotent and receipted]
  P[(Signed Pool Snapshots LP mode optional)] --> SNAP

  SNAP --> M[Metrics table first<br/>sv sv_effective rho_effective<br/>Δm_hat XA_hat<br/>correlated share<br/>coverage share concentration<br/>implied fail and calibration<br/>cost proof share anomalies<br/>cost integrity score spread<br/>incident and drift rates<br/>auth and cert distribution<br/>capital coverage ratios]

  M --> POL[PolicyBundle evaluation deterministic<br/>category tfb severity precedence<br/>deterministic tie break<br/>record rule ids]
  POL --> ACT[Deterministic action order<br/>1 autonomy mode<br/>2 raise tier require human step<br/>3 raise provenance require attestations<br/>4 tighten or halt envelopes<br/>5 disable or cap warranty<br/>6 enforce identity cert coverage gates<br/>7 apply or deny market safe harbor<br/>8 enforce cost proof level and anomaly actions]

  ACT --> DEC[Authority decisions<br/>allow withhold fail<br/>typed reason_code<br/>bind snapshot_id and snapshot_hash]
  DEC --> R

  SNAP --> PUB[stats public snapshot<br/>redacted cached]
  SNAP --> RESTR[Restricted safety signals feed<br/>policy gated]
  R --> AUD[AuditPackage export<br/>versioned redaction mode]
  AUD --> RESTR

  SNAP -. hashes only .-> ANC[Optional anchoring receipt]
  R -. hashes only .-> ANC
```

---

## 6) Insurability, Certification, and “Digital Borders” (Kernel-Addressable Primitives)

This diagram makes explicit the “insurance boundary” and certification gates as first-class kernel behaviors, without requiring any specific external regulator.

```mermaid
flowchart LR
  subgraph Actors
    BUY[Buyer Operator]
    WRK[Worker Provider]
    VER[Verifier Adjudicator]
    UW[Underwriter Insurer optional]
    AUDR[Auditor optional]
    ISS[Credential and cert issuers<br/>optional external]
  end

  subgraph Kernel
    POL[PolicyBundle gates<br/>identity cert coverage]
    CT[Contract creation<br/>precondition checks]
    MKT[Liability markets optional<br/>coverage bind signals]
    LE[Liability engine<br/>premiums claims]
    CI[Cost integrity lane<br/>cost proof validation]
    IR[Incident and groundtruth<br/>taxonomy and linkage]
    OR[Outcome registry<br/>aggregated entries]
    SF[Safety signals<br/>public and restricted]
    EXP[AuditPackage export<br/>versioned redaction]
  end

  WRK -->|credential proofs| ISS
  VER -->|credential proofs| ISS
  UW -->|coverage evidence| ISS

  BUY --> CT
  CT --> POL

  POL -->|require disclosure| DISC[DisclosureBundleRef<br/>risk card coverage exclusions]
  POL -->|require proof of coverage| POC[ProofOfCoverageRef<br/>insurance captive reserve]
  POL -->|require certification| CERT[SafetyCertification<br/>issued revoked receipts]
  POL -->|require identity assurance| IDG[AuthAssuranceLevel<br/>personhood org vetted etc]

  WRK -->|cost proof bundle ref| CI
  CI -->|integrity and anomaly signals| LE
  CT --> MKT
  MKT --> LE
  CT --> LE
  LE -->|claims and remedies| IR
  IR --> OR
  IR --> SF
  OR --> SF

  IR --> EXP
  OR --> EXP
  CERT --> EXP
  LE --> EXP
  EXP --> AUDR
  SF --> AUDR
  SF --> UW
```

---

## 7) Proto Package Dependency Map (Expanded for Interop + Safety)

This extends your proto plan with the minimal additional packages required by the paper/gap analysis. You can treat these as **recommended** additions even if you implement them incrementally.

```mermaid
flowchart LR
  COMMON[common/v1/common.proto<br/>trace money receipts<br/>provenance identity hints]
  OUT[aegis/outcomes/v1/outcomes_work.proto<br/>work contract verdict claims cost proof ref]
  MKT[aegis/markets/v1/liability_market.proto<br/>coverage offers bindings signals]
  INC[aegis/incidents/v1/incidents.proto<br/>incident near miss<br/>ground truth taxonomy]
  POL[policy/v1/policy_bundle.proto<br/>tiers provenance autonomy<br/>identity cert coverage gates]
  SNAP[economy/v1/economy_snapshot.proto<br/>sv_effective xa_hat<br/>drift incident loss auth cert stats<br/>market and cost integrity stats]
  HYDRA[hydra/v1/abp_bonds.proto]

  ID[identity/v1/identity.proto<br/>assurance levels<br/>credential refs]
  CERT[compliance/v1/certification.proto<br/>issue revoke receipts]
  SAFE[safety/v1/safety_signals.proto<br/>public aggregates<br/>restricted feed]
  AUD[audit/v1/audit_package.proto<br/>versioned export<br/>redaction]
  REG[registry/v1/outcome_registry.proto<br/>linkable entries]
  MON[monitoring/v1/drift.proto<br/>drift receipts detectors]
  DISC[interop/v1/disclosure.proto<br/>disclosure bundle<br/>proof of coverage refs]
  RBK[remedy/v1/rollback.proto<br/>rollback and comp action receipts]

  COMMON --> OUT
  COMMON --> MKT
  COMMON --> INC
  COMMON --> POL
  COMMON --> SNAP
  COMMON --> ID
  COMMON --> CERT
  COMMON --> SAFE
  COMMON --> AUD
  COMMON --> REG
  COMMON --> MON
  COMMON --> DISC
  COMMON --> RBK

  HYDRA --> OUT
  POL --> OUT
  POL --> MKT
  OUT --> SNAP
  MKT --> SNAP
  INC --> SNAP
  MON --> SNAP
  REG --> SNAP

  INC --> AUD
  CERT --> AUD
  SAFE --> AUD
  SNAP --> AUD
  OUT --> AUD
  DISC --> OUT
  ID --> POL
  CERT --> POL
  RBK --> OUT
```

---

## 8) Autopilot Desktop ↔ Economy Kernel

How the Autopilot desktop app connects to and uses the economy kernel: authority path (commands and canonical data), projection path (progress only), and local state (receipt stream, snapshot derivation, job lifecycle).

```mermaid
flowchart TB
  subgraph AP[Autopilot Desktop]
    UI[UI Panes<br/>earnings scoreboard job inbox<br/>economy snapshot treasury simulation<br/>agent profile trajectory audit]
    EKR[EarnKernelReceiptState<br/>receipt stream load persist<br/>policy slice evaluation<br/>work unit metadata idempotency]
    ESN[EconomySnapshotState<br/>load persist snapshots<br/>compute_minute_snapshot from receipts]
    EJP[EarnJobLifecycleProjection<br/>derived from same events<br/>job lifecycle rows for UI]
    JOB[Job and earn state<br/>job_inbox active_job job_history<br/>provider ingress]
  end

  subgraph AUTH[Authority Plane]
    TR[TreasuryRouter]
    K[Kernel Authority API]
    RS[(Receipt Stream)]
    SS[(EconomySnapshot Store)]
  end

  subgraph PROJ[Projection Plane non authoritative]
    WS[WebSocket / Nostr / Spacetime<br/>progress and coordination only]
  end

  %% User actions: future HTTP commands to kernel
  UI -->|CreateWorkUnit CreateContract<br/>Fund Submit etc intended| TR
  TR --> K
  K --> RS
  K --> SS

  %% Receipt stream and snapshot: where they come from
  RS -.->|subscription sync or HTTP tail optional<br/>MVP: local file| EKR
  SS -.->|stats public optional<br/>MVP: local compute| ESN
  EKR -->|receipts as input| ESN
  ESN -->|compute_minute_snapshot<br/>idempotent per UTC minute| ESN

  %% Local recording: app emits receipts for job lifecycle
  JOB -->|record_ingress_request<br/>record_active_job_stage<br/>record_preflight_rejection<br/>record_history_receipt<br/>record_swap_execute_attempt<br/>record_economy_snapshot_receipt| EKR
  EKR --> EJP
  EKR --> UI
  ESN --> UI
  EJP --> UI

  %% Progress only
  UI <-->|progress only| WS
```

**In plain English:**

- **Autopilot’s local state**
  The app keeps an **earn kernel receipt stream** (load/save from a local file), an **economy snapshot** state (snapshots loaded/saved and/or **computed from receipts** at UTC minute boundaries), and an **earn job lifecycle projection** (derived from the same events for the job/earnings UI). Job inbox, active job, job history, and provider ingress feed into both the projection and the receipt state.

- **Authority path (intended)**
  User actions (create work, create contract, fund, submit, etc.) are sent over **authenticated HTTP to TreasuryRouter**, which talks to the Kernel Authority API. The kernel writes to the canonical **Receipt Stream** and **EconomySnapshot Store**. The app does not mutate kernel state except via these commands.

- **How the app gets kernel data**
  In the full design the app can use **subscription-driven sync** and/or a receipt-stream HTTP tail, and consume the **stats public** snapshot (redacted) without polling loops. In the **current MVP** the receipt stream is a **local file** (same schema as the kernel’s); the app **computes** economy snapshots locally from that receipt stream and persists them, and may later replace this with kernel-published stats or receipt tail.

- **Local receipt recording**
  The app **records receipts locally** for job-lifecycle events (ingress request, active job stage, preflight rejection, history receipt, swap execute attempt, economy snapshot receipt). That keeps a single receipt stream and projection consistent on the client; when the kernel is authoritative, those events would be produced by the kernel after the app sends commands, and the app would consume kernel receipts instead of (or in addition to) writing its own.

- **Projection path**
  Autopilot and the kernel (or other services) can use **WebSocket, Nostr, or Spacetime** for **progress and coordination only**—no authority for money, verdicts, or state. (Nostr = protocol for relays/identity/job coordination; Spacetime = sync/presence/projection backend.) The diagram shows the desktop app and this projection plane as separate from the authority path.

---

### Notes for maintainers (diagram intent)

* **Authority plane is HTTP-only**: everything that mutates money/credit/liability/verdict/breakers/snapshots is idempotent + receipted.
* **Projection plane is non-authoritative**: progress streams only.
* **Public vs restricted publication**: `/stats` is public and redacted; safety/audit feeds can have restricted modes for certified parties; *all* derived from receipts/snapshots.
* **Interop is a first-class output**: AuditPackage, incident taxonomy objects, outcome registry entries, and (optional) anchoring are all receipts-first and exportable.

If you want, I can also add two more diagram pages that are sometimes useful in implementation:

1. a “Reason Code + Receipt Type Registry” diagram (who emits what, and how it links), and
2. a “Privacy/Redaction Transform” diagram (what can be public vs restricted vs internal evidence).
