import { Schema } from "effect"

/**
 * HANDS-2 (#9173): host-executed done-condition verification. Today a Full
 * Auto turn self-reports completion and the host raises only a post-hoc
 * `unverified_completion_risk` flag (full-auto-run-analyzer.ts). This module
 * turns "the host or owner verifies the done condition" (the exact literal the
 * mission packet carries, full-auto-mission.ts) into an EXECUTED check.
 *
 * The spec is a typed value, never an ad-hoc keyword match on user intent. The
 * runnable command is either supplied EXPLICITLY on the run's autonomy block,
 * or extracted from a STRUCTURED marker in the done-condition text (a
 * `verify:`-prefixed line or a fenced ```verify block) -- bounded-field
 * parsing on already-selected content, not semantic routing. When no runnable
 * check exists the spec is `none`: the host cannot verify and completion stays
 * owner-gated (never auto-admitted).
 *
 * Admission discipline (the core safety property):
 *  - Only a PASSED host verification admits completion.
 *  - A failed, absent, or errored verification keeps the run active with a
 *    typed reason. Provider self-report remains self-reported evidence only.
 *  - The host verdict is recorded SEPARATELY from the provider self-report, so
 *    "provider done" and "host verified" never collapse into one fact.
 */
export const FULL_AUTO_VERIFICATION_SPEC_SCHEMA = "openagents.desktop.full_auto_verification_spec.v1" as const
export const FULL_AUTO_VERIFICATION_RESULT_SCHEMA = "openagents.desktop.full_auto_verification_result.v1" as const

export const FULL_AUTO_VERIFICATION_COMMAND_LIMIT = 2000
export const FULL_AUTO_VERIFICATION_DETAIL_LIMIT = 2000
/** Bound the captured command output so a chatty verifier cannot bloat the
 * durable run record. */
export const FULL_AUTO_VERIFICATION_OUTPUT_TAIL_LIMIT = 1600
export const FULL_AUTO_VERIFICATION_DEFAULT_TIMEOUT_MS = 15 * 60_000

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const Command = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_VERIFICATION_COMMAND_LIMIT))
const Cwd = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1024))

/**
 * The typed verification spec. `command` runs a named check in the bound
 * workspace; `evidence_ref` asserts a durable evidence ref is present;
 * `none` means there is nothing the host can execute (owner-gated).
 */
export const FullAutoVerificationSpecSchema = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("command"),
    command: Command,
    cwd: Schema.optional(Cwd),
  }),
  Schema.Struct({
    kind: Schema.Literal("evidence_ref"),
    ref: Ref,
  }),
  Schema.Struct({
    kind: Schema.Literal("none"),
  }),
])
export type FullAutoVerificationSpec = typeof FullAutoVerificationSpecSchema.Type

export const FULL_AUTO_VERIFICATION_SPEC_NONE: FullAutoVerificationSpec = { kind: "none" }

export const decodeFullAutoVerificationSpec = Schema.decodeUnknownSync(FullAutoVerificationSpecSchema)

/**
 * `passed` -- the host ran the check and it succeeded (admits completion).
 * `failed` -- the host ran the check and it failed (keeps the run active).
 * `absent` -- there is no runnable check (spec `none`, or an evidence ref that
 *   is not present): the host could not verify, so completion is NOT admitted.
 * `error` -- the check could not be executed (spawn failure, timeout): the
 *   host could not verify, so completion is NOT admitted.
 */
export const FullAutoVerificationStatusSchema = Schema.Literals(["passed", "failed", "absent", "error"])
export type FullAutoVerificationStatus = typeof FullAutoVerificationStatusSchema.Type

export const FullAutoVerificationResultSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_VERIFICATION_RESULT_SCHEMA),
  spec: FullAutoVerificationSpecSchema,
  status: FullAutoVerificationStatusSchema,
  /** Non-null only for a command verification that actually ran. */
  exitCode: Schema.NullOr(Schema.Number.check(Schema.isInt())),
  /** Bounded, public-safe detail: a command's tail output, the evidence ref,
   * or an error message. Never raw secrets -- callers must not put credential
   * material in a verification command. */
  detail: Schema.NullOr(Schema.String.check(Schema.isMaxLength(FULL_AUTO_VERIFICATION_DETAIL_LIMIT))),
  evidenceRef: Schema.optional(Ref),
  at: Schema.String,
})
export type FullAutoVerificationResult = typeof FullAutoVerificationResultSchema.Type

const decodeFullAutoVerificationResult = Schema.decodeUnknownSync(FullAutoVerificationResultSchema)

// -----------------------------------------------------------------------
// Structured extraction from a done-condition -- bounded-field parsing only.
// -----------------------------------------------------------------------

