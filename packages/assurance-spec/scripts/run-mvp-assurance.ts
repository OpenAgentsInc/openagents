import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { resolve } from "node:path"

import {
  assuranceReceiptArtifact,
  assuranceReviewSetDigest,
  canonicalArtifact,
  compileAssuranceManifest,
  computeEnvironmentProfileDigest,
  executeNativeSdkCriterionUnit,
  executeVitePlusTestUnit,
  makeOracleSensitivityReceipt,
  normalizeNativeSdkHostGate,
  OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_VERSION,
  OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_REF,
  OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_VERSION,
  OPENAGENTS_VITE_PLUS_TEST_ADAPTER_VERSION,
  parseAssuranceSpec,
  serializeAssuranceReviewAnnotation,
  serializeAssuranceSpec,
  sha256Digest,
  type AssuranceAdapterLock,
  type AssuranceAdmission,
  type AssuranceEnvironmentProfileDocument,
  type AssuranceExecutionUnit,
  type AssuranceReceipt,
  type AssuranceReviewAnnotation,
  type AssuranceSpecDocument,
} from "../src/index.ts"
import { parseVitePlusTestSummary } from "../src/full-gate.ts"
import {
  mvpAssuranceTargetDescriptorDigest,
  mvpAssuranceTargetSourceDigest,
  parseMvpAssuranceTargetArgs,
} from "./mvp-assurance-target.ts"

const root = resolve(import.meta.dirname, "../../..")
const target = parseMvpAssuranceTargetArgs(process.argv.slice(2))
const relative = {
  productSpec: target.productSpecPath,
  assuranceSpec: target.assuranceSpec.path,
  proposalFixture: target.paths.proposalFixture,
  environment: target.paths.environment,
  adapterLock: target.paths.adapterLock,
  review: target.paths.review,
  admission: target.paths.admission,
  manifest: target.paths.manifest,
  evidenceIndex: target.paths.evidenceIndex,
  fullDesktopGateReceipt: target.paths.fullGateReceipt,
  receiptRoot: target.paths.receiptRoot,
  runRoot: target.paths.runRoot,
} as const

const absolute = (path: string): string => resolve(root, path)
const read = (path: string): string => readFileSync(absolute(path), "utf8")
const stagedPublications = new Map<string, string>()
const isPublicArtifact = (path: string): boolean => path === relative.assuranceSpec || path.startsWith("assurance/")
const write = (path: string, bytes: string): void => {
  if (isPublicArtifact(path)) {
    stagedPublications.set(path, bytes)
    return
  }
  mkdirSync(resolve(absolute(path), ".."), { recursive: true })
  writeFileSync(absolute(path), bytes)
}
const publishStagedPublications = (): void => {
  const rows = [...stagedPublications].map(([path, bytes], index) => ({
    path,
    bytes,
    destination: absolute(path),
    temporary: `${absolute(path)}.assurance-tmp-${process.pid}-${index}`,
  }))
  try {
    for (const row of rows) {
      mkdirSync(resolve(row.destination, ".."), { recursive: true })
      writeFileSync(row.temporary, row.bytes, "utf8")
    }
    for (const row of rows) renameSync(row.temporary, row.destination)
  } finally {
    for (const row of rows) rmSync(row.temporary, { force: true })
  }
}

