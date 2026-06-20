import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"

// AO-3 (EPIC #5441, issue #5444): first-run identity choice.
//
// On the very first launch — BEFORE the node mints a fresh identity — detect
// whether the user already has a Pylon identity and let them choose:
//   - "Use your existing Pylon identity"  → boot that home so the existing
//     wallet / payout target / history carry over (no fork).
//   - "Create a new Autopilot identity"   → user names it; mint a fresh
//     PYLON_HOME + identity under that name and run the from-scratch chain.
//
// Hard rule (the v1.0.3 "Orwell" lesson, see apps/pylon/src/bootstrap.ts): NEVER
// silently overwrite or adopt the wrong home. We DETECT (marker-presence only —
// never read the seed), then ASK, then act only on the CHOSEN home. Create-new
// is always available even when an existing Pylon is detected (multi-identity +
// a demoable from-scratch flow on a machine that already has a Pylon). The
// user's choice is persisted so it is not asked again.
//
// Detection mirrors Pylon v1.0.3's `selectPylonHomeResolution`
// (apps/pylon/src/bootstrap.ts): the authoritative marker a home must hold is
// the NIP-06 seed file `identity.mnemonic`. We test the file's PRESENCE only and
// never read or print the seed. `identity.json` is the public projection we read
// (npub / pylonRef / nodeLabel) purely to show the user which identity they have.
//
// The desktop deliberately does not depend on the `@openagentsinc/pylon` package
// (a packaged `.app` has no repo to import from; the build bundles the node entry
// by relative path). So this is a desktop-owned, faithful reimplementation of the
// same marker-presence contract, kept honest by the same homes + source labels.

// The authoritative seed marker (matches bootstrap.ts `HOME_SEED_MARKER`). We
// only test for its presence; we never read or print the seed.
const HOME_SEED_MARKER = "identity.mnemonic"
// The public identity projection a Pylon home writes (apps/pylon/src/state.ts).
const IDENTITY_FILENAME = "identity.json"

// Public-safe label describing WHY a home was selected (never the seed). Mirrors
// bootstrap.ts `PylonHomeResolutionSource`.
export type DetectedHomeSource =
  | "discovered_openagents_pylon"
  | "discovered_dot_pylon"

// A detected existing seed-bearing Pylon home + its public identity projection.
export type DetectedPylonIdentity = {
  readonly home: string
  readonly source: DetectedHomeSource
  // Public-safe identity fields read from identity.json (never the seed). Any
  // may be null if identity.json is absent/partial even though the seed exists.
  readonly npub: string | null
  readonly pylonRef: string | null
  readonly nodeLabel: string | null
}

export type ReadFile = (path: string) => string | null
export type FileExists = (path: string) => boolean
export type WriteFile = (path: string, content: string) => void
export type EnsureDir = (path: string) => void

const defaultReadFile: ReadFile = path => {
  try {
    if (!existsSync(path)) return null
    return readFileSync(path, "utf8")
  } catch {
    return null
  }
}

const defaultFileExists: FileExists = path => {
  try {
    return existsSync(path)
  } catch {
    return false
  }
}

const defaultWriteFile: WriteFile = (path, content) => {
  writeFileSync(path, content, { mode: 0o600 })
}

const defaultEnsureDir: EnsureDir = path => {
  mkdirSync(path, { recursive: true })
}

const asTrimmedString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

// --- detection --------------------------------------------------------------

// The candidate homes, in the same priority order bootstrap.ts uses:
// `~/.openagents/pylon` (the historical-config identity home a live node uses)
// wins ties over a bare `~/.pylon`.
const candidateHomes = (
  home: string,
): ReadonlyArray<{ readonly home: string; readonly source: DetectedHomeSource }> => [
  { home: join(home, ".openagents", "pylon"), source: "discovered_openagents_pylon" },
  { home: join(home, ".pylon"), source: "discovered_dot_pylon" },
]

const homeHasSeed = (home: string, fileExists: FileExists): boolean =>
  fileExists(join(home, HOME_SEED_MARKER))

// Read the public identity projection of a home (never the seed). Returns null
// fields when identity.json is missing/malformed — the seed presence still makes
// the home authoritative; the missing projection just means we cannot show the
// npub yet.
const readPublicIdentity = (
  home: string,
  readFile: ReadFile,
): { npub: string | null; pylonRef: string | null; nodeLabel: string | null } => {
  const raw = readFile(join(home, IDENTITY_FILENAME))
  if (raw === null) return { npub: null, pylonRef: null, nodeLabel: null }
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return { npub: null, pylonRef: null, nodeLabel: null }
  }
  if (typeof parsed !== "object" || parsed === null) {
    return { npub: null, pylonRef: null, nodeLabel: null }
  }
  const record = parsed as Record<string, unknown>
  return {
    npub: asTrimmedString(record.npub),
    pylonRef: asTrimmedString(record.pylonRef),
    nodeLabel: asTrimmedString(record.nodeLabel),
  }
}