/**
 * Extract a runnable verification command from STRUCTURED markers in the
 * done-condition text. Recognizes exactly two bounded shapes:
 *  - a fenced block:  ```verify\n<command>\n```
 *  - a line prefix:   verify: <command>   (case-insensitive prefix)
 * Anything else yields `none` -- this is deterministic bounded-field parsing,
 * not a semantic guess at what the owner "meant". An explicit typed spec on
 * the run always takes precedence over this extraction.
 */
export const deriveFullAutoVerificationSpec = (
  doneCondition: string,
  options?: Readonly<{ cwd?: string }>,
): FullAutoVerificationSpec => {
  const fenced = /```verify[ \t]*\r?\n([\s\S]*?)```/i.exec(doneCondition)
  const fencedCommand = fenced?.[1]?.trim()
  if (fencedCommand !== undefined && fencedCommand.length > 0 && fencedCommand.length <= FULL_AUTO_VERIFICATION_COMMAND_LIMIT) {
    return decodeFullAutoVerificationSpec({
      kind: "command",
      command: fencedCommand,
      ...(options?.cwd === undefined ? {} : { cwd: options.cwd }),
    })
  }
  for (const rawLine of doneCondition.split(/\r?\n/)) {
    const match = /^[ \t>*-]*verify:[ \t]*(.+?)[ \t]*$/i.exec(rawLine)
    const command = match?.[1]?.trim()
    if (command !== undefined && command.length > 0 && command.length <= FULL_AUTO_VERIFICATION_COMMAND_LIMIT) {
      return decodeFullAutoVerificationSpec({
        kind: "command",
        command,
        ...(options?.cwd === undefined ? {} : { cwd: options.cwd }),
      })
    }
  }
  return FULL_AUTO_VERIFICATION_SPEC_NONE
}

/**
 * The structured marker a provider turn emits to REQUEST host verification of
 * the done condition. It is a self-report of belief, never a completion: the
 * host still runs {@link runFullAutoVerification} and only a PASSED verdict
 * admits completion. Recognized shapes (case-insensitive keyword only):
 *  - a line:  FULL-AUTO-COMPLETE   (optionally list/quote prefixed)
 *  - a fence: ```full-auto-complete```
 * Deterministic bounded detection -- never an NLP guess that a turn "sounds
 * done". Absent the marker, the host does not run verification for the turn
 * (so a normal continuation never pays for a verification command).
 */
export const detectFullAutoSelfReportedCompletion = (assistantText: string): boolean => {
  if (/```full-auto-complete[ \t]*\r?\n?[\s\S]*?```/i.test(assistantText)) return true
  return assistantText
    .split(/\r?\n/)
    .some((line) => /^[ \t>*-]*full-auto-complete\b/i.test(line.trim()))
}

// -----------------------------------------------------------------------
// Execution seam.
// -----------------------------------------------------------------------

export type FullAutoVerificationCommandOutcome = Readonly<{
  exitCode: number
  stdout?: string
  stderr?: string
}>

/** The injected command runner. The host binds a real child-process adapter
 * (`makeNodeVerificationExec`); tests inject a deterministic stub. It runs in
 * the bound workspace and returns an exit code plus bounded output, or throws
 * when the process could not be run (which becomes an `error` verdict). */
export type FullAutoVerificationExec = (
  input: Readonly<{ command: string; cwd: string | undefined; timeoutMs: number }>,
) => Promise<FullAutoVerificationCommandOutcome>

export type FullAutoVerificationEvidencePresent = (ref: string) => boolean | Promise<boolean>

const boundTail = (value: string): string =>
  value.length <= FULL_AUTO_VERIFICATION_OUTPUT_TAIL_LIMIT
    ? value
    : `…${value.slice(value.length - FULL_AUTO_VERIFICATION_OUTPUT_TAIL_LIMIT)}`

const boundDetail = (value: string): string =>
  value.length <= FULL_AUTO_VERIFICATION_DETAIL_LIMIT ? value : `${value.slice(0, FULL_AUTO_VERIFICATION_DETAIL_LIMIT)}…`

export type RunFullAutoVerificationInput = Readonly<{
  spec: FullAutoVerificationSpec
  /** Required for a `command` spec; ignored otherwise. */
  exec?: FullAutoVerificationExec
  /** Required for an `evidence_ref` spec; ignored otherwise. */
  evidencePresent?: FullAutoVerificationEvidencePresent
  /** Overrides `spec.cwd` for a command (e.g. the run's bound workspaceRef). */
  workspaceRef?: string
  timeoutMs?: number
  now?: () => Date
}>

/**
 * Execute the verification and return a typed, durable-safe result. Never
 * throws: a spawn failure or timeout becomes an `error` verdict. A `command`
 * spec with no `exec` supplied is `error` (misconfiguration, never a silent
 * pass). An `evidence_ref` spec with no `evidencePresent` supplied is
 * `absent`.
 */