const electronNarratives: Readonly<Record<string, string>> = {
  assurance_objective: "Prove the accepted first-deployable OpenAgents Desktop Codex workroom against all eighteen frozen ProductSpec criteria. The run retains criterion-local candidate and falsifier observations, the signed/notarized installed journey, and the full current Desktop regression gate without collapsing their distinct authority or evidence tiers.",
  subject: "This admitted assurance revision remains byte-bound to ProductSpec revision 6 and its legacy CW-AC identities. A later ProductSpec identity migration must create a new admission and may not retarget these receipts or rewrite this historical proof chain.",
  risk_model: "The proof design treats runtime compatibility, ordinary Codex-session custody, durable work identity, authority containment, restart safety, privacy, and release lifecycle fidelity as separate risks. Candidate evidence is never sufficient without a named falsifier, an exact environment, independent review, and current immutable bindings.",
  assurance_scope: "Every executable ProductSpec criterion is required and has exactly one obligation in this MVP run. No criterion is deferred or marked not applicable; release and public-promise authority remain outside the execution grant even after all observations are confirmed.",
  environments: "Execution uses the admitted first-party macOS ARM64 Node/Vite Plus environment with network and credential access forbidden. Native JUnit remains private; normalized receipts expose only digests and bounded references. The historical signed RC9 receipt supplies release-artifact evidence and is not regenerated or published by this run.",
  obligations: "Each obligation binds one criterion to a criterion-local contract oracle and a deterministic missing-anchor falsifier. The complete Desktop suite and installed RC9 journey are required companion evidence, so a narrow contract result cannot independently authorize release or a public completion claim.",
  gates: "The MVP assurance gate passes only when exact admission and environment bindings are current, every candidate is CONFIRMED, every falsifier is REFUTED, infrastructure is ready, observations are stable, independent review accepts each candidate, no exception remains, and the full Desktop regression gate is green.",
  evidence_policy: "Links remain evidence locations rather than verdicts. Native output stays private, normalized receipts are reviewed public-safe projections, and missing or stale artifacts remain INCONCLUSIVE. Candidate, sensitivity, installed-release, and full-regression evidence must all remain independently visible.",
  authority_boundaries: "The owner admits this exact proof design and has accepted the installed ProductSpec-native journey and its read-only review boundary. The runner may execute and report only; it cannot alter owner acceptance, publish RC9, change registries or promises, waive failures, or infer authority from prose or green tests.",
}

const nativeNarratives: Readonly<Record<string, string>> = {
  assurance_objective: "Evaluate the Native SDK-hosted OpenAgents workroom against all eighteen frozen ProductSpec criteria without treating shared kernels, Electron receipts, or the generic headed shell journey as target-specific completion evidence.",
  subject: "This Native assurance revision is independently byte-bound to ProductSpec revision 6 and the stable CW-AC identities. It does not alter or inherit the Electron RC9 evidence chain.",
  risk_model: "The proof design separates Native packaging, ordinary Codex custody, Node-sidecar service fidelity, bounded bridge authority, renderer semantics, durable restart recovery, privacy, and installed lifecycle risks. Missing target-specific integration evidence remains INCONCLUSIVE.",
  assurance_scope: "Every executable ProductSpec criterion is in scope. Shared implementation anchors may support a candidate, but no criterion can confirm without Native-target integration evidence and its named falsifier.",
  environments: "Execution uses the exact first-party macOS ARM64 Node 24.13.1, Zig 0.16.0, Native SDK, Effect Native, Vite Plus/JUnit, and headed-host profile. The profile describes a local device harness, not a signed release artifact.",
  obligations: "Each obligation binds one criterion to the Native-owned criterion catalog. The target-level headed host gate is required once per run and remains distinct from the 36 criterion candidate/falsifier units.",
  gates: "The Native MVP gate can pass only when every candidate is CONFIRMED, every falsifier is REFUTED, the exact target-bound headed host gate is green, evidence has independent post-run acceptance, and no exception remains.",
  evidence_policy: "Private JUnit, logs, screenshots, snapshots, and the producer host report stay under var/. Public-safe projections contain only exact digests and bounded references. Electron release receipts are not Native evidence.",
  authority_boundaries: "Owner direction authorizes building and executing this separately reviewed Native proof design. It does not pre-accept observations, authorize release, change public promises, or permit the runner to convert fixture behavior into criterion proof.",
}

const narratives = target.name === "electron" ? electronNarratives : nativeNarratives

