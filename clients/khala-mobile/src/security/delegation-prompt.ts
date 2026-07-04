export type DelegationPromptValidation = Readonly<{
  ok: boolean
  prompt: string
  blockerRefs: ReadonlyArray<string>
  messageSafe: string
}>

export class DelegationPromptValidationError extends Error {
  readonly _tag = "DelegationPromptValidationError"

  constructor(readonly validation: DelegationPromptValidation) {
    super(validation.messageSafe)
    this.name = "DelegationPromptValidationError"
  }
}

const MIN_PROMPT_CHARS = 3
const MAX_PROMPT_CHARS = 2_000

const unsafePromptPatterns: ReadonlyArray<Readonly<{
  pattern: RegExp
  blockerRef: string
}>> = [
  {
    blockerRef: "blocker.khala_mobile.prompt.local_path",
    pattern: /(^|[\s"'`])(?:\/Users\/|\/private\/|~\/|~\.|\.\/\.secrets\/|\.secrets\/)/iu
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.codex_auth_path",
    pattern: /(?:^|[\s"'`])(?:~\/)?\.codex\/auth\.json\b/iu
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.bearer_token",
    pattern: /\bBearer\s+[A-Za-z0-9._~+/=-]{12,}/u
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.openagents_api_key",
    pattern: /\boa_agent_[A-Za-z0-9._:-]+/u
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.provider_secret",
    pattern: /\b(?:OPENAI|ANTHROPIC|STRIPE|MDK|NEXUS|PROBE|CLOUDFLARE)_[A-Z0-9_]*(?:KEY|TOKEN|SECRET)\b/iu
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.private_material",
    pattern: /\b(?:mnemonic|private key|refresh token|access token|client secret|password)\b/iu
  },
  {
    blockerRef: "blocker.khala_mobile.prompt.email_address",
    pattern: /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/iu
  },
  {
    blockerRef: "blocker.khala_mobile.prompt_high_entropy",
    pattern: /[A-Za-z0-9+/=_-]{40,}/u
  }
]

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)]

export const normalizeDelegationPrompt = (prompt: string): string =>
  prompt.replace(/\s+/g, " ").trim()

export const validateDelegationPrompt = (
  rawPrompt: string,
): DelegationPromptValidation => {
  const prompt = normalizeDelegationPrompt(rawPrompt)
  const blockerRefs: Array<string> = []

  if (prompt.length < MIN_PROMPT_CHARS) {
    blockerRefs.push("blocker.khala_mobile.prompt_too_short")
  }

  if (prompt.length > MAX_PROMPT_CHARS) {
    blockerRefs.push("blocker.khala_mobile.prompt_too_long")
  }

  for (const unsafe of unsafePromptPatterns) {
    if (unsafe.pattern.test(prompt)) blockerRefs.push(unsafe.blockerRef)
  }

  const refs = unique(blockerRefs)
  return {
    blockerRefs: refs,
    messageSafe:
      refs.length === 0
        ? "Delegation prompt is public-safe."
        : "Delegation prompt includes private or unsafe material.",
    ok: refs.length === 0,
    prompt
  }
}

export const assertDelegationPrompt = (rawPrompt: string): string => {
  const validation = validateDelegationPrompt(rawPrompt)
  if (!validation.ok) throw new DelegationPromptValidationError(validation)

  return validation.prompt
}
