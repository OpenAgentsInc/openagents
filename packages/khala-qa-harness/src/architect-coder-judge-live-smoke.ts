const schema = "openagents.khala_code.architect_coder_judge_live_smoke.v1" as const

type SmokeEvidence = {
  readonly schema: typeof schema
  readonly ok: boolean
  readonly mode: "env_armed" | "skip_safe_default"
  readonly observedAt: string
  readonly publicSafe: true
  readonly refs: readonly string[]
  readonly status: "pass" | "skip"
  readonly summary: string
}

const armed = process.env.KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_SMOKE === "1"

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)

const stringArray = (value: unknown): readonly string[] =>
  Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : []

const fail = (summary: string): never => {
  console.error(JSON.stringify({
    schema,
    ok: false,
    mode: "env_armed",
    observedAt: new Date().toISOString(),
    publicSafe: true,
    refs: [],
    status: "fail",
    summary,
  }, null, 2))
  process.exit(1)
}

const validateArchivedEvidence = async (path: string): Promise<SmokeEvidence> => {
  if (path.trim().length === 0) {
    fail("KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_EVIDENCE_JSON is required when the live smoke is armed.")
  }
  const raw = await Bun.file(path).text().catch((error) =>
    fail(`Could not read KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_EVIDENCE_JSON: ${error instanceof Error ? error.message : String(error)}`)
  )
  const parsed = JSON.parse(raw) as unknown
  if (!isRecord(parsed)) fail("Archived evidence must be a JSON object.")
  const record = parsed as Record<string, unknown>
  const roles = stringArray(record.roles)
  const exactTokenRoles = stringArray(record.exactTokenRoles)
  const refs = stringArray(record.refs)
  for (const role of ["architect", "coder", "judge"]) {
    if (!roles.includes(role)) fail(`Archived evidence is missing role ${role}.`)
    if (!exactTokenRoles.includes(role)) fail(`Archived evidence is missing exact token rows for role ${role}.`)
  }
  if (record.publicSafe !== true) fail("Archived evidence must declare publicSafe=true.")
  if (record.judgeVerdict !== "accept" && record.judgeVerdict !== "request_changes" && record.judgeVerdict !== "replan") {
    fail("Archived evidence must include judgeVerdict accept|request_changes|replan.")
  }
  return {
    schema,
    ok: true,
    mode: "env_armed",
    observedAt: new Date().toISOString(),
    publicSafe: true,
    refs,
    status: "pass",
    summary: "Architect/Coder/Judge archived live evidence validated with exact per-role token rows and public-safe projection metadata.",
  }
}

const evidence: SmokeEvidence = armed
  ? await validateArchivedEvidence(process.env.KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_EVIDENCE_JSON ?? "")
  : {
      schema,
      ok: true,
      mode: "skip_safe_default",
      observedAt: new Date().toISOString(),
      publicSafe: true,
      refs: [
        "env.KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_SMOKE.required",
        "nightly.skip_safe.default",
      ],
      status: "skip",
      summary: "Architect/Coder/Judge live smoke skipped by default; set KHALA_CODE_ARCHITECT_CODER_JUDGE_LIVE_SMOKE=1 for an owner-armed run.",
    }

console.log(JSON.stringify(evidence, null, 2))