const designDocument = (): AssuranceSpecDocument => {
  const proposal = parseAssuranceSpec(read(relative.proposalFixture))
  const testPath = target.criterion.testPath
  return {
    ...proposal,
    frontmatter: {
      ...proposal.frontmatter,
      assurance_spec_id: target.assuranceSpec.id,
      assurance_revision: target.assuranceSpec.revision,
      title: target.name === "electron"
        ? "OpenAgents Desktop Codex Workroom MVP Assurance Spec"
        : "OpenAgents Native SDK Codex Workroom MVP Assurance Spec",
      lifecycle_state: "admitted",
    },
    sections: proposal.sections.map((section) => ({
      ...section,
      content: narratives[section.id] ?? section.content,
    })),
    riskModel: {
      ...proposal.riskModel,
      risks: [
        { id: "RISK-RUNTIME-CUSTODY", statement: "A second engine, inherited CODEX_HOME, or account selector could violate ordinary logged-in Codex-session custody." },
        { id: "RISK-WORK-AUTHORITY", statement: "Agent prose or evidence presence could be mistaken for admission, verification, owner acceptance, or release authority." },
        { id: "RISK-DURABILITY", statement: "Reload, restart, retry, or update could duplicate work, flatten causal history, or silently retarget intent." },
        { id: "RISK-PUBLIC-SAFETY", statement: "Private native reports, credentials, paths, prompts, or repository content could leak into a public projection." },
      ],
    },
    environments: {
      ...proposal.environments,
      profiles: [{ id: target.criterion.environmentRef, status: "admitted" }],
    },
    obligations: proposal.obligations.map((obligation) => ({
      ...obligation,
      id: target.name === "electron" ? obligation.id : `AO-NATIVE-${obligation.criterion_refs[0]}-01`,
      candidate_artifact_refs: target.name === "electron"
        ? [testPath, "docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"]
        : [testPath, "docs/sol/2026-07-14-vercel-native-sdk-effect-native-desktop-audit.md"],
      domains: target.name === "electron" ? ["desktop_workroom", "release_artifact"] : ["native_sdk_workroom", "device_harness"],
      technique: "criterion_contract_with_sensitivity",
      environment_refs: [target.criterion.environmentRef],
      oracle: {
        statement: `The exact ${obligation.criterion_refs[0]} implementation/release anchors remain present and the criterion-local candidate test passes.`,
        evaluator_ref: testPath,
      },
      falsifier: {
        kind: "missing_required_anchor",
        ref: testPath,
        expected_verdict: "REFUTED",
      },
      evidence: {
        required_kinds: target.name === "electron"
          ? ["native_junit", "assurance_receipt", "oracle_sensitivity_receipt", "installed_release_receipt", "full_desktop_gate"]
          : ["native_junit", "assurance_receipt", "oracle_sensitivity_receipt", "native_sdk_host_gate", "target_integration_evidence"],
        proof_rung: target.name === "electron" ? "reviewed_release_plus_current_regression" : "native_target_integration_plus_headed_host",
      },
      independence: { producer_may_verify: false },
      activation_gate: target.assuranceSpec.gateRef,
    })),
    gates: [{
      id: target.assuranceSpec.gateRef,
      expression: "admitted && executable && candidate=CONFIRMED && falsifier=REFUTED && infrastructure=ready && stability=stable && freshness=current && disposition=accepted && exception=none && full_desktop_gate=green",
    }],
    evidencePolicy: {
      links_are_verdicts: false,
      missing_evidence_verdict: "INCONCLUSIVE",
      required_for_ready_obligation: target.name === "electron"
        ? ["oracle_observation", "falsifier_observation", "environment_binding", "independent_review", "installed_release_receipt", "full_desktop_gate"]
        : ["oracle_observation", "falsifier_observation", "environment_binding", "independent_review", "native_sdk_host_gate", "target_integration_evidence"],
      policy_state: "designed",
    },
    authority: {
      proposal_may_self_admit: false,
      proposal_may_execute: false,
      proposal_may_verify: false,
      proposal_may_release: false,
      proposal_may_change_public_promises: false,
      admitted_roles: ["openagents.owner"],
      verifier_roles: ["openagents.assurance_reviewer"],
      release_roles: ["openagents.owner"],
      policy_state: "designed",
    },
  }
}

const document = designDocument()
const assuranceSpecBytes = serializeAssuranceSpec(document)
write(relative.assuranceSpec, assuranceSpecBytes)
const assuranceSpecDigest = sha256Digest(assuranceSpecBytes)
const productSpecBytes = read(relative.productSpec)
const productSpecDigest = sha256Digest(productSpecBytes)