export type DetectExistingIdentityOptions = {
  // The user's home directory (defaults to os.homedir()). Injectable for tests.
  readonly homeDir?: string
  readonly readFile?: ReadFile
  readonly fileExists?: FileExists
}

/**
 * AO-3: detect a single existing seed-bearing Pylon identity, preferring
 * `~/.openagents/pylon` over `~/.pylon` (same order as bootstrap.ts). Returns
 * null when no seed-bearing home exists (a fresh machine). Marker-presence only:
 * we never read or print the seed. Never throws.
 */
export const detectExistingPylonIdentity = (
  options: DetectExistingIdentityOptions = {},
): DetectedPylonIdentity | null => {
  const home = options.homeDir ?? homedir()
  const readFile = options.readFile ?? defaultReadFile
  const fileExists = options.fileExists ?? defaultFileExists

  for (const candidate of candidateHomes(home)) {
    if (!homeHasSeed(candidate.home, fileExists)) continue
    const identity = readPublicIdentity(candidate.home, readFile)
    return {
      home: candidate.home,
      source: candidate.source,
      npub: identity.npub,
      pylonRef: identity.pylonRef,
      nodeLabel: identity.nodeLabel,
    }
  }
  return null
}

// A public-safe short label for a detected identity, e.g. `pylon.ab12cd` or an
// abbreviated npub. Never the seed. Used by the choice screen so the user can
// recognize their identity.
export const detectedIdentityShortLabel = (
  identity: DetectedPylonIdentity,
): string => {
  if (identity.pylonRef !== null) {
    const tail = identity.pylonRef.replace(/^pylon\./i, "").slice(0, 8)
    return `pylon.${tail}`
  }
  if (identity.npub !== null) {
    return `${identity.npub.slice(0, 12)}…`
  }
  if (identity.nodeLabel !== null) return identity.nodeLabel
  return "existing Pylon"
}

// --- persisted choice -------------------------------------------------------

// The first-run choice the user made. `use_existing` boots the detected
// seed-bearing home; `create_new` mints a fresh managed home for a named
// identity (the default on a fresh machine, and always available besides).
export type IdentityChoiceKind = "use_existing" | "create_new"

export type PersistedIdentityChoice = {
  readonly kind: IdentityChoiceKind
  // For `use_existing`: the chosen seed-bearing home to boot against.
  // For `create_new`: null (the launcher mints/uses its own managed home).
  readonly home: string | null
  // For `create_new`: the display name the user chose. Flows into AO-1
  // (selfRegisterAgent.displayName). Null for `use_existing`.
  readonly displayName: string | null
  readonly chosenAt: string
}

// Where the first-run choice is persisted. Lives under the desktop's managed
// parent dir (NOT inside any Pylon seed home), so recording the choice never
// touches a node's seed-bearing home. 0600, neutral metadata, no secrets.
const CHOICE_PARENT = join(".openagents", "autopilot-desktop")
const CHOICE_FILENAME = "identity-choice.json"

const choiceParentDir = (homeDir: string): string =>
  join(homeDir, CHOICE_PARENT)

const choicePath = (homeDir: string): string =>
  join(choiceParentDir(homeDir), CHOICE_FILENAME)

export type LoadIdentityChoiceOptions = {
  readonly homeDir?: string
  readonly readFile?: ReadFile
}

/**
 * Load the persisted first-run identity choice. Returns null when none exists
 * (the choice has not been made yet → the wizard should ask). Never throws.
 */
