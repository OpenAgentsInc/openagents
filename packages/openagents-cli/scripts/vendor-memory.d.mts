// The types `test/vendored-memory-drift.test.ts` imports from the vendor
// script. The script itself stays plain `.mjs` — it is build tooling, run by
// `node` and not compiled — but importing it from a checked test file left the
// module implicitly `any` (TS7016), which is a hole in exactly the guard that
// exists to stop the vendored tree drifting silently.

/** Each vendored file: source package, destination path, and its rewrites. */
export declare const VENDORED: ReadonlyArray<
  readonly [string, string, ReadonlyArray<readonly [string, string]>]
>;

/** Apply one file's rewrites, the single definition of the transform. */
export declare const renderVendored: (
  source: string,
  rewrites: ReadonlyArray<readonly [string, string]>,
) => string;