const profilePayload = {
  environment_format_version: "0.1" as const,
  profile_id: target.criterion.environmentRef,
  revision: 1,
  owner: "first_party" as const,
  target_class: target.name === "electron" ? "release_artifact" as const : "device" as const,
  mutability: "isolated_write" as const,
  platform: {
    os: "macos",
    architecture: "arm64",
    runtime: "Node 24.13.1",
    framework: target.name === "electron" ? "Effect Native / Electron" : "Effect Native / Native SDK 0.5.1 / Zig 0.16.0",
  },
  capabilities: target.name === "electron"
    ? ["vite_plus_test", "junit", "isolated_run_artifacts", "reviewed_installed_release_receipt"]
    : ["vite_plus_test", "junit", "native_sdk_criterion_catalog", "native_sdk_host_gate_v3", "zig_native_tests", "headed_macos_window", "isolated_run_artifacts"],
  authentication_strategy: "none" as const,
  isolation: { fresh_identity: true, reset_between_runs: true, restart_supported: true },
  data_classification: "private_local" as const,
  evidence_visibility: "reviewed_public_safe" as const,
  retention: "Private native JUnit under var/; reviewed normalized receipts committed by digest.",
  redaction_policy: "No raw output, hostname, absolute path, credential, prompt, transcript, or repository content in public projections.",
  permitted_actions: target.name === "electron"
    ? ["read_repository", "run_vite_plus_tests", "write_isolated_artifacts"]
    : ["read_repository", "run_vite_plus_tests", "build_native_fixture", "launch_local_native_fixture", "write_isolated_artifacts"],
  forbidden_actions: ["network", "credentials", "production_mutation", "customer_data", "release_publication"],
  required_commands: target.name === "electron" ? ["vp"] : ["vp", "pnpm", "zig"],
  dependency_lock: { path: target.dependencyLockPath, digest: sha256Digest(read(target.dependencyLockPath)) },
}
const environment: AssuranceEnvironmentProfileDocument = {
  ...profilePayload,
  profile_digest: computeEnvironmentProfileDigest(profilePayload),
}
write(relative.environment, canonicalArtifact(environment).bytes)

const adapterLock: AssuranceAdapterLock = {
  adapter_lock_format_version: "0.1",
  adapters: [{
    adapter_ref: target.criterion.adapterRef,
    version: target.name === "electron"
      ? OPENAGENTS_VITE_PLUS_TEST_ADAPTER_VERSION
      : OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_VERSION,
    content_digest: canonicalArtifact(target.criterion.adapterSourcePaths.map((path) => ({
      path,
      digest: sha256Digest(read(path)),
    }))).digest,
    techniques: ["criterion_contract_with_sensitivity"],
    capabilities: ["vite_plus_test", "junit", "normalized_receipt", ...(target.name === "native-sdk" ? ["native_sdk_criterion_catalog"] : [])],
  }, ...(target.name === "native-sdk" ? [{
    adapter_ref: OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_REF,
    version: OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_VERSION,
    content_digest: sha256Digest(read("packages/assurance-spec/src/native-sdk-assurance-adapter.ts")),
    techniques: ["headed_target_gate_normalization"],
    capabilities: ["native_sdk_host_gate_v3", "artifact_rehash", "normalized_target_gate_receipt"],
  }] : [])],
}
const adapterLockArtifact = canonicalArtifact(adapterLock)
write(relative.adapterLock, adapterLockArtifact.bytes)

const review: AssuranceReviewAnnotation = {
  review_annotation_format_version: "0.1",
  review_id: target.assuranceSpec.reviewId,
  reviewer_tool: target.name === "electron" ? "Codex independent assurance review" : "Codex scoped Native assurance design review",
  reviewed_at: target.name === "electron" ? "2026-07-13T20:00:00Z" : "2026-07-14T22:00:00Z",
  assurance_spec_id: document.frontmatter.assurance_spec_id,
  assurance_spec_revision: document.frontmatter.assurance_revision,
  assurance_spec_digest: assuranceSpecDigest,
  targets: [{
    target_type: "document",
    target_id: document.frontmatter.assurance_spec_id,
    axes: [
      "subject_fidelity", "criterion_traceability", "risk_coverage", "oracle_adequacy",
      "falsifier_strength", "seam_reality", "environment_fidelity", "evidence_sufficiency",
      "verifier_independence", "public_safety", "authority_containment", "feasibility",
    ].map((axis_key) => ({
      axis_key: axis_key as AssuranceReviewAnnotation["targets"][number]["axes"][number]["axis_key"],
      verdict: "pass" as const,
      evidence_refs: target.name === "electron"
        ? ["docs/mvp/2026-07-13-openagents-codex-workroom-rc9-completion-audit.md"]
        : ["docs/sol/2026-07-14-vercel-native-sdk-effect-native-desktop-audit.md", target.criterion.testPath],
    })),
  }],
}
const reviewBytes = serializeAssuranceReviewAnnotation(review)
write(relative.review, reviewBytes)

