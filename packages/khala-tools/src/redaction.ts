import type { GuardOptions, NerDetector, ScrubResult, TokenClassifier } from "@nationaldesignstudio/rampart"
import { Context, Effect, Layer } from "effect"

export type KhalaRampartGuard = {
  readonly protect: (text: string) => Promise<ScrubResult>
  readonly protectReply: (text: string) => Promise<ScrubResult>
  readonly reveal: (text: string) => string
  readonly revealTransform: () => TransformStream<string, string>
}

export type KhalaRampartGuardFactory = (options?: GuardOptions) => Promise<KhalaRampartGuard>

export type KhalaPrivacyRedactionMode = "rampart_model" | "rampart_heuristics" | "regex_only"

export type KhalaPrivacyRedactionResult = {
  readonly engine: "@nationaldesignstudio/rampart" | "@openagentsinc/khala-tools.regex"
  readonly mode: KhalaPrivacyRedactionMode
  readonly placeholders: readonly string[]
  readonly redacted: boolean
  readonly redactionRefs: readonly string[]
  readonly text: string
}

export type KhalaPrivacyRedactionServiceShape = {
  readonly protectModelText: (text: string) => Effect.Effect<KhalaPrivacyRedactionResult, never>
  readonly protectUserText: (text: string) => Effect.Effect<KhalaPrivacyRedactionResult, never>
  readonly revealForLocalUser: (text: string) => Effect.Effect<string, never>
  readonly revealTransform: () => Effect.Effect<TransformStream<string, string>, never>
}

export type KhalaPrivacyRedactionServiceOptions = {
  readonly guardFactory?: KhalaRampartGuardFactory
  readonly rampartOptions?: GuardOptions
}

type LoadedRampartGuard = {
  readonly guard?: KhalaRampartGuard
  readonly mode: KhalaPrivacyRedactionMode
  readonly redactionRefs: readonly string[]
}

const RAMPART_PII_REF = "redaction.khala.rampart.pii"
const RAMPART_FULL_MODEL_UNAVAILABLE_REF = "redaction.khala.rampart.full_model_unavailable"
const RAMPART_HEURISTICS_UNAVAILABLE_REF = "redaction.khala.rampart.heuristics_unavailable"
const RAMPART_RUNTIME_FAILURE_REF = "redaction.khala.rampart.runtime_failure"
const REGEX_SECRET_REF = "redaction.khala.regex.secret_material"

export class KhalaPrivacyRedactionService extends Context.Service<
  KhalaPrivacyRedactionService,
  KhalaPrivacyRedactionServiceShape
>()("@openagentsinc/khala-tools/KhalaPrivacyRedactionService") {
  static readonly Default = Layer.succeed(KhalaPrivacyRedactionService, makeKhalaPrivacyRedactionService())
}

export const KhalaPrivacyRedactionLive = KhalaPrivacyRedactionService.Default

export function makeKhalaPrivacyRedactionService(
  options: KhalaPrivacyRedactionServiceOptions = {},
): KhalaPrivacyRedactionServiceShape {
  const guardFactory = options.guardFactory ?? createRampartGuard
  const rampartOptions = options.rampartOptions ?? {}
  let guardPromise: Promise<LoadedRampartGuard> | undefined
  let loadedGuard: LoadedRampartGuard | undefined

  const loadGuard = async (): Promise<LoadedRampartGuard> => {
    if (guardPromise === undefined) {
      guardPromise = buildRampartGuard(guardFactory, rampartOptions)
        .then(guard => {
          loadedGuard = guard
          return guard
        })
    }
    return await guardPromise
  }

  const protect = (kind: "reply" | "user", text: string): Effect.Effect<KhalaPrivacyRedactionResult, never> =>
    Effect.promise(async () => {
      const loaded = await loadGuard()
      if (loaded.guard === undefined) {
        return regexOnlyResult(text, loaded.redactionRefs)
      }
      try {
        const scrubbed = kind === "reply"
          ? await loaded.guard.protectReply(text)
          : await loaded.guard.protect(text)
        return rampartResult(text, scrubbed, loaded)
      } catch {
        return regexOnlyResult(text, [...loaded.redactionRefs, RAMPART_RUNTIME_FAILURE_REF])
      }
    })

  return {
    protectModelText: text => protect("reply", text),
    protectUserText: text => protect("user", text),
    revealForLocalUser: text =>
      Effect.sync(() => {
        if (loadedGuard?.guard === undefined) return text
        try {
          return loadedGuard.guard.reveal(text)
        } catch {
          return text
        }
      }),
    revealTransform: () =>
      Effect.sync(() => {
        if (loadedGuard?.guard === undefined) return identityTransform()
        try {
          return loadedGuard.guard.revealTransform()
        } catch {
          return identityTransform()
        }
      }),
  }
}

export function redactKhalaPublicText(value: string): string {
  return value
    .replace(/OPENROUTER_API_KEY\s*[:=]\s*\S+/giu, "OPENROUTER_API_KEY=[REDACTED]")
    .replace(/sk-or-[A-Za-z0-9_-]{8,}/gu, "[REDACTED_OPENROUTER_KEY]")
    .replace(/sk-[A-Za-z0-9_-]{16,}/gu, "[REDACTED_API_KEY]")
    .replace(/Bearer\s+[A-Za-z0-9._-]{16,}/giu, "Bearer [REDACTED_TOKEN]")
}

