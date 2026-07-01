/**
 * Blueprint Signature 4 — `command-execution-source-verified`
 *
 * No command is recommended without reading its source.
 *
 * Pure, ordered-predicate state machine implementing the gate from
 * `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`:
 *
 *   UNVERIFIED → SOURCE_READ → FLAGS_VERIFIED → RUNTIME_CONFIRMED → SAFE_TO_PROPOSE
 *
 * Only the terminal state `SAFE_TO_PROPOSE` unlocks proposing the command.
 *
 * Ordered predicates:
 *  1. source-read    — the file contents were read + hashed (a content hash is
 *                      present) before proposing.
 *  2. flag-verification — every flag in the proposed command exists in the
 *                      script's actual parsed argument surface (`declaredFlags`).
 *                      A stub with zero declared flags fails here, so the
 *                      "fabricated executable" mistake (a command recommended
 *                      against a script that does not accept those flags) is
 *                      structurally impossible.
 *  3. runtime-check  — a dry-run / `--help` probe exited 0, proving the script is
 *                      executable and accepts the expected flags.
 *
 * SINGLE AUTHORITY: this is the one home for the S4 evaluator. The Pylon gate
 * module (`apps/pylon/src/blueprint-gates/command-execution-source-verified.ts`)
 * and the openagents.com Worker operator loop both import + apply THIS function
 * rather than re-describing it, so the two consumers can never drift. This
 * package is `@openagentsinc/blueprint-contracts` — a dependency-light,
 * cross-consumer Blueprint contract home (it already hosts the canonical
 * private-data-safety predicate family). Both consumers already depend on it.
 */

export const COMMAND_SOURCE_VERIFIED_STATES = [
  "UNVERIFIED",
  "SOURCE_READ",
  "FLAGS_VERIFIED",
  "RUNTIME_CONFIRMED",
  "SAFE_TO_PROPOSE",
] as const

export type CommandSourceVerifiedState =
  (typeof COMMAND_SOURCE_VERIFIED_STATES)[number]

export const COMMAND_SOURCE_VERIFIED_EVIDENCE = {
  sourceRead: "evidence://command/source-read",
  flagVerification: "evidence://command/flag-verification",
  runtimeCheck: "evidence://command/runtime-check",
} as const

export type CommandSourceVerifiedEvidenceRef =
  (typeof COMMAND_SOURCE_VERIFIED_EVIDENCE)[keyof typeof COMMAND_SOURCE_VERIFIED_EVIDENCE]

export interface CommandSourceVerifiedInputs {
  /** The full command line that is being proposed. */
  readonly commandString: string
  /** Path to the script the command invokes. */
  readonly scriptPath: string
  /** The flags the proposed command intends to use. */
  readonly expectedFlags: ReadonlyArray<string>
  /**
   * evidence://command/source-read — a content hash of the script, present only
   * after the file was actually read.
   */
  readonly sourceReadHash: string | null
  /**
   * evidence://command/flag-verification — the script's actual parsed argument
   * surface: every flag its argument parser declares. A stub declares none.
   */
  readonly declaredFlags: ReadonlyArray<string>
  /**
   * evidence://command/runtime-check — exit code of a dry-run / `--help` probe.
   * `0` means the probe succeeded; `null` means no probe was run.
   */
  readonly dryRunExitCode: number | null
}

export interface CommandSourceVerifiedResult {
  readonly state: CommandSourceVerifiedState
  /** True only when the terminal state SAFE_TO_PROPOSE is reached. */
  readonly canPropose: boolean
  /** The command identity evaluated by this result. */
  readonly identity: Readonly<{
    readonly commandString: string
    readonly scriptPath: string
  }>
  /** Flags considered (command-parsed flags unioned with expectedFlags). */
  readonly proposedFlags: ReadonlyArray<string>
  /** Proposed flags absent from the declared argument surface. */
  readonly unknownFlags: ReadonlyArray<string>
  readonly satisfiedEvidence: ReadonlyArray<CommandSourceVerifiedEvidenceRef>
  readonly missingEvidence: ReadonlyArray<CommandSourceVerifiedEvidenceRef>
  readonly locked: boolean
  readonly lockedAt: CommandSourceVerifiedState | null
  readonly blockedReason: string | null
}

/**
 * Parse the flag tokens from a command string. A flag is a token beginning with
 * `-`; `--flag=value` is normalized to `--flag`. Bare `--` (end-of-options) is
 * ignored.
 */