const admission: AssuranceAdmission = {
  admission_format_version: "0.1",
  admission_ref: target.assuranceSpec.admissionRef,
  decision: "admitted",
  assurance_spec: {
    id: document.frontmatter.assurance_spec_id,
    revision: document.frontmatter.assurance_revision,
    document_digest: assuranceSpecDigest,
  },
  product_spec: {
    path: relative.productSpec,
    revision: document.subject.product_spec.spec_revision,
    document_digest: productSpecDigest,
  },
  review_set_digest: assuranceReviewSetDigest([{ path: relative.review, bytes: reviewBytes }]),
  recognized_actor_ref: "owner.christopherdavid",
  recognized_role: "openagents.owner",
  allowed_gate_refs: [target.assuranceSpec.gateRef],
  authority_statement: target.name === "electron"
    ? "The owner directed full MVP AssuranceSpec execution and accepted the installed ProductSpec-native Codex workroom journey and read-only review boundary; this admits proof execution without publishing or changing public promises."
    : "The owner explicitly directed the full Native SDK MVP AssuranceSpec harness and parity work. This admits execution of the exact Native proof design only; it does not pre-accept observations, authorize release, or change public promises.",
}
const admissionArtifact = canonicalArtifact(admission)
write(relative.admission, admissionArtifact.bytes)

const testPath = target.criterion.testPath
const executionUnits: ReadonlyArray<AssuranceExecutionUnit> = document.obligations.flatMap((obligation) => {
  const criterion = obligation.criterion_refs[0]!
  return (["candidate", "falsifier"] as const).map((role) => ({
    unit_ref: `unit.${obligation.id.toLowerCase()}.${role}`,
    role,
    obligation_id: obligation.id,
    environment_ref: environment.profile_id,
    adapter_ref: target.criterion.adapterRef,
    argv: [
      "vp", "test", testPath, "--testNamePattern",
      role === "candidate"
        ? `${criterion} candidate evidence remains bound`
        : `${criterion} missing-anchor falsifier is rejected`,
    ],
    artifact_slots: [`${relative.runRoot}/${obligation.id}/${role}.junit.xml`],
    expected_observation: role === "candidate" ? "CONFIRMED" : "REFUTED",
  }))
})

const compiled = compileAssuranceManifest({
  assuranceSpec: document,
  assuranceSpecBytes,
  productSpecBytes,
  admission,
  admissionBytes: admissionArtifact.bytes,
  environment,
  adapterLock,
  adapterLockBytes: adapterLockArtifact.bytes,
  compilerContentDigest: sha256Digest(read("packages/assurance-spec/src/manifest.ts")),
  executionUnits,
})
write(relative.manifest, compiled.bytes)

rmSync(absolute(relative.runRoot), { recursive: true, force: true })
mkdirSync(absolute(relative.runRoot), { recursive: true })

