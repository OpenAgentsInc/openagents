/**
 * The patch checker through the real boundary: the checked-in
 * `patch_check` plugin loaded from its own manifest — pure computation, no
 * mounts, so no fixture staging or manifest rewriting — and invoked with
 * JSON packets. The placement logic is unit-tested in
 * `plugins/patch-check/src/tests.rs`; this file proves the same behavior
 * holds through the WASM sandbox: a clean apply, drift reporting, and the
 * refusal envelope for a malformed diff.
 */

import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/patch-check/manifest.json", import.meta.url),
);

const load = (): LoadedPlugin => {
  const outcome = loadPluginFromManifest(MANIFEST);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

type HunkReport = {
  index: number;
  applies: boolean;
  at_line?: number;
  drift_lines?: number;
  reason?: string;
  mismatch?: string;
};

type Output = {
  applies: boolean;
  hunks: HunkReport[];
  applied_hunks: number;
  failed_hunks: number;
  preview?: string;
  preview_truncated?: boolean;
};

/** The guest envelope: `ok` on success, `refusal` as a value otherwise. */
type Envelope = { ok?: Output; refusal?: { code: string; reason: string } };

const invoke = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Envelope> => {
  const packet = new TextEncoder().encode(JSON.stringify(input));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Envelope;
};

const call = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Output> => {
  const envelope = await invoke(plugin, input);
  if (envelope.ok === undefined) throw new Error(JSON.stringify(envelope.refusal));
  return envelope.ok;
};

const DIFF =
  "--- a/greeting.txt\n" +
  "+++ b/greeting.txt\n" +
  "@@ -2,3 +2,3 @@\n" +
  " beta\n" +
  "-gamma\n" +
  "+GAMMA\n" +
  " delta\n";

describe("the patch_check plugin through the sandbox", () => {
  it("confirms a clean apply and returns the preview", async () => {
    const plugin = load();
    const out = await call(plugin, {
      diff: DIFF,
      content: "alpha\nbeta\ngamma\ndelta\nepsilon\n",
      include_preview: true,
    });

    expect(out.applies).toBe(true);
    expect(out.applied_hunks).toBe(1);
    expect(out.failed_hunks).toBe(0);
    expect(out.hunks).toHaveLength(1);
    expect(out.hunks[0]).toMatchObject({ index: 0, applies: true, at_line: 2 });
    expect(out.hunks[0]?.drift_lines).toBeUndefined();
    expect(out.preview).toBe("alpha\nbeta\nGAMMA\ndelta\nepsilon\n");
    expect(out.preview_truncated).toBeUndefined();
  });

  it("finds a drifted hunk within fuzz and reports the signed offset", async () => {
    const plugin = load();
    const out = await call(plugin, {
      diff: DIFF,
      content: "extra one\nextra two\nextra three\nalpha\nbeta\ngamma\ndelta\nepsilon\n",
    });

    expect(out.applies).toBe(true);
    expect(out.hunks[0]).toMatchObject({ applies: true, at_line: 5, drift_lines: 3 });
  });

  it("reports a hunk whose context is gone without refusing", async () => {
    const plugin = load();
    const out = await call(plugin, {
      diff: DIFF,
      content: "the file\nwas rewritten\nentirely\n",
    });

    expect(out.applies).toBe(false);
    expect(out.applied_hunks).toBe(0);
    expect(out.failed_hunks).toBe(1);
    expect(out.hunks[0]?.reason).toBe("context_not_found");
    expect(out.hunks[0]?.mismatch).toBe("beta");
    expect(out.preview).toBeUndefined();
  });

  it("refuses a malformed diff with the refusal envelope, naming the shape", async () => {
    const plugin = load();
    const envelope = await invoke(plugin, {
      diff: "this is prose, not a patch",
      content: "alpha\n",
    });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.code).toBe("unsupported");
    expect(envelope.refusal?.reason).toContain("@@ -start,count +start,count @@");
  });
});