export const runFullAutoVerification = async (
  input: RunFullAutoVerificationInput,
): Promise<FullAutoVerificationResult> => {
  const now = input.now ?? (() => new Date())
  const at = now().toISOString()
  const timeoutMs = input.timeoutMs ?? FULL_AUTO_VERIFICATION_DEFAULT_TIMEOUT_MS
  const base = { schema: FULL_AUTO_VERIFICATION_RESULT_SCHEMA, spec: input.spec, at } as const

  if (input.spec.kind === "none") {
    return decodeFullAutoVerificationResult({
      ...base,
      status: "absent",
      exitCode: null,
      detail: "no host-runnable verification is defined for this run",
    })
  }

  if (input.spec.kind === "evidence_ref") {
    if (input.evidencePresent === undefined) {
      return decodeFullAutoVerificationResult({
        ...base,
        status: "absent",
        exitCode: null,
        detail: "no evidence resolver supplied",
        evidenceRef: input.spec.ref,
      })
    }
    let present: boolean
    try {
      present = await input.evidencePresent(input.spec.ref)
    } catch (error) {
      return decodeFullAutoVerificationResult({
        ...base,
        status: "error",
        exitCode: null,
        detail: boundDetail(error instanceof Error ? error.message : "evidence resolver threw"),
        evidenceRef: input.spec.ref,
      })
    }
    return decodeFullAutoVerificationResult({
      ...base,
      status: present ? "passed" : "absent",
      exitCode: null,
      detail: present ? `evidence ref present: ${input.spec.ref}` : `evidence ref not present: ${input.spec.ref}`,
      evidenceRef: input.spec.ref,
    })
  }

  // command
  if (input.exec === undefined) {
    return decodeFullAutoVerificationResult({
      ...base,
      status: "error",
      exitCode: null,
      detail: "no command executor supplied for a command verification",
    })
  }
  const cwd = input.workspaceRef ?? input.spec.cwd
  try {
    const outcome = await input.exec({ command: input.spec.command, cwd, timeoutMs })
    const tail = boundTail([outcome.stdout ?? "", outcome.stderr ?? ""].filter((part) => part.length > 0).join("\n").trim())
    return decodeFullAutoVerificationResult({
      ...base,
      status: outcome.exitCode === 0 ? "passed" : "failed",
      exitCode: outcome.exitCode,
      detail: tail.length > 0 ? boundDetail(tail) : `exit code ${outcome.exitCode}`,
    })
  } catch (error) {
    return decodeFullAutoVerificationResult({
      ...base,
      status: "error",
      exitCode: null,
      detail: boundDetail(error instanceof Error ? error.message : "verification command could not be executed"),
    })
  }
}

// -----------------------------------------------------------------------
// Admission -- the single gate that separates "provider done" from "verified".
// -----------------------------------------------------------------------

/** Only a PASSED host verification admits completion. */
export const admitFullAutoCompletion = (result: FullAutoVerificationResult): boolean => result.status === "passed"

/** The typed reason a self-reported completion was NOT admitted, or null when
 * it was. Callers persist this on the run so the owner sees WHY the run stayed
 * active despite a provider "done". */
export const fullAutoCompletionBlockReason = (result: FullAutoVerificationResult): string | null => {
  switch (result.status) {
    case "passed":
      return null
    case "failed":
      return `host_verification_failed${result.exitCode === null ? "" : `:exit_${result.exitCode}`}`
    case "absent":
      return "host_verification_absent"
    case "error":
      return "host_verification_error"
  }
}

// -----------------------------------------------------------------------
// Real host adapter -- a bounded child-process runner. Injectable so tests
// never spawn a process.
// -----------------------------------------------------------------------

/**
 * A child-process-backed executor for a `command` verification. Runs the
 * command through the platform shell in `cwd`, captures bounded stdout/stderr,
 * enforces the timeout (a killed process reports a non-zero exit code, i.e. a
 * FAILED verdict, not a silent pass). Imported lazily so the module stays
 * pure/testable and does not pull `node:child_process` into a renderer bundle.
 */
export const makeNodeVerificationExec = (): FullAutoVerificationExec => async ({ command, cwd, timeoutMs }) => {
  const { spawn } = await import("node:child_process")
  return await new Promise<FullAutoVerificationCommandOutcome>((resolve, reject) => {
    const child = spawn(command, {
      cwd,
      shell: true,
      timeout: timeoutMs,
      env: process.env,
    })
    let stdout = ""
    let stderr = ""
    child.stdout?.on("data", (chunk: Buffer) => {
      stdout = boundTail(stdout + chunk.toString("utf8"))
    })
    child.stderr?.on("data", (chunk: Buffer) => {
      stderr = boundTail(stderr + chunk.toString("utf8"))
    })
    child.on("error", (error) => reject(error))
    child.on("close", (code, signal) => {
      resolve({
        exitCode: code ?? (signal === null ? 1 : 128),
        stdout,
        stderr: signal === null ? stderr : `${stderr}\n[terminated by signal ${signal}]`.trim(),
      })
    })
  })
}
