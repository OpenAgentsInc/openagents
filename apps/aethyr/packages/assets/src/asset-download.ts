export interface AssetDownloadNotice {
  path: string;
  attempt: number;
  status?: number;
}
interface AssetDownloadPolicy {
  signal?: AbortSignal;
  onRetry?: (notice: AssetDownloadNotice) => void;
}
let policy: AssetDownloadPolicy = {};

/** One worker lifetime owns its downloads; closing it also cancels pending retries. */
export function configureAssetDownloads(next: AssetDownloadPolicy) {
  const previous = policy;
  policy = next;
  return () => {
    if (policy === next) policy = previous;
  };
}

export class AssetDownloadError extends Error {
  constructor(
    readonly path: string,
    readonly attempts: number,
    readonly status?: number,
  ) {
    super("A world file could not finish downloading. Try again.");
    this.name = "AssetDownloadError";
  }
}
/** A complete HTTP response can still contain a truncated or damaged immutable file. */
export class AssetIntegrityError extends Error {}
export async function verifyAssetBytes(
  bytes: ArrayBuffer | Uint8Array<ArrayBuffer>,
  expected: { byteLength: number; sha256: string },
  failure = "A world file is incomplete. Try again.",
) {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  if (view.byteLength !== expected.byteLength)
    throw new AssetIntegrityError(failure);
  const hash = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", view)),
    (v) => v.toString(16).padStart(2, "0"),
  ).join("");
  if (hash !== expected.sha256) throw new AssetIntegrityError(failure);
}
class TemporaryResponse extends Error {
  constructor(readonly status: number) {
    super("Temporary asset response");
  }
}
export interface AssetDownloadOptions {
  fetcher?: (url: string, init?: RequestInit) => Promise<Response>;
  init?: RequestInit;
  /** Override only for non-browser callers such as deterministic loader tests. */
  baseURL?: string;
}
const retryableStatus = new Set([408, 425, 429, 500, 502, 503, 504]);
const backoff = [250, 1000];
function abort(signal?: AbortSignal) {
  if (signal?.aborted)
    throw (
      signal.reason ?? new DOMException("The world was closed.", "AbortError")
    );
}
function pause(ms: number, signal?: AbortSignal): Promise<void> {
  abort(signal);
  return new Promise((resolve, reject) => {
    const cancel = () => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", cancel);
      reject(
        signal?.reason ??
          new DOMException("The world was closed.", "AbortError"),
      );
    };
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", cancel);
      resolve();
    }, ms);
    signal?.addEventListener("abort", cancel, { once: true });
  });
}

/** Retry the complete read, never append a new response to a partially read file. */
export async function readAsset<T>(
  url: string,
  consume: (response: Response) => Promise<T>,
  options: AssetDownloadOptions = {},
): Promise<T> {
  const fetcher = options.fetcher ?? globalThis.fetch;
  const base = new URL(
    options.baseURL ?? globalThis.location?.href ?? "http://localhost/",
  );
  const target = new URL(url, base);
  const method = (options.init?.method ?? "GET").toUpperCase();
  const local =
    target.origin === base.origin &&
    ["http:", "https:"].includes(target.protocol) &&
    ["GET", "HEAD"].includes(method);
  if (!local) return consume(await fetcher(url, options.init));
  const lifetime = policy;
  const signals = [options.init?.signal, lifetime.signal].filter(
    (signal): signal is AbortSignal => !!signal,
  );
  const signal = signals.length > 1 ? AbortSignal.any(signals) : signals[0];
  const deadline = performance.now() + 60_000;
  for (let attempt = 1; attempt <= 3; attempt++) {
    abort(signal);
    const timeout = new AbortController();
    const remaining = Math.max(
      1,
      Math.min(30_000, deadline - performance.now()),
    );
    const timer = setTimeout(
      () =>
        timeout.abort(
          new DOMException("The download timed out.", "TimeoutError"),
        ),
      remaining,
    );
    const attemptSignal = signal
      ? AbortSignal.any([signal, timeout.signal])
      : timeout.signal;
    let response: Response | undefined;
    try {
      response = await fetcher(url, { ...options.init, signal: attemptSignal });
      if (retryableStatus.has(response.status))
        throw new TemporaryResponse(response.status);
      const value = await consume(response);
      abort(attemptSignal);
      return value;
    } catch (error) {
      abort(signal);
      if (response && !response.ok && !retryableStatus.has(response.status))
        throw error;
      const temporary =
        error instanceof TemporaryResponse ||
        error instanceof AssetIntegrityError ||
        error instanceof TypeError ||
        timeout.signal.aborted ||
        (error instanceof DOMException && error.name === "NetworkError");
      if (!temporary) throw error;
      await response?.body?.cancel().catch(() => {});
      const status =
        error instanceof TemporaryResponse ? error.status : response?.status;
      if (attempt === 3 || performance.now() >= deadline) {
        console.warn("[asset-download]", {
          path: target.pathname,
          attempt,
          status,
          failure: error instanceof Error ? error.name : "NetworkError",
        });
        if (error instanceof AssetIntegrityError) throw error;
        throw new AssetDownloadError(target.pathname, attempt, status);
      }
      lifetime.onRetry?.({ path: target.pathname, attempt, status });
    } finally {
      clearTimeout(timer);
    }
    await pause(
      Math.min(backoff[attempt - 1], Math.max(0, deadline - performance.now())),
      signal,
    );
  }
  throw new AssetDownloadError(target.pathname, 3);
}
