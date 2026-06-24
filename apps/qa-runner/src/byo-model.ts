// Bring-your-own-model config for the OSS `qa` CLI — NO OpenAgents login.
//
// This is the OSS, local-first, runtime-agnostic credential path required by
// issue #6191 (Rhys req #5). It deliberately does NOT touch `khala-config.ts`:
// that module discovers OpenAgents agent tokens in `~/work/.secrets/` and treats
// the OpenAI-compatible endpoint as a "fallback". Here the OpenAI-compatible
// endpoint is the FIRST-CLASS, ONLY path: you bring any model + baseURL + key.
//
//   - Khala is just ONE option (point `--base-url` at openagents.com if you want
//     it) — it is NOT required and NOT preferred.
//   - There is no `~/work/.secrets/` discovery, no agent-token lookup, and no
//     hidden OpenAgents dependency. The core local run works with no OpenAgents
//     account.
//
// Resolution is pure and explicit (flags win over env), so a third party
// (executor.sh's CI) can reason about exactly what endpoint will be hit. The
// credential VALUE is never returned in a log line — only its SOURCE label.

/** A fully-resolved BYO OpenAI-compatible model endpoint. */
export interface ByoModelConfig {
  /** OpenAI-compatible base URL, e.g. https://api.openai.com/v1 (no trailing /). */
  readonly baseUrl: string;
  /** The model id, e.g. gpt-4o-mini, or any served alias on your endpoint. */
  readonly model: string;
  /** The bearer key for the endpoint. May be empty for keyless local servers. */
  readonly apiKey: string;
  /** Where the key came from (label only, never the value). */
  readonly keySource: string;
}

/** Explicit flags (parsed from argv) that override env. */
export interface ByoModelFlags {
  readonly model?: string;
  readonly baseUrl?: string;
  readonly apiKey?: string;
}

export interface ResolveByoModelOptions {
  readonly flags?: ByoModelFlags;
  readonly env?: Readonly<Record<string, string | undefined>>;
  /**
   * Allow a keyless run (some local OpenAI-compatible servers — llama.cpp,
   * vLLM, Ollama's OpenAI shim — accept any/no key). Default false: a missing
   * key is an honest error rather than a silent unauthenticated call.
   */
  readonly allowKeyless?: boolean;
}

export class ByoModelConfigError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ByoModelConfigError";
  }
}

const stripTrailingSlash = (s: string): string => s.replace(/\/+$/, "");

/**
 * Resolve the BYO model endpoint from flags + env. Pure and deterministic.
 *
 * Precedence (first non-empty wins), per field:
 *   model    : --model    > QA_MODEL    > OPENAI_MODEL    > (required)
 *   base-url : --base-url > QA_BASE_URL > OPENAI_BASE_URL > (required)
 *   api-key  : --api-key  > QA_API_KEY  > OPENAI_API_KEY  > (required, unless allowKeyless)
 *
 * `QA_*` are the CLI's own neutral names; `OPENAI_*` are accepted because they
 * are the de-facto standard for OpenAI-compatible endpoints, so an existing CI
 * with `OPENAI_API_KEY` / `OPENAI_BASE_URL` set works with zero new config.
 *
 * No OpenAgents-specific env var, no secrets-dir discovery, no login.
 */
export function resolveByoModelConfig(options: ResolveByoModelOptions = {}): ByoModelConfig {
  const flags = options.flags ?? {};
  const env = options.env ?? process.env;
  const allowKeyless = options.allowKeyless ?? false;

  const model = firstNonEmpty(flags.model, env.QA_MODEL, env.OPENAI_MODEL);
  if (!model) {
    throw new ByoModelConfigError(
      "no model specified: pass --model <id> (or set QA_MODEL / OPENAI_MODEL). " +
        "The model is bring-your-own; e.g. --model gpt-4o-mini, or any served alias on your endpoint.",
    );
  }

  const baseUrl = firstNonEmpty(flags.baseUrl, env.QA_BASE_URL, env.OPENAI_BASE_URL);
  if (!baseUrl) {
    throw new ByoModelConfigError(
      "no base URL specified: pass --base-url <url> (or set QA_BASE_URL / OPENAI_BASE_URL). " +
        "Use any OpenAI-compatible endpoint, e.g. https://api.openai.com/v1 or http://localhost:8080/v1.",
    );
  }

  const keyResolution = resolveKey(flags.apiKey, env);
  if (!keyResolution.apiKey && !allowKeyless) {
    throw new ByoModelConfigError(
      "no API key specified: pass --api-key <key> (or set QA_API_KEY / OPENAI_API_KEY). " +
        "Pass --allow-keyless for a local server that needs no key.",
    );
  }

  return {
    baseUrl: stripTrailingSlash(baseUrl),
    model,
    apiKey: keyResolution.apiKey,
    keySource: keyResolution.keySource,
  };
}

function resolveKey(
  flagKey: string | undefined,
  env: Readonly<Record<string, string | undefined>>,
): { apiKey: string; keySource: string } {
  if (flagKey && flagKey.length > 0) return { apiKey: flagKey, keySource: "--api-key flag" };
  if (env.QA_API_KEY && env.QA_API_KEY.length > 0) return { apiKey: env.QA_API_KEY, keySource: "QA_API_KEY env" };
  if (env.OPENAI_API_KEY && env.OPENAI_API_KEY.length > 0)
    return { apiKey: env.OPENAI_API_KEY, keySource: "OPENAI_API_KEY env" };
  return { apiKey: "", keySource: "none (keyless)" };
}

function firstNonEmpty(...values: ReadonlyArray<string | undefined>): string | undefined {
  for (const v of values) if (v && v.length > 0) return v;
  return undefined;
}

/**
 * Build a plain `fetch`-based OpenAI-compatible chat client from a BYO config.
 * No new dependency; no OpenAgents code path. A per-request timeout aborts a
 * stuck call. `Authorization` is omitted entirely for a keyless endpoint.
 *
 * Shaped to satisfy the runner's `ChatClient` (`{ complete(messages) }`) without
 * importing the Khala module, keeping the BYO core free of Khala coupling.
 */
export function makeByoChatClient(
  config: ByoModelConfig,
  options: { readonly timeoutMs?: number; readonly maxTokens?: number; readonly fetchImpl?: typeof fetch } = {},
): { complete: (messages: ReadonlyArray<{ role: string; content: string }>) => Promise<string> } {
  const timeoutMs = options.timeoutMs ?? 90_000;
  const maxTokens = options.maxTokens ?? 1024;
  const fetchImpl = options.fetchImpl ?? fetch;
  const url = `${config.baseUrl}/chat/completions`;
  return {
    complete: async (messages) => {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const headers: Record<string, string> = { "content-type": "application/json" };
        if (config.apiKey.length > 0) headers.authorization = `Bearer ${config.apiKey}`;
        const response = await fetchImpl(url, {
          method: "POST",
          headers,
          body: JSON.stringify({ model: config.model, messages, temperature: 0, max_tokens: maxTokens }),
          signal: controller.signal,
        });
        if (!response.ok) {
          const body = await response.text().catch(() => "");
          throw new Error(`chat/completions HTTP ${response.status}: ${body.slice(0, 300)}`);
        }
        const json = (await response.json()) as { choices?: Array<{ message?: { content?: string } }> };
        return json.choices?.[0]?.message?.content ?? "";
      } finally {
        clearTimeout(timer);
      }
    },
  };
}
