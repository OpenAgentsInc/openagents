// Forge cockpit surface for terminal-agent system #27
// (Help, Doctor, And Debug Surfaces).
//
// This is a refs-only read projection. It folds the runtime's help,
// doctor, preflight, support-bundle, and diagnostic-log evidence into a
// single cockpit lane WITHOUT acquiring any runtime authority. The lane
// cannot run doctor checks, execute preflight probes, build or upload
// support bundles, grant submission consent, mutate settings, or read
// secrets/credentials. It only surfaces public-safe refs and an explicit
// export-readiness verdict that stays gated on user consent.
//
// Redaction follows the established session-navigation pattern: every ref
// must match a conservative safe-ref shape AND avoid private-material
// markers (secret-like tokens, raw diffs, raw prompts, absolute/relative
// filesystem paths, URLs, shell metacharacters). Anything that fails is
// dropped and counted in `omittedUnsafeRefCount`, never rendered.

export type ForgeDoctorSeverity = 'error' | 'info' | 'ok' | 'warning'

export type ForgeDoctorCategory =
  | 'extension'
  | 'install'
  | 'integration'
  | 'keybinding'
  | 'network'
  | 'sandbox'
  | 'search'
  | 'settings'
  | 'shell'
  | 'update'

export type ForgeSupportDiagnosticsStatus =
  | 'attention'
  | 'empty'
  | 'failing'
  | 'ready'

export type ForgeSupportBundleConsent = 'consented' | 'declined' | 'pending'

export type ForgeSupportExportReadiness =
  | 'blocked'
  | 'consent_required'
  | 'ready'

export type ForgeDoctorCheckInput = Readonly<{
  category?: ForgeDoctorCategory
  checkRef: string
  evidenceRefs?: ReadonlyArray<string>
  fixRefs?: ReadonlyArray<string>
  severity?: ForgeDoctorSeverity
}>

export type ForgeSupportBundleSectionInput = Readonly<{
  consent?: ForgeSupportBundleConsent
  evidenceRefs?: ReadonlyArray<string>
  sectionRef: string
}>

export type ForgeSupportDiagnosticsInput = Readonly<{
  diagnosticLogRefs?: ReadonlyArray<string>
  doctorChecks?: ReadonlyArray<ForgeDoctorCheckInput>
  generatedAt: string
  helpCommandRefs?: ReadonlyArray<string>
  preflightRefs?: ReadonlyArray<string>
  supportBundleSections?: ReadonlyArray<ForgeSupportBundleSectionInput>
  workOrderRef: string
}>

export type ForgeDoctorCheckItem = Readonly<{
  category: ForgeDoctorCategory
  checkRef: string
  evidenceRefs: ReadonlyArray<string>
  fixRefs: ReadonlyArray<string>
  severity: ForgeDoctorSeverity
}>

export type ForgeSupportBundleSectionItem = Readonly<{
  consent: ForgeSupportBundleConsent
  evidenceRefs: ReadonlyArray<string>
  sectionRef: string
}>

export type ForgeSupportDiagnosticsCounts = Readonly<{
  error: number
  info: number
  ok: number
  warning: number
}>

export type ForgeSupportDiagnosticsView = Readonly<{
  authority: Readonly<{
    bundleExportAuthority: false
    consentGrantAuthority: false
    credentialReadAuthority: false
    doctorExecutionAuthority: false
    preflightExecutionAuthority: false
    settingsMutationAuthority: false
  }>
  blockerRefs: ReadonlyArray<string>
  counts: ForgeSupportDiagnosticsCounts
  diagnosticLogRefs: ReadonlyArray<string>
  doctorChecks: ReadonlyArray<ForgeDoctorCheckItem>
  exportReadiness: ForgeSupportExportReadiness
  generatedAt: string
  helpCommandRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  preflightRefs: ReadonlyArray<string>
  status: ForgeSupportDiagnosticsStatus
  supportBundleSections: ReadonlyArray<ForgeSupportBundleSectionItem>
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const DOCTOR_SEVERITIES: ReadonlyArray<ForgeDoctorSeverity> = [
  'error',
  'info',
  'ok',
  'warning',
]
const DOCTOR_CATEGORIES: ReadonlyArray<ForgeDoctorCategory> = [
  'extension',
  'install',
  'integration',
  'keybinding',
  'network',
  'sandbox',
  'search',
  'settings',
  'shell',
  'update',
]
const SUPPORT_BUNDLE_CONSENTS: ReadonlyArray<ForgeSupportBundleConsent> = [
  'consented',
  'declined',
  'pending',
]

// Conservative shape for a public-safe ref. Mirrors the session-navigation
// lane so the cockpit redaction posture stays consistent across surfaces.
const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_MATERIAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|transcript)/i,
  /private[-_ ](?:repo|content|source|transcript|path)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /authorization[-_ ]header/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk|AKIA|xox[baprs])-?[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice|password|passwd|api[-_]?key|credential)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MATERIAL_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const blockerRef = (scopeRef: string, suffix: string): string =>
  `forge-support-diagnostics-blocker:${scopeRef}:${suffix}`

