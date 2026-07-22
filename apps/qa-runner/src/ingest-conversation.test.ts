// Tests for local conversation-id resolution + trajectory building + the
// dry-run CLI path (issue: local-conversation -> public /trace/{uuid} ingest).
//
// Each source is materialized under a throwaway temp HOME so the tests never
// depend on the developer's real ~/.claude, ~/.codex, or desktop store.

import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  ATIF_PINNED_SCHEMA_VERSION,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from "@openagentsinc/atif/trace";

import { ConversationNotFoundError, resolveConversation } from "./conversation-source";
import { runIngestConversationCli } from "./ingest-conversation-cli";
import {
  buildTrajectoryFromConversationId,
  capTrajectorySteps,
} from "./ingest-conversation";
import { convertOpenAgentsConversationToAtif } from "./openagents-conversation-to-atif";

const CLAUDE_ID = "9c3062b0-60eb-49ba-b64f-e421b374310f";
const CODEX_ID = "019f0295-36b9-7e91-aae0-e1c5c4b05406";
const OA_ID = "C292D324-2BD7-4355-8B53-8D483151F04A";
// A Full Auto isolated-host run-thread id, stored in `<userData>/threads.json`.
const FA_THREAD_ID = "72d6ef5c-cc29-4472-bf5b-534632728184";

/** A throwaway `userData` dir holding a Full Auto `threads.json`. */
const makeUserData = (): string => {
  const dir = mkdtempSync(join(tmpdir(), "oa-fa-userdata-"));
  writeFileSync(
    join(dir, "threads.json"),
    JSON.stringify({
      version: 1,
      threads: [
        {
          id: FA_THREAD_ID,
          title: "NIP-34 GitReply (Full Auto Stage 0a dogfood)",
          notes: [
            { role: "system", text: "Full Auto run started.", timestamp: "02:26 PM" },
            { role: "user", text: "Execute the mission packet.", timestamp: "02:26 PM" },
            { role: "assistant", text: "Read the repo and opened a PR.", timestamp: "02:30 PM" },
          ],
        },
      ],
    }),
  );
  return dir;
};

const CLAUDE_JSONL = [
  JSON.stringify({
    type: "user",
    uuid: "u1",
    sessionId: CLAUDE_ID,
    message: { role: "user", content: "Fix the failing login test." },
  }),
  JSON.stringify({
    type: "assistant",
    uuid: "a1",
    sessionId: CLAUDE_ID,
    message: {
      role: "assistant",
      model: "claude-opus-4-8",
      content: [{ type: "text", text: "On it — reading the test." }],
    },
  }),
  "",
].join("\n");

const codexFixture = (): string =>
  readFileSync(new URL("./fixtures/codex-rollout.sample.jsonl", import.meta.url), "utf8");

/** Build a temp HOME populated with the requested sources. */
const makeHome = (
  sources: ReadonlyArray<"claude" | "codex" | "openagents">,
): string => {
  const home = mkdtempSync(join(tmpdir(), "oa-trace-home-"));
  if (sources.includes("claude")) {
    const dir = join(home, ".claude", "projects", "-tmp-project");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, `${CLAUDE_ID}.jsonl`), CLAUDE_JSONL);
  }
  if (sources.includes("codex")) {
    const dir = join(home, ".codex", "sessions", "2026", "06", "26");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, `rollout-2026-06-26T01-19-21-${CODEX_ID}.jsonl`),
      codexFixture(),
    );
  }
  if (sources.includes("openagents")) {
    const dir = join(home, "Library", "Application Support", "openagents", "KhalaDesktop");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, "conversations.json"),
      JSON.stringify([
        {
          id: OA_ID,
          title: "Fix the login flow",
          createdAt: "2026-07-19T00:00:00.000Z",
          messages: [
            { role: "user", content: "Fix the failing login test." },
            { role: "assistant", content: [{ type: "text", text: "Patching now." }] },
          ],
        },
      ]),
    );
  }
  return home;
};

describe("resolveConversation", () => {
  it("resolves a Claude session file by id", () => {
    const home = makeHome(["claude"]);
    const r = resolveConversation(CLAUDE_ID, "auto", { home });
    expect(r.kind).toBe("claude");
    expect(r.path.endsWith(`${CLAUDE_ID}.jsonl`)).toBe(true);
  });

  it("resolves a Codex rollout file by id", () => {
    const home = makeHome(["codex"]);
    const r = resolveConversation(CODEX_ID, "auto", { home });
    expect(r.kind).toBe("codex");
    expect(r.path).toContain(CODEX_ID);
  });

  it("resolves an OpenAgents desktop conversation (case-insensitive)", () => {
    const home = makeHome(["openagents"]);
    const r = resolveConversation(OA_ID.toLowerCase(), "auto", { home });
    expect(r.kind).toBe("openagents");
    if (r.kind === "openagents") expect(r.conversation.title).toBe("Fix the login flow");
  });

  it("throws ConversationNotFoundError for an unknown id", () => {
    const home = makeHome([]);
    expect(() => resolveConversation("nope", "auto", { home })).toThrow(
      ConversationNotFoundError,
    );
  });

  it("resolves a Full Auto host thread from --user-data threads.json", () => {
    const home = makeHome([]);
    const userData = makeUserData();
    const r = resolveConversation(FA_THREAD_ID, "openagents", { home, userData });
    expect(r.kind).toBe("openagents");
    if (r.kind === "openagents") {
      expect(r.path.endsWith("threads.json")).toBe(true);
      expect(r.conversation.title).toContain("NIP-34 GitReply");
      expect(Array.isArray(r.conversation.messages)).toBe(true);
    }
  });

  it("does not see the threads.json id without --user-data (default unchanged)", () => {
    const home = makeHome([]);
    expect(() => resolveConversation(FA_THREAD_ID, "openagents", { home })).toThrow(
      ConversationNotFoundError,
    );
  });
});