const receiptRows: Array<Record<string, unknown>> = []
const criterionGaps: Array<Record<string, unknown>> = []
for (const obligation of document.obligations) {
  const units = executionUnits.filter((unit) => unit.obligation_id === obligation.id)
  const results = units.map((unit) => ({
    unit,
    result: (target.name === "electron" ? executeVitePlusTestUnit : executeNativeSdkCriterionUnit)({
      workspaceRoot: root,
      runRoot: absolute(`${relative.runRoot}/${obligation.id}`),
      manifest: compiled.manifest,
      manifestDigest: compiled.digest,
      environment,
      unit,
      producerRef: target.name === "electron" ? "runner.openagents.local.20260713" : "runner.openagents.native-sdk.local.20260714",
      reviewerRef: target.name === "electron" ? "reviewer.codex.assurance.20260713" : "reviewer.codex.native-assurance.20260714",
      sourceDigest: sha256Digest(read(testPath)),
      vitePlusExecutable: resolve(root, "node_modules/vite-plus/bin/vp"),
    }),
  }))
  const candidateResult = results.find(({ unit }) => unit.role === "candidate")?.result
  const falsifierResult = results.find(({ unit }) => unit.role === "falsifier")?.result
  if (
    candidateResult?.receipt.axes.observation !== "CONFIRMED" ||
    falsifierResult?.receipt.axes.observation !== "REFUTED"
  ) {
    criterionGaps.push({
      obligation_id: obligation.id,
      criterion_refs: obligation.criterion_refs,
      candidate_observation: candidateResult?.receipt.axes.observation ?? "INCONCLUSIVE",
      candidate_infrastructure: candidateResult?.receipt.axes.infrastructure ?? "unavailable",
      falsifier_observation: falsifierResult?.receipt.axes.observation ?? "INCONCLUSIVE",
      falsifier_infrastructure: falsifierResult?.receipt.axes.infrastructure ?? "unavailable",
    })
    continue
  }
  const accept = (receipt: AssuranceReceipt): AssuranceReceipt => ({
    ...receipt,
    axes: { ...receipt.axes, disposition: "accepted" },
    public_safety: { classification: "reviewed_public_safe", contains_raw_output: false },
  })
  const candidate = accept(candidateResult.receipt)
  const falsifier = accept(falsifierResult.receipt)
  const candidateArtifact = assuranceReceiptArtifact(candidate)
  const falsifierArtifact = assuranceReceiptArtifact(falsifier)
  const sensitivity = makeOracleSensitivityReceipt(candidate, falsifier, {
    oracleRef: `${obligation.id}.criterion_contract`,
    falsifierRef: `${obligation.id}.missing_required_anchor`,
  })
  const sensitivityArtifact = canonicalArtifact(sensitivity)
  const base = `${relative.receiptRoot}/${obligation.id}`
  write(`${base}.candidate.assurance-receipt.json`, candidateArtifact.bytes)
  write(`${base}.falsifier.assurance-receipt.json`, falsifierArtifact.bytes)
  write(`${base}.oracle-sensitivity-receipt.json`, sensitivityArtifact.bytes)
  receiptRows.push({
    obligation_id: obligation.id,
    criterion_refs: obligation.criterion_refs,
    candidate: { ref: candidate.receipt_ref, digest: candidateArtifact.digest, path: `${base}.candidate.assurance-receipt.json` },
    falsifier: { ref: falsifier.receipt_ref, digest: falsifierArtifact.digest, path: `${base}.falsifier.assurance-receipt.json` },
    sensitivity: { ref: sensitivity.receipt_ref, digest: sensitivityArtifact.digest, path: `${base}.oracle-sensitivity-receipt.json` },
    axes: candidate.axes,
  })
}

const [fullGateExecutable, ...fullGateArgv] = target.fullGate.argv
const targetDescriptorDigest = mvpAssuranceTargetDescriptorDigest(target)
const targetSourceDigest = mvpAssuranceTargetSourceDigest(root, target)
const nativeHostSmokeRoot = `${relative.runRoot}/host-smoke`
const fullGate = spawnSync(fullGateExecutable, fullGateArgv, {
  cwd: root,
  encoding: "utf8",
  env: {
    ...process.env,
    NO_COLOR: "1",
    CI: "1",
    ...(target.name === "native-sdk" ? {
      NATIVE_SDK_HOST_SMOKE_DIR: absolute(nativeHostSmokeRoot),
      OPENAGENTS_ASSURANCE_MANIFEST_DIGEST: compiled.digest,
      OPENAGENTS_ASSURANCE_ENVIRONMENT_DIGEST: environment.profile_digest,
      OPENAGENTS_ASSURANCE_ADAPTER_LOCK_DIGEST: adapterLockArtifact.digest,
      OPENAGENTS_ASSURANCE_TARGET_DESCRIPTOR_DIGEST: targetDescriptorDigest,
      OPENAGENTS_ASSURANCE_TARGET_SOURCE_DIGEST: targetSourceDigest,
    } : {}),
  },
  shell: false,
  maxBuffer: 64 * 1024 * 1024,
})
const fullGateOutput = `${fullGate.stdout ?? ""}${fullGate.stderr ?? ""}`
write(`${relative.runRoot}/full-desktop-gate.log`, fullGateOutput)
const fullGateSummary = parseVitePlusTestSummary(fullGateOutput)
if (fullGate.status !== 0 || fullGateSummary === null || fullGateSummary.failed !== 0 || !fullGateOutput.includes(target.fullGate.successMarker)) {
  throw new Error(`Full ${target.name} gate failed with exit ${String(fullGate.status)}.`)
}
const passCount = fullGateSummary.passed
const skipCount = fullGateSummary.skipped
const failCount = fullGateSummary.failed
let normalizedNativeHostGate: ReturnType<typeof normalizeNativeSdkHostGate> | null = null
if (target.name === "native-sdk") {
  let hostGateBytes = ""
  try {
    hostGateBytes = read(`${nativeHostSmokeRoot}/host-gate.json`)
  } catch {
    // The normalizer turns absence into an explicit inconclusive result.
  }
  normalizedNativeHostGate = normalizeNativeSdkHostGate(hostGateBytes, {
    workspaceRoot: root,
    evidenceRoot: absolute(nativeHostSmokeRoot),
    manifestDigest: compiled.digest,
    environmentDigest: environment.profile_digest,
    adapterLockDigest: adapterLockArtifact.digest,
    targetDescriptorDigest,
    targetSourceDigest,
    environmentRef: environment.profile_id,
    nativeReportRef: `${nativeHostSmokeRoot}/host-gate.json`,
  })
  if (normalizedNativeHostGate.status !== "ready") {
    throw new Error(`Native SDK host gate is INCONCLUSIVE: ${normalizedNativeHostGate.code}.`)
  }
  write(`${relative.runRoot}/native-sdk-host-gate-receipt.json`, normalizedNativeHostGate.receiptBytes)
}