const emptyCounts = (): {
  error: number
  info: number
  ok: number
  warning: number
} => ({
  error: 0,
  info: 0,
  ok: 0,
  warning: 0,
})

const normalizeDoctorChecks = (
  checks: ReadonlyArray<ForgeDoctorCheckInput> | undefined,
): Readonly<{
  items: ReadonlyArray<ForgeDoctorCheckItem>
  omitted: number
}> =>
  (checks ?? []).reduce<{
    items: ReadonlyArray<ForgeDoctorCheckItem>
    omitted: number
  }>(
    (state, check) => {
      const checkRef = safeRef(check.checkRef)
      const evidenceRefs = safeRefs(check.evidenceRefs)
      const fixRefs = safeRefs(check.fixRefs)
      const omitted =
        evidenceRefs.omittedUnsafeRefCount +
        fixRefs.omittedUnsafeRefCount +
        (checkRef === null ? 1 : 0)

      if (checkRef === null) {
        return { items: state.items, omitted: state.omitted + omitted }
      }

      return {
        items: [
          ...state.items,
          {
            category: check.category ?? 'install',
            checkRef,
            evidenceRefs: evidenceRefs.refs,
            fixRefs: fixRefs.refs,
            severity: check.severity ?? 'info',
          },
        ],
        omitted: state.omitted + omitted,
      }
    },
    { items: [], omitted: 0 },
  )

const normalizeBundleSections = (
  sections: ReadonlyArray<ForgeSupportBundleSectionInput> | undefined,
): Readonly<{
  items: ReadonlyArray<ForgeSupportBundleSectionItem>
  omitted: number
}> =>
  (sections ?? []).reduce<{
    items: ReadonlyArray<ForgeSupportBundleSectionItem>
    omitted: number
  }>(
    (state, section) => {
      const sectionRef = safeRef(section.sectionRef)
      const evidenceRefs = safeRefs(section.evidenceRefs)
      const omitted =
        evidenceRefs.omittedUnsafeRefCount + (sectionRef === null ? 1 : 0)

      if (sectionRef === null) {
        return { items: state.items, omitted: state.omitted + omitted }
      }

      return {
        items: [
          ...state.items,
          {
            consent: section.consent ?? 'pending',
            evidenceRefs: evidenceRefs.refs,
            sectionRef,
          },
        ],
        omitted: state.omitted + omitted,
      }
    },
    { items: [], omitted: 0 },
  )

const severityRank = (severity: ForgeDoctorSeverity): number =>
  severity === 'error' ? 0 : severity === 'warning' ? 1 : severity === 'info' ? 2 : 3

const statusForChecks = (
  checks: ReadonlyArray<ForgeDoctorCheckItem>,
  hasAnyEvidence: boolean,
): ForgeSupportDiagnosticsStatus => {
  if (!hasAnyEvidence) {
    return 'empty'
  }

  if (checks.some(check => check.severity === 'error')) {
    return 'failing'
  }

  if (checks.some(check => check.severity === 'warning')) {
    return 'attention'
  }

  return 'ready'
}