describe("buildTrajectoryFromConversationId", () => {
  for (const source of ["claude", "codex", "openagents"] as const) {
    it(`builds a valid ATIF-v1.7 trajectory from a ${source} source`, () => {
      const home = makeHome([source]);
      const id = source === "claude" ? CLAUDE_ID : source === "codex" ? CODEX_ID : OA_ID;
      const { resolved, trajectory } = buildTrajectoryFromConversationId(id, { home });
      expect(resolved.kind).toBe(source);
      expect(trajectory.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
      expect(trajectory.steps.length).toBeGreaterThan(0);
      expect(validateAtifTrajectory(decodeAtifTrajectorySync(trajectory))).toEqual([]);
    });
  }

  it("builds a valid, tripwire-clean ATIF from a Full Auto thread via --user-data", () => {
    const home = makeHome([]);
    const userData = makeUserData();
    const { resolved, trajectory } = buildTrajectoryFromConversationId(FA_THREAD_ID, {
      kind: "openagents",
      home,
      userData,
    });
    expect(resolved.kind).toBe("openagents");
    const strict = decodeAtifTrajectorySync(trajectory);
    expect(trajectory.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
    expect(trajectory.steps.length).toBe(3);
    expect(validateAtifTrajectory(strict)).toEqual([]);
    expect(atifTraceTripwire(strict)).toEqual([]);
  });
});

describe("capTrajectorySteps", () => {
  const bigTrajectory = () =>
    convertOpenAgentsConversationToAtif({
      id: "big",
      messages: Array.from({ length: 10 }, (_, i) => ({
        role: i % 2 === 0 ? "user" : "assistant",
        content: `message ${i + 1}`,
      })),
    });

  it("keeps a valid, sequential prefix and notes the truncation", () => {
    const capped = capTrajectorySteps(bigTrajectory(), 4);
    expect(capped.steps.length).toBe(4);
    expect(capped.steps.map((s) => s.step_id)).toEqual([1, 2, 3, 4]);
    expect(capped.notes).toContain("Truncated to the first 4");
    expect(validateAtifTrajectory(decodeAtifTrajectorySync(capped))).toEqual([]);
  });

  it("returns the trajectory unchanged when within the cap", () => {
    const t = bigTrajectory();
    expect(capTrajectorySteps(t, 100)).toBe(t);
  });
});

describe("runIngestConversationCli --dry-run", () => {
  it("writes a redacted, valid ATIF file and does not upload", async () => {
    const home = makeHome(["claude"]);
    const out = join(mkdtempSync(join(tmpdir(), "oa-trace-out-")), "trajectory.json");
    const lines: string[] = [];
    const code = await runIngestConversationCli(
      [CLAUDE_ID, "--home", home, "--dry-run", "--out", out, "--json"],
      (m) => lines.push(m),
    );
    expect(code).toBe(0);
    expect(existsSync(out)).toBe(true);
    const parsed = JSON.parse(readFileSync(out, "utf8"));
    expect(parsed.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
    // The strict schema still decodes the written body.
    expect(() => decodeAtifTrajectorySync(parsed)).not.toThrow();
    // No fabricated uuid/url in a dry run.
    expect(lines.join("\n")).not.toContain("/trace/");
  });

  it("exits non-zero for an unknown id", async () => {
    const home = makeHome([]);
    const code = await runIngestConversationCli(["missing", "--home", home, "--dry-run"], () => {});
    expect(code).toBe(1);
  });

  it("ingests a Full Auto host thread via --source openagents --user-data", async () => {
    const home = makeHome([]);
    const userData = makeUserData();
    const out = join(mkdtempSync(join(tmpdir(), "oa-fa-out-")), "trajectory.json");
    const lines: string[] = [];
    const code = await runIngestConversationCli(
      [
        FA_THREAD_ID,
        "--source",
        "openagents",
        "--home",
        home,
        "--user-data",
        userData,
        "--dry-run",
        "--out",
        out,
      ],
      (m) => lines.push(m),
    );
    expect(code).toBe(0);
    expect(existsSync(out)).toBe(true);
    const parsed = JSON.parse(readFileSync(out, "utf8"));
    expect(parsed.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
    expect(() => decodeAtifTrajectorySync(parsed)).not.toThrow();
    expect(lines.join("\n")).not.toContain("/trace/");
  });
});
