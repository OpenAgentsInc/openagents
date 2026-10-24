import assert from "node:assert/strict";
import test from "node:test";
import {
  readAsset,
  AssetIntegrityError,
  AssetDownloadError,
  configureAssetDownloads,
} from "../packages/assets/src/asset-download.ts";

test("a broken response body is discarded and the complete next response is read", async () => {
  let requests = 0;
  const notices: unknown[] = [];
  const stop = configureAssetDownloads({ onRetry: (n) => notices.push(n) });
  try {
    const bytes = await readAsset(
      "/private-assets/file.bin",
      (r) => r.arrayBuffer(),
      {
        fetcher: async () => {
          requests++;
          if (requests === 1) {
            let pulls = 0;
            return new Response(
              new ReadableStream({
                pull(c) {
                  if (pulls++ === 0) c.enqueue(new Uint8Array([99]));
                  else c.error(new TypeError("NetworkError"));
                },
              }),
            );
          }
          return new Response(new Uint8Array([1, 2, 3]));
        },
      },
    );
    assert.deepEqual([...new Uint8Array(bytes)], [1, 2, 3]);
    assert.equal(requests, 2);
    assert.equal(notices.length, 1);
  } finally {
    stop();
  }
});

test("a resolved but truncated body retries through its declared integrity check", async () => {
  let requests = 0;
  const value = await readAsset(
    "/private-assets/file.bin",
    async (r) => {
      const bytes = await r.arrayBuffer();
      if (bytes.byteLength !== 3)
        throw new AssetIntegrityError("The file is incomplete.");
      return bytes;
    },
    {
      fetcher: async () =>
        new Response(new Uint8Array(++requests === 1 ? [1] : [1, 2, 3])),
    },
  );
  assert.equal(value.byteLength, 3);
  assert.equal(requests, 2);
});

test("transient HTTP failure retries, while missing files and validation errors do not", async () => {
  let calls = 0;
  const result = await readAsset("/private-assets/file.bin", (r) => r.text(), {
    fetcher: async () =>
      ++calls === 1
        ? new Response("busy", { status: 503 })
        : new Response("complete"),
  });
  assert.equal(result, "complete");
  assert.equal(calls, 2);
  calls = 0;
  await assert.rejects(
    readAsset(
      "/private-assets/missing.bin",
      async (r) => {
        if (!r.ok) throw Error("Missing file");
        return r.text();
      },
      {
        fetcher: async () => {
          calls++;
          return new Response("missing", { status: 404 });
        },
      },
    ),
    /Missing file/,
  );
  assert.equal(calls, 1);
  calls = 0;
  await assert.rejects(
    readAsset("/private-assets/missing.bin", (r) => r.arrayBuffer(), {
      fetcher: async () => {
        calls++;
        return new Response(
          new ReadableStream({
            start(controller) {
              controller.error(new TypeError("NetworkError"));
            },
          }),
          { status: 404 },
        );
      },
    }),
    TypeError,
  );
  assert.equal(
    calls,
    1,
    "A known404 cannot become retryable because its body fails.",
  );
  calls = 0;
  await assert.rejects(
    readAsset(
      "/private-assets/large.bin",
      async () => {
        throw Error("Byte limit");
      },
      {
        fetcher: async () => {
          calls++;
          return new Response("large");
        },
      },
    ),
    /Byte limit/,
  );
  assert.equal(calls, 1);
});

test("cancellation stops both an active read and pending retry without a new request", async () => {
  const controller = new AbortController();
  let requests = 0;
  const stop = configureAssetDownloads({
    signal: controller.signal,
    onRetry: () => controller.abort(),
  });
  try {
    await assert.rejects(
      readAsset("/private-assets/file.bin", (r) => r.arrayBuffer(), {
        fetcher: async () => {
          requests++;
          throw new TypeError("NetworkError");
        },
      }),
      (e) => e instanceof DOMException && e.name === "AbortError",
    );
    assert.equal(requests, 1);
  } finally {
    stop();
  }
  requests = 0;
  await assert.rejects(
    readAsset("/private-assets/file.bin", (r) => r.arrayBuffer(), {
      fetcher: async () => {
        requests++;
        throw new DOMException("Cancelled", "AbortError");
      },
    }),
    (e) => e instanceof DOMException && e.name === "AbortError",
  );
  assert.equal(requests, 1);
});

test("closing the world during a body read cancels that read without retry", async () => {
  const controller = new AbortController();
  let requests = 0;
  const stop = configureAssetDownloads({ signal: controller.signal });
  try {
    await assert.rejects(
      readAsset("/private-assets/file.bin", (r) => r.arrayBuffer(), {
        fetcher: async (_url, init) => {
          requests++;
          return new Response(
            new ReadableStream({
              start(stream) {
                init!.signal!.addEventListener(
                  "abort",
                  () => stream.error(init!.signal!.reason),
                  { once: true },
                );
                queueMicrotask(() => controller.abort());
              },
            }),
          );
        },
      }),
      (e) => e instanceof DOMException && e.name === "AbortError",
    );
    assert.equal(requests, 1);
  } finally {
    stop();
  }
});

test("an explicit candidate signal cannot replace the worker download lifetime", async () => {
  const worker = new AbortController(),
    candidate = new AbortController();
  let requests = 0;
  const stop = configureAssetDownloads({
    signal: worker.signal,
    onRetry: () => worker.abort(),
  });
  try {
    await assert.rejects(
      readAsset("/private-assets/file.bin", (r) => r.arrayBuffer(), {
        init: { signal: candidate.signal },
        fetcher: async () => {
          requests++;
          throw new TypeError("NetworkError");
        },
      }),
      (e) => e instanceof DOMException && e.name === "AbortError",
    );
    assert.equal(requests, 1);
    assert.equal(candidate.signal.aborted, false);
  } finally {
    stop();
  }
});

test("exhausted local retries produce a human error and omit query credentials", async () => {
  let requests = 0;
  await assert.rejects(
    readAsset(
      "/private-assets/file.bin?token=private",
      (r) => r.arrayBuffer(),
      {
        fetcher: async () => {
          requests++;
          throw new TypeError("NetworkError");
        },
      },
    ),
    (e) =>
      e instanceof AssetDownloadError &&
      e.attempts === 3 &&
      e.path === "/private-assets/file.bin" &&
      e.message === "A world file could not finish downloading. Try again.",
  );
  assert.equal(requests, 3);
});

test("cross-origin requests and writes retain their single-attempt behavior", async () => {
  for (const [url, init] of [
    ["https://other.example/file.bin", undefined],
    ["/private-assets/file.bin", { method: "POST" }],
  ] as const) {
    let requests = 0;
    await assert.rejects(
      readAsset(url, (r) => r.text(), {
        init,
        fetcher: async () => {
          requests++;
          throw new TypeError("NetworkError");
        },
      }),
      TypeError,
    );
    assert.equal(requests, 1);
  }
});