// Export readiness is consent-gated and refs-only. It NEVER means the
// projection can export anything: it only reports whether the runtime,
// given user consent, would be allowed to. A failing doctor or omitted
// unsafe material blocks readiness so we never advertise an exportable
// bundle that we could not prove safe.
const exportReadinessFor = (
  status: ForgeSupportDiagnosticsStatus,
  sections: ReadonlyArray<ForgeSupportBundleSectionItem>,
  hasBlockers: boolean,
): ForgeSupportExportReadiness => {
  if (status === 'failing' || hasBlockers || sections.length === 0) {
    return 'blocked'
  }

  return sections.every(section => section.consent === 'consented')
    ? 'ready'
    : 'consent_required'
}

export const projectForgeSupportDiagnostics = (
  input: ForgeSupportDiagnosticsInput,
): ForgeSupportDiagnosticsView => {
  const doctor = normalizeDoctorChecks(input.doctorChecks)
  const bundleSections = normalizeBundleSections(input.supportBundleSections)
  const helpCommandRefs = safeRefs(input.helpCommandRefs)
  const preflightRefs = safeRefs(input.preflightRefs)
  const diagnosticLogRefs = safeRefs(input.diagnosticLogRefs)

  const doctorChecks = Array.from(doctor.items).sort(
    (left, right) =>
      severityRank(left.severity) - severityRank(right.severity) ||
      left.checkRef.localeCompare(right.checkRef),
  )

  const counts = doctorChecks.reduce<{
    error: number
    info: number
    ok: number
    warning: number
  }>((state, check) => {
    state[check.severity] += 1

    return state
  }, emptyCounts())
  DOCTOR_SEVERITIES.forEach(severity => {
    counts[severity] = counts[severity] ?? 0
  })

  const hasAnyEvidence =
    doctorChecks.length > 0 ||
    bundleSections.items.length > 0 ||
    helpCommandRefs.refs.length > 0 ||
    preflightRefs.refs.length > 0 ||
    diagnosticLogRefs.refs.length > 0

  const omittedUnsafeRefCount =
    doctor.omitted +
    bundleSections.omitted +
    helpCommandRefs.omittedUnsafeRefCount +
    preflightRefs.omittedUnsafeRefCount +
    diagnosticLogRefs.omittedUnsafeRefCount

  const status = statusForChecks(doctorChecks, hasAnyEvidence)
  const blockerRefs = safeRefs([
    ...(hasAnyEvidence
      ? []
      : [blockerRef(input.workOrderRef, 'no-support-diagnostics-evidence')]),
    ...doctorChecks
      .filter(check => check.severity === 'error')
      .map(check => blockerRef(input.workOrderRef, `doctor-error:${check.checkRef}`)),
    ...(omittedUnsafeRefCount > 0
      ? [blockerRef(input.workOrderRef, 'unsafe-support-material-omitted')]
      : []),
  ]).refs

  const exportReadiness = exportReadinessFor(
    status,
    bundleSections.items,
    blockerRefs.length > 0,
  )

  return {
    authority: {
      bundleExportAuthority: false,
      consentGrantAuthority: false,
      credentialReadAuthority: false,
      doctorExecutionAuthority: false,
      preflightExecutionAuthority: false,
      settingsMutationAuthority: false,
    },
    blockerRefs,
    counts,
    diagnosticLogRefs: diagnosticLogRefs.refs,
    doctorChecks,
    exportReadiness,
    generatedAt: input.generatedAt,
    helpCommandRefs: helpCommandRefs.refs,
    omittedUnsafeRefCount,
    preflightRefs: preflightRefs.refs,
    status,
    supportBundleSections: Array.from(bundleSections.items).sort((left, right) =>
      left.sectionRef.localeCompare(right.sectionRef),
    ),
    workOrderRef: input.workOrderRef,
  }
}

// Re-exported constants so the cockpit view and tests can stay in lockstep
// with the projection's closed sets without re-declaring them.
export const FORGE_DOCTOR_SEVERITIES = DOCTOR_SEVERITIES
export const FORGE_DOCTOR_CATEGORIES = DOCTOR_CATEGORIES
export const FORGE_SUPPORT_BUNDLE_CONSENTS = SUPPORT_BUNDLE_CONSENTS