export function parseCommandFlags(
  commandString: string,
): ReadonlyArray<string> {
  if (typeof commandString !== "string" || commandString.trim().length === 0) {
    return []
  }
  const flags: Array<string> = []
  for (const rawToken of commandString.trim().split(/\s+/)) {
    // `--` is end-of-options: everything after it is positional, not a flag.
    if (rawToken === "--") {
      break
    }
    if (!rawToken.startsWith("-")) {
      continue
    }
    const eq = rawToken.indexOf("=")
    const flag = eq === -1 ? rawToken : rawToken.slice(0, eq)
    if (flag.length > 1 && !flags.includes(flag)) {
      flags.push(flag)
    }
  }
  return flags
}

function uniqueFlags(
  flags: ReadonlyArray<string>,
  extra: ReadonlyArray<string>,
): ReadonlyArray<string> {
  const out: Array<string> = []
  for (const flag of [...flags, ...extra]) {
    if (flag.length > 0 && !out.includes(flag)) {
      out.push(flag)
    }
  }
  return out
}

/**
 * Evaluate the `command-execution-source-verified` gate. Pure function.
 */
export function evaluateCommandSourceVerified(
  inputs: CommandSourceVerifiedInputs,
): CommandSourceVerifiedResult {
  const satisfied: Array<CommandSourceVerifiedEvidenceRef> = []
  const proposedFlags = uniqueFlags(
    parseCommandFlags(inputs.commandString),
    inputs.expectedFlags ?? [],
  )
  const identity = {
    commandString: inputs.commandString,
    scriptPath: inputs.scriptPath,
  }
  const declared = inputs.declaredFlags ?? []
  const unknownFlags = proposedFlags.filter((flag) => !declared.includes(flag))

  const lock = (
    state: CommandSourceVerifiedState,
    lockedAt: CommandSourceVerifiedState,
    blockedReason: string,
    missing: ReadonlyArray<CommandSourceVerifiedEvidenceRef>,
  ): CommandSourceVerifiedResult => ({
    state,
    canPropose: false,
    identity,
    proposedFlags,
    unknownFlags,
    satisfiedEvidence: satisfied,
    missingEvidence: missing,
    locked: true,
    lockedAt,
    blockedReason,
  })

  // Predicate 1 — source read + hashed.
  if (
    typeof inputs.sourceReadHash !== "string" ||
    inputs.sourceReadHash.trim().length === 0
  ) {
    return lock(
      "UNVERIFIED",
      "SOURCE_READ",
      `script ${inputs.scriptPath} was not read + hashed before proposing`,
      [
        COMMAND_SOURCE_VERIFIED_EVIDENCE.sourceRead,
        COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification,
        COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck,
      ],
    )
  }
  satisfied.push(COMMAND_SOURCE_VERIFIED_EVIDENCE.sourceRead)

  // Predicate 2 — flag verification. A real command being verified must carry at
  // least one flag, and every proposed flag must exist in the declared surface.
  if (proposedFlags.length === 0) {
    return lock(
      "SOURCE_READ",
      "FLAGS_VERIFIED",
      "no flags were proposed or expected; nothing to verify against the argument surface",
      [
        COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification,
        COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck,
      ],
    )
  }
  if (unknownFlags.length > 0) {
    return lock(
      "SOURCE_READ",
      "FLAGS_VERIFIED",
      `proposed flags not present in the parsed argument surface: ${unknownFlags.join(", ")}`,
      [
        COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification,
        COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck,
      ],
    )
  }
  satisfied.push(COMMAND_SOURCE_VERIFIED_EVIDENCE.flagVerification)

  // Predicate 3 — runtime / dry-run probe succeeded.
  if (inputs.dryRunExitCode !== 0) {
    return lock(
      "FLAGS_VERIFIED",
      "RUNTIME_CONFIRMED",
      inputs.dryRunExitCode === null
        ? "no dry-run / --help probe was run"
        : `dry-run / --help probe exited ${inputs.dryRunExitCode}`,
      [COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck],
    )
  }
  satisfied.push(COMMAND_SOURCE_VERIFIED_EVIDENCE.runtimeCheck)

  return {
    state: "SAFE_TO_PROPOSE",
    canPropose: true,
    identity,
    proposedFlags,
    unknownFlags,
    satisfiedEvidence: satisfied,
    missingEvidence: [],
    locked: false,
    lockedAt: null,
    blockedReason: null,
  }
}
