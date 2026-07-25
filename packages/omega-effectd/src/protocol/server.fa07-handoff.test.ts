/**
 * FA-07 gate 5 (omega#26) — a cross-provider handoff must be VISIBLE.
 *
 * `server.test.ts` already proves the handoff is CORRECT: the lane rebinds, the
 * thread splits, the transition record is durable, and the free-text reason is
 * redacted. None of that is what gate 5 asks for. Gate 5 asks for "one visible
 * cross-provider handoff ... with sidebar/transcript evidence" — that an owner
 * reading the thread can tell a different model took over.
 *
 * That property was FALSE on the Omega framed path. `full-auto-run-actions.ts`
 * emits `Provider handoff: <from> → <to> (<disposition>)` against the new
 * target thread, and this server bound `appendSystemNote` to `() => {}`. The
 * handoff was durable, correct, and completely silent: a different provider
 * began spending the owner's budget and the transcript said nothing. The
 * Electron control-API path did not have this defect, which is why every
 * existing handoff test passed.
 *
 * The tests below assert delivery to the HOST, because the host is what renders
 * a transcript. Asserting the registry again would re-prove the half that was
 * never broken.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA, type OmegaEffectdHostRequest } from "./framed.ts";

const CODEX_LANE = "codex-local";
const CLAUDE_LANE = "claude-local";

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-fa07-handoff-"));
  try {
    await fn(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  });

type Note = Readonly<{ threadRef: string; text: string }>;

/**
 * A host that records the notes it is asked to render.
 *
 * `makeOmegaEffectdTestHost` answers `append_system_note` and throws the
 * content away, so a test built on it cannot tell a delivered note from a
 * dropped one — which is how the defect survived. This host keeps them.
 */