async function buildRampartGuard(
  guardFactory: KhalaRampartGuardFactory,
  rampartOptions: GuardOptions,
): Promise<LoadedRampartGuard> {
  const fullOptions: GuardOptions = {
    device: defaultRampartDevice(),
    ...rampartOptions,
  }

  if (fullOptions.heuristicsOnly === true) {
    try {
      return {
        guard: await guardFactory(fullOptions),
        mode: "rampart_heuristics",
        redactionRefs: [],
      }
    } catch {
      return {
        mode: "regex_only",
        redactionRefs: [RAMPART_HEURISTICS_UNAVAILABLE_REF],
      }
    }
  }

  try {
    return {
      guard: await guardFactory(fullOptions),
      mode: "rampart_model",
      redactionRefs: [],
    }
  } catch {
    try {
      return {
        guard: await guardFactory({ ...fullOptions, heuristicsOnly: true }),
        mode: "rampart_heuristics",
        redactionRefs: [RAMPART_FULL_MODEL_UNAVAILABLE_REF],
      }
    } catch {
      return {
        mode: "regex_only",
        redactionRefs: [RAMPART_FULL_MODEL_UNAVAILABLE_REF, RAMPART_HEURISTICS_UNAVAILABLE_REF],
      }
    }
  }
}

async function createRampartGuard(options?: GuardOptions): Promise<KhalaRampartGuard> {
  const rampart = await import("@nationaldesignstudio/rampart")
  if (shouldInjectNodeCompatibleNer(options)) {
    const ner = await createNodeCompatibleRampartNer(options)
    return await rampart.createGuard({ ...options, ner })
  }
  return await rampart.createGuard(options)
}

function shouldInjectNodeCompatibleNer(options: GuardOptions | undefined): boolean {
  return isNodeLikeRuntime() &&
    options?.heuristicsOnly !== true &&
    options?.ner === undefined &&
    options?.worker === undefined
}

async function createNodeCompatibleRampartNer(options: GuardOptions | undefined): Promise<NerDetector> {
  const [transformers, rampart] = await Promise.all([
    import("@huggingface/transformers"),
    import("@nationaldesignstudio/rampart"),
  ])
  const model = options?.model ?? rampart.RAMPART_MODEL_ID
  const device = options?.device ?? "cpu"
  const classifier = await (transformers as unknown as TransformersModule).pipeline("token-classification", model, {
    dtype: "q4",
    device,
  })
  const adapter = classifierAdapter(classifier)
  return (text: string) => rampart.detectNer(text, adapter, options?.minScore)
}

type TransformersModule = {
  readonly pipeline: (
    task: "token-classification",
    model: string,
    options: {
      readonly dtype: "q4"
      readonly device: NonNullable<GuardOptions["device"]>
    },
  ) => Promise<TransformersTokenClassificationPipeline>
}

type TransformersTokenClassificationPipeline = {
  readonly tokenizer?: {
    readonly encode?: (text: string, options?: { readonly add_special_tokens?: boolean }) => number[]
    readonly tokenize?: (text: string) => string[]
  }
  (text: string, options?: unknown): Promise<unknown>
}

function classifierAdapter(classifier: TransformersTokenClassificationPipeline): TokenClassifier {
  const adapter: TokenClassifier = (text: string, options?: { aggregation_strategy?: "simple" | "first" | "max" }) =>
    classifier(text, options) as ReturnType<TokenClassifier>
  const tokenizer = classifier.tokenizer
  if (tokenizer?.encode !== undefined) {
    adapter.countTokens = (text: string) => tokenizer.encode!(text, { add_special_tokens: false }).length
  }
  if (tokenizer?.tokenize !== undefined) {
    adapter.tokenize = (text: string) => tokenizer.tokenize!(text)
  }
  return adapter
}

function rampartResult(
  originalText: string,
  scrubbed: ScrubResult,
  loaded: LoadedRampartGuard,
): KhalaPrivacyRedactionResult {
  const text = redactKhalaPublicText(scrubbed.text)
  return {
    engine: "@nationaldesignstudio/rampart",
    mode: loaded.mode,
    placeholders: scrubbed.placeholders,
    redacted: text !== originalText,
    redactionRefs: unique([
      ...loaded.redactionRefs,
      ...(scrubbed.text === originalText ? [] : [RAMPART_PII_REF]),
      ...(text === scrubbed.text ? [] : [REGEX_SECRET_REF]),
    ]),
    text,
  }
}

function regexOnlyResult(
  originalText: string,
  redactionRefs: readonly string[],
): KhalaPrivacyRedactionResult {
  const text = redactKhalaPublicText(originalText)
  return {
    engine: "@openagentsinc/khala-tools.regex",
    mode: "regex_only",
    placeholders: [],
    redacted: text !== originalText,
    redactionRefs: unique([
      ...redactionRefs,
      ...(text === originalText ? [] : [REGEX_SECRET_REF]),
    ]),
    text,
  }
}

function defaultRampartDevice(): NonNullable<GuardOptions["device"]> {
  return isNodeLikeRuntime() ? "cpu" : "wasm"
}

function isNodeLikeRuntime(): boolean {
  const runtime = globalThis as typeof globalThis & {
    readonly process?: {
      readonly versions?: {
        readonly node?: string
      }
    }
  }
  return typeof runtime.process?.versions?.node === "string"
}

function identityTransform(): TransformStream<string, string> {
  return new TransformStream<string, string>({
    transform(chunk, controller) {
      controller.enqueue(chunk)
    },
  })
}

function unique(values: readonly string[]): readonly string[] {
  return [...new Set(values)]
}
