import type {
  AutopilotWorkBrowserDesktopIntegration,
  AutopilotWorkBrowserDesktopIntegrationEntry,
  AutopilotWorkBrowserDesktopIntegrationFreshness,
  AutopilotWorkBrowserDesktopIntegrationState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeBrowserDesktopIntegrationStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeBrowserDesktopIntegrationAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  browserAutomationAuthority: false
  deepLinkOpenAuthority: false
  deploymentAuthority: false
  desktopControlAuthority: false
  extensionInstallAuthority: false
  fileReadAuthority: false
  notificationSendAuthority: false
  permissionInspectAuthority: false
  publicClaimAuthority: false
  sessionInspectAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolExecutionAuthority: false
  toolRoutingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeBrowserDesktopIntegrationItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  browserRefs: ReadonlyArray<string>
  companionRefs: ReadonlyArray<string>
  deepLinkRefs: ReadonlyArray<string>
  desktopAppRefs: ReadonlyArray<string>
  extensionRefs: ReadonlyArray<string>
  freshness: AutopilotWorkBrowserDesktopIntegrationFreshness
  installRefs: ReadonlyArray<string>
  integrationRef: string
  notificationRefs: ReadonlyArray<string>
  permissionRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  state: AutopilotWorkBrowserDesktopIntegrationState
  statusRefs: ReadonlyArray<string>
  surfaceRefs: ReadonlyArray<string>
  updateRefs: ReadonlyArray<string>
}>

export type ForgeBrowserDesktopIntegrationInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkBrowserDesktopIntegrationEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeBrowserDesktopIntegrationCounts = Readonly<{
  blocked: number
  connected: number
  installed: number
  ready: number
  total: number
  unavailable: number
}>

export type ForgeBrowserDesktopIntegrationView = Readonly<{
  authority: ForgeBrowserDesktopIntegrationAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeBrowserDesktopIntegrationCounts
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeBrowserDesktopIntegrationStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_BROWSER_DESKTOP_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:app|browser|command|content|cookie|desktop|extension|file|input|log|notification|output|path|permission|profile|session|shell|source|tab|trace|transcript|window)/i,
  /private[-_ ](?:app|browser|content|cookie|desktop|extension|file|path|profile|session|source|tab|window|workspace)/i,
  /browser[-_ ](?:body|cookie|payload|profile|session|tab|text)/i,
  /desktop[-_ ](?:app[-_ ]?path|body|payload|window)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)[A-Za-z][A-Za-z0-9+.-]*:\/\//,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeBrowserDesktopIntegrationAuthority = {
  acceptedOutcomeAuthority: false,
  browserAutomationAuthority: false,
  deepLinkOpenAuthority: false,
  deploymentAuthority: false,
  desktopControlAuthority: false,
  extensionInstallAuthority: false,
  fileReadAuthority: false,
  notificationSendAuthority: false,
  permissionInspectAuthority: false,
  publicClaimAuthority: false,
  sessionInspectAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolExecutionAuthority: false,
  toolRoutingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_BROWSER_DESKTOP_MARKERS.some(marker => marker.test(trimmed))
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

const safeOptionalRef = (
  value: string | null | undefined,
): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-browser-desktop-integration-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkBrowserDesktopIntegrationEntry,
): Readonly<{
  entry: ForgeBrowserDesktopIntegrationItem | null
  omittedUnsafeRefCount: number
}> => {
  const integrationRef = safeOptionalRef(entry.integrationRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const browserRefs = safeRefs(entry.browserRefs)
  const companionRefs = safeRefs(entry.companionRefs)
  const deepLinkRefs = safeRefs(entry.deepLinkRefs)
  const desktopAppRefs = safeRefs(entry.desktopAppRefs)
  const extensionRefs = safeRefs(entry.extensionRefs)
  const installRefs = safeRefs(entry.installRefs)
  const notificationRefs = safeRefs(entry.notificationRefs)
  const permissionRefs = safeRefs(entry.permissionRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const statusRefs = safeRefs(entry.statusRefs)
  const surfaceRefs = safeRefs(entry.surfaceRefs)
  const updateRefs = safeRefs(entry.updateRefs)
  const omittedUnsafeRefCount =
    integrationRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    browserRefs.omittedUnsafeRefCount +
    companionRefs.omittedUnsafeRefCount +
    deepLinkRefs.omittedUnsafeRefCount +
    desktopAppRefs.omittedUnsafeRefCount +
    extensionRefs.omittedUnsafeRefCount +
    installRefs.omittedUnsafeRefCount +
    notificationRefs.omittedUnsafeRefCount +
    permissionRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    statusRefs.omittedUnsafeRefCount +
    surfaceRefs.omittedUnsafeRefCount +
    updateRefs.omittedUnsafeRefCount

  return integrationRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          browserRefs: browserRefs.refs,
          companionRefs: companionRefs.refs,
          deepLinkRefs: deepLinkRefs.refs,
          desktopAppRefs: desktopAppRefs.refs,
          extensionRefs: extensionRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          installRefs: installRefs.refs,
          integrationRef: integrationRef.ref,
          notificationRefs: notificationRefs.refs,
          permissionRefs: permissionRefs.refs,
          policyRefs: policyRefs.refs,
          state: entry.state,
          statusRefs: statusRefs.refs,
          surfaceRefs: surfaceRefs.refs,
          updateRefs: updateRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ForgeBrowserDesktopIntegrationCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  connected: entries.filter(entry => entry.state === 'connected').length,
  installed: entries.filter(entry => entry.state === 'installed').length,
  ready: entries.filter(entry => entry.state === 'ready').length,
  total: entries.length,
  unavailable: entries.filter(entry => entry.state === 'unavailable').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry =>
      blockerRef(
        workOrderRef,
        `stale-browser-desktop-evidence:${entry.integrationRef}`,
      ),
    )

const readinessBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'ready' || entry.state === 'connected') &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `browser-desktop-policy-ref-missing:${entry.integrationRef}`),
    )

const deepLinkBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.deepLinkRefs.length > 0 &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `deep-link-policy-ref-missing:${entry.integrationRef}`))

const notificationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.notificationRefs.length > 0 &&
        entry.permissionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `notification-permission-ref-missing:${entry.integrationRef}`),
    )

const installBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'installed' &&
        entry.installRefs.length === 0 &&
        entry.updateRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `install-update-ref-missing:${entry.integrationRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeBrowserDesktopIntegrationItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeBrowserDesktopIntegrationStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (
    entries.every(
      entry =>
        entry.state === 'ready' ||
        entry.state === 'connected' ||
        entry.state === 'installed',
    )
  ) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'unavailable') ? 'warning' : 'unknown'
}

export const projectForgeBrowserDesktopIntegration = (
  input: ForgeBrowserDesktopIntegrationInput,
): ForgeBrowserDesktopIntegrationView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.freshness.localeCompare(right.freshness) ||
        left.integrationRef.localeCompare(right.integrationRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...readinessBlockers(input.workOrderRef, entries),
      ...deepLinkBlockers(input.workOrderRef, entries),
      ...notificationBlockers(input.workOrderRef, entries),
      ...installBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-browser-desktop-integration-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [
            blockerRef(
              input.workOrderRef,
              'unsafe-browser-desktop-integration-material-omitted',
            ),
          ]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeBrowserDesktopIntegrationInput = (
  work: AutopilotWorkProjection,
): ForgeBrowserDesktopIntegrationInput => {
  const source: AutopilotWorkBrowserDesktopIntegration | undefined =
    work.browserDesktopIntegration

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