const makeRecordingHost = (readyLanes: ReadonlyArray<string>) => {
  const notes: Array<Note> = [];
  const threadLanes = new Map<string, string>();
  let threadCounter = 0;
  const ready = new Set(readyLanes);
  const handler = async (hostRequest: OmegaEffectdHostRequest): Promise<unknown> => {
    const params = (hostRequest.params ?? {}) as Record<string, unknown>;
    switch (hostRequest.method) {
      case "resolve_workspace":
        return { workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised" };
      case "resolve_sync_session":
        return { available: false };
      case "lane_readiness": {
        const admitted = ready.has(String(params.lane));
        return {
          known: true,
          admitted,
          fullAuto: admitted,
          state: admitted ? "available" : "unavailable",
        };
      }
      case "create_thread": {
        const threadRef = `thread.handoff.${(threadCounter += 1)}`;
        threadLanes.set(threadRef, String(params.lane ?? CODEX_LANE));
        return { threadRef };
      }
      case "refresh_evidence":
        return { present: true, revision: 1, live: null, turns: [] };
      case "dispatch_turn":
        return { accepted: true };
      case "interrupt_turn":
        return { interrupted: true };
      case "append_system_note":
        notes.push({ threadRef: String(params.threadRef), text: String(params.text ?? "") });
        return { appended: true };
      default:
        return undefined;
    }
  };
  return { handler, notes, threadLanes };
};

const startServer = async (root: string, readyLanes: ReadonlyArray<string>) => {
  const host = makeRecordingHost(readyLanes);
  const service = createOmegaEffectdService({ paths: { dataRoot: root } });
  const server = createOmegaEffectdFramedServer(
    service,
    { dataRoot: root },
    { hostRequestHandler: host.handler },
  );
  await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
  return { server, host };
};

const runOf = (frame: { result?: unknown } | null): Record<string, unknown> =>
  ((frame?.result as Record<string, unknown>)?.run ?? {}) as Record<string, unknown>;

const startPausedRun = async (
  server: Awaited<ReturnType<typeof startServer>>["server"],
  objective: string,
) => {
  const started = await server.handleLine(
    request("start", 1, "start", {
      workspaceRef: "workspace.omega.supervised",
      title: "FA-07 handoff visibility",
      objective,
      doneCondition: "FA07_HANDOFF_DONE_CONDITION",
      lane: CODEX_LANE,
      turnCap: 6,
      projectRef: "project.fa07.handoff",
      worktreeRef: "worktree.fa07.handoff",
    }),
  );
  const runRef = String(runOf(started).runRef);
  await server.handleLine(request("pause", 1, "pause", { runRef }));
  return runRef;
};

describe("FA-07 gate 5: a cross-provider handoff is visible in the transcript", () => {
  test("the handoff writes a note the host can render, addressed to the target thread", async () => {
    await withRoot(async (root) => {
      const { server, host } = await startServer(root, [CODEX_LANE, CLAUDE_LANE]);
      const runRef = await startPausedRun(server, "FA07_HANDOFF_OBJECTIVE");

      const notesBefore = host.notes.length;
      const handoff = await server.handleLine(
        request("handoff", 1, "handoff", {
          runRef,
          targetLaneRef: CLAUDE_LANE,
          reason: "gate 5 visibility",
        }),
      );
      expect(handoff?.ok).toBe(true);
      const transition = (handoff?.result as { transition: Record<string, unknown> }).transition;

      // The note must arrive by the time the caller is answered. A note that
      // lands on some later unrelated reconciliation is not evidence a person
      // reading the thread right now would see.
      const delivered = host.notes.slice(notesBefore);
      const handoffNotes = delivered.filter((note) => note.text.startsWith("Provider handoff:"));
      expect(handoffNotes).toHaveLength(1);

      const note = handoffNotes[0]!;
      expect(note.text).toContain(CODEX_LANE);
      expect(note.text).toContain(CLAUDE_LANE);
      expect(note.text).toContain("complete_within_bounds");
      // Addressed to the NEW thread: the owner reads the thread the new
      // provider is now writing into, not the one Codex left behind.
      expect(note.threadRef).toBe(transition.targetThreadRef);
      expect(transition.sourceThreadRef).not.toBe(transition.targetThreadRef);
    });
  });

  test("the visible note never carries the objective or the caller's free-text reason", async () => {
    await withRoot(async (root) => {
      const { server, host } = await startServer(root, [CODEX_LANE, CLAUDE_LANE]);
      const objective = "FA07_HANDOFF_SECRET_OBJECTIVE_MUST_NOT_REACH_THE_TRANSCRIPT";
      const runRef = await startPausedRun(server, objective);

      await server.handleLine(
        request("handoff", 1, "handoff", {
          runRef,
          targetLaneRef: CLAUDE_LANE,
          reason: "switching because sk-live-000000000000000000 was rate limited",
        }),
      );
      const note = host.notes.find((entry) => entry.text.startsWith("Provider handoff:"));
      expect(note).toBeDefined();
      // Making the handoff visible must not make it a leak. The note names the
      // lanes and the disposition; it is not a place to echo owner text or a
      // credential-shaped string the caller supplied.
      expect(note?.text).not.toContain(objective);
      expect(note?.text).not.toContain("sk-live-");
    });
  });

  test("a refused handoff writes no note, so the transcript cannot claim a switch that did not happen", async () => {
    await withRoot(async (root) => {
      // Claude is NOT authenticated on this host, so the target lane fails
      // admission and the run keeps its original provider.
      const { server, host } = await startServer(root, [CODEX_LANE]);
      const runRef = await startPausedRun(server, "FA07_HANDOFF_REFUSED_OBJECTIVE");

      const notesBefore = host.notes.length;
      const refused = await server.handleLine(
        request("handoff", 1, "handoff", { runRef, targetLaneRef: CLAUDE_LANE }),
      );
      expect(refused?.ok).toBe(false);

      const delivered = host.notes
        .slice(notesBefore)
        .filter((note) => note.text.startsWith("Provider handoff:"));
      expect(delivered).toHaveLength(0);

      const detail = runOf(await server.handleLine(request("detail", 1, "get_run", { runRef })));
      expect(detail.lane).toBe(CODEX_LANE);
    });
  });
});
