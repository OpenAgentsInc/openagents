// gh-attach integration (#6185): upload artifacts and get embeddable markdown.
//
// `ain3sh/gh-attach` is a Go binary that uploads images/videos to a GitHub
// issue/PR via the WEB upload path the REST API lacks, and prints embeddable
// markdown (or a URL/JSON). We adopt the binary directly (runtime-agnostic, no
// Factory coupling). In CI the binary may be UNAVAILABLE or UNAUTHENTICATED
// (it reads browser cookies); this module degrades HONESTLY in that case —
// returning no embed so the PR comment falls back to the in-eval relative video
// ref, never a broken/fake embed.
//
// PURE/TESTABLE: the binary invocation is injected (`run`), so the composition
// is unit-tested with a fake runner — no real network, no real upload.

export interface GhAttachRunner {
  /** Invoke gh-attach with args; resolve stdout, or reject/return null-ish on
   *  failure. Returns `{ ok, stdout }` so a non-zero exit is HONEST, not thrown. */
  readonly run: (
    args: ReadonlyArray<string>,
  ) => Promise<{ ok: boolean; stdout: string }>;
}

export interface GhAttachOptions {
  /** owner/repo target for the upload (gh-attach `--repo`). */
  readonly repo?: string;
}

// Upload a single file and return the embeddable markdown gh-attach prints
// (`--auto`: `![name](url)` for images, raw URL otherwise). Returns null when
// the binary is unavailable/unauthenticated or prints nothing — the caller then
// falls back to the relative artifact ref (honest, never a broken embed).
export const ghAttachUpload = async (
  runner: GhAttachRunner,
  filePath: string,
  options: GhAttachOptions = {},
): Promise<string | null> => {
  const args = [
    '--auto',
    ...(options.repo !== undefined ? ['--repo', options.repo] : []),
    '--',
    filePath,
  ];
  let result: { ok: boolean; stdout: string };
  try {
    result = await runner.run(args);
  } catch {
    return null;
  }
  if (!result.ok) return null;
  const md = result.stdout.trim();
  return md.length === 0 ? null : md;
};

// Upload each variant's video (keyed by variant id) and return a map of the
// embeddable markdown per variant. Missing/failed uploads are simply absent
// from the map — the markdown renderer then shows the relative video ref.
export const ghAttachVariantVideos = async (
  runner: GhAttachRunner,
  videos: ReadonlyArray<{ variantId: string; filePath: string }>,
  options: GhAttachOptions = {},
): Promise<Record<string, string>> => {
  const out: Record<string, string> = {};
  for (const v of videos) {
    const md = await ghAttachUpload(runner, v.filePath, options);
    if (md !== null) out[v.variantId] = md;
  }
  return out;
};

// A real runner backed by a spawned process. Kept tiny + injectable so the
// composition stays unit-tested with a fake. `spawn` is passed in so this file
// has no hard node:child_process import in the test path.
export const makeProcessRunner = (
  spawn: (
    cmd: string,
    args: ReadonlyArray<string>,
  ) => Promise<{ exitCode: number; stdout: string }>,
  binary = 'gh-attach',
): GhAttachRunner => ({
  run: async args => {
    const { exitCode, stdout } = await spawn(binary, args);
    return { ok: exitCode === 0, stdout };
  },
});