export const loadIdentityChoice = (
  options: LoadIdentityChoiceOptions = {},
): PersistedIdentityChoice | null => {
  const homeDir = options.homeDir ?? homedir()
  const readFile = options.readFile ?? defaultReadFile
  const raw = readFile(choicePath(homeDir))
  if (raw === null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const record = parsed as Record<string, unknown>
  const kind = record.kind
  if (kind !== "use_existing" && kind !== "create_new") return null
  return {
    kind,
    home: asTrimmedString(record.home),
    displayName: asTrimmedString(record.displayName),
    chosenAt: asTrimmedString(record.chosenAt) ?? new Date().toISOString(),
  }
}

export type SaveIdentityChoiceInput = {
  readonly kind: IdentityChoiceKind
  // Required for `use_existing`: the detected seed-bearing home to boot.
  readonly home?: string | null
  // Required for `create_new`: the user-chosen display name.
  readonly displayName?: string | null
}

export type SaveIdentityChoiceOptions = {
  readonly homeDir?: string
  readonly writeFile?: WriteFile
  readonly ensureDir?: EnsureDir
  readonly fileExists?: FileExists
}

export type SaveIdentityChoiceResult =
  | { readonly ok: true; readonly choice: PersistedIdentityChoice }
  | { readonly ok: false; readonly reason: string }

/**
 * AO-3: persist the user's first-run identity choice so it is not asked again.
 *
 * Hard rule (the v1.0.3/Orwell lesson): `use_existing` is only ever recorded
 * against a home that ACTUALLY holds the seed marker — we re-verify presence
 * here so a stale/wrong home can never be adopted. We NEVER write into a
 * seed-bearing Pylon home (the choice file lives under the desktop's own managed
 * parent), so recording a choice can never overwrite an existing identity.
 * Never throws — a write failure returns an honest `{ ok: false }`.
 */
export const saveIdentityChoice = (
  input: SaveIdentityChoiceInput,
  options: SaveIdentityChoiceOptions = {},
): SaveIdentityChoiceResult => {
  const homeDir = options.homeDir ?? homedir()
  const writeFile = options.writeFile ?? defaultWriteFile
  const ensureDir = options.ensureDir ?? defaultEnsureDir
  const fileExists = options.fileExists ?? defaultFileExists

  let home: string | null = null
  let displayName: string | null = null

  if (input.kind === "use_existing") {
    home = asTrimmedString(input.home)
    if (home === null) {
      return { ok: false, reason: "use_existing requires a detected home" }
    }
    // Re-verify the seed marker: never adopt a home that does not actually hold
    // an identity. This is the guardrail against adopting the wrong home.
    if (!homeHasSeed(home, fileExists)) {
      return { ok: false, reason: "chosen home has no identity seed" }
    }
  } else {
    // create_new: the display name is optional here (the user may name it later
    // / the launcher falls back to the neutral auto name), but we record it when
    // present so it flows into AO-1 registration.
    displayName = asTrimmedString(input.displayName)
  }

  const choice: PersistedIdentityChoice = {
    kind: input.kind,
    home,
    displayName,
    chosenAt: new Date().toISOString(),
  }

  try {
    ensureDir(choiceParentDir(homeDir))
    writeFile(
      choicePath(homeDir),
      `${JSON.stringify(choice, null, 2)}\n`,
    )
    return { ok: true, choice }
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    return { ok: false, reason }
  }
}

// --- resolved first-run identity state --------------------------------------

// The public-safe projection the webview consumes to render the choice screen
// and to know whether a choice is still needed. No seeds, no tokens.
export type IdentityChoiceState = {
  // Whether the first screen of the wizard must still ask. False once a choice
  // is persisted (then the wizard moves on to the live status steps).
  readonly choiceNeeded: boolean
  // The detected existing identity (null on a fresh machine). Public-safe.
  readonly detected: {
    readonly present: boolean
    readonly shortLabel: string | null
    readonly npub: string | null
    readonly pylonRef: string | null
    readonly source: DetectedHomeSource | null
  }
  // The persisted choice, if one was already made (public-safe; no displayName
  // is surfaced as a secret — it is user-entered, not sensitive).
  readonly chosen: {
    readonly kind: IdentityChoiceKind
    readonly displayName: string | null
  } | null
  // Create-new is ALWAYS offered (even when an existing Pylon is detected).
  readonly createNewAvailable: true
}

export type ProjectIdentityChoiceOptions = DetectExistingIdentityOptions &
  LoadIdentityChoiceOptions

/**
 * AO-3: project the first-run identity-choice state for the webview. Combines
 * live detection with the persisted choice. Pure read; never mutates anything.
 */
export const projectIdentityChoiceState = (
  options: ProjectIdentityChoiceOptions = {},
): IdentityChoiceState => {
  const detected = detectExistingPylonIdentity(options)
  const chosen = loadIdentityChoice(options)
  return {
    choiceNeeded: chosen === null,
    detected: {
      present: detected !== null,
      shortLabel: detected ? detectedIdentityShortLabel(detected) : null,
      npub: detected?.npub ?? null,
      pylonRef: detected?.pylonRef ?? null,
      source: detected?.source ?? null,
    },
    chosen:
      chosen === null
        ? null
        : { kind: chosen.kind, displayName: chosen.displayName },
    createNewAvailable: true,
  }
}