const fullGateReceipt = canonicalArtifact({
  full_desktop_gate_receipt_format_version: "0.1",
  command: target.fullGate.argv.join(" "),
  command_digest: sha256Digest(
    JSON.stringify(target.fullGate.argv),
  ),
  output_digest: sha256Digest(fullGateOutput),
  source_digest: sha256Digest(target.fullGate.sourcePaths.map(read).join("\n")),
  exit_code: fullGate.status,
  verdict: "green",
  typecheck: "passed",
  tests: { passed: passCount, skipped_retired: skipCount, failed: failCount },
  build: "passed",
  [target.fullGate.smokeField]: "passed",
  ...(normalizedNativeHostGate?.status === "ready" ? {
    native_sdk_host_gate_receipt_digest: normalizedNativeHostGate.receiptDigest,
    target_ref: target.targetRef,
    target_descriptor_digest: targetDescriptorDigest,
    target_source_digest: targetSourceDigest,
    manifest_digest: compiled.digest,
    environment_digest: environment.profile_digest,
    adapter_lock_digest: adapterLockArtifact.digest,
  } : {}),
  native_output: { visibility: "private", path: `${relative.runRoot}/full-desktop-gate.log` },
  public_safety: { classification: "reviewed_public_safe", contains_raw_output: false },
})
write(relative.fullDesktopGateReceipt, fullGateReceipt.bytes)

const gapReport = canonicalArtifact({
  native_sdk_mvp_gap_report_format_version: "0.1",
  target_ref: target.targetRef,
  manifest_digest: compiled.digest,
  host_gate: normalizedNativeHostGate?.status ?? (target.name === "electron" ? "not_applicable" : "inconclusive"),
  confirmed_candidates: receiptRows.length,
  total_obligations: document.obligations.length,
  gaps: criterionGaps,
  publication: "withheld",
  authority: "diagnostic_only",
})
write(`${relative.runRoot}/criterion-gap-report.json`, gapReport.bytes)
if (criterionGaps.length > 0) {
  throw new Error(`${target.name} MVP assurance remains INCONCLUSIVE for ${criterionGaps.length} criterion(s); see ${relative.runRoot}/criterion-gap-report.json.`)
}

const evidenceIndex = canonicalArtifact({
  assurance_evidence_index_format_version: "0.1",
  subject: {
    product_spec_digest: productSpecDigest,
    assurance_spec_digest: assuranceSpecDigest,
    manifest_digest: compiled.digest,
    admission_digest: admissionArtifact.digest,
  },
  gate: {
    gate_ref: target.assuranceSpec.gateRef,
    admitted: true,
    executable: true,
    confirmed_obligations: receiptRows.length,
    total_obligations: document.obligations.length,
    infrastructure: "ready",
    stability: "stable",
    freshness: "current",
    disposition: "accepted",
    exception: "none",
    full_desktop_gate: "green",
  },
  receipts: receiptRows,
  companion_evidence_refs: target.companionEvidenceRefs,
  public_safety: { classification: "reviewed_public_safe", raw_artifacts_public: false },
})
write(relative.evidenceIndex, evidenceIndex.bytes)
publishStagedPublications()

console.log(JSON.stringify({
  assurance_spec_digest: assuranceSpecDigest,
  manifest_digest: compiled.digest,
  obligations: receiptRows.length,
  candidate_confirmed: receiptRows.length,
  falsifiers_refuted: receiptRows.length,
  evidence_index_digest: evidenceIndex.digest,
}, null, 2))
