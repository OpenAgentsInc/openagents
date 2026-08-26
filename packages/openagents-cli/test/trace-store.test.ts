import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  defaultDiscoveryBounds,
  defaultTraceStores,
  pathTraceStore,
  redactedPathFor,
  redactionRules,
  redactText,
  redactTraceFile,
  scanTraceStore,
  summarizeTraceFile,
} from "../src/trace-store.js";

const scratch = () => mkdtempSync(join(tmpdir(), "trace-store-"));

const writeAt = (path: string, content: string, at: Date) => {
  writeFileSync(path, content, "utf8");
  utimesSync(path, at, at);
};

describe("trace discovery", () => {
  it("names the three default stores under the home directory", () => {
    const stores = defaultTraceStores("/home/octavia");
    expect(stores.map((store) => store.kind)).toEqual([
      "openagents_export",
      "claude_session",
      "codex_session",
    ]);
    expect(stores[0]?.root).toBe("/home/octavia/.openagents/exports");
    expect(stores[1]?.root).toBe("/home/octavia/.claude/projects");
    expect(stores[2]?.root).toBe("/home/octavia/.codex/sessions");
  });

  it("reports a missing store as absent instead of failing", () => {
    const { scan, candidates } = scanTraceStore(pathTraceStore(join(scratch(), "nope")));
    expect(scan.present).toBe(false);
    expect(candidates).toEqual([]);
  });

  it("lists matching files newest first with metadata only", () => {
    const root = scratch();
    writeAt(join(root, "old.json"), "{}", new Date("2026-08-01T00:00:00Z"));
    writeAt(join(root, "new.json"), "{}", new Date("2026-08-20T00:00:00Z"));
    writeAt(join(root, "ignored.txt"), "not a trace", new Date("2026-08-21T00:00:00Z"));

    const { scan, candidates } = scanTraceStore(pathTraceStore(root));
    expect(scan).toMatchObject({ present: true, matched: 2, listed: 2, skipped_symlinks: 0 });
    expect(candidates.map((candidate) => candidate.path)).toEqual([
      join(root, "new.json"),
      join(root, "old.json"),
    ]);
    expect(candidates[0]).toMatchObject({
      kind: "trace_path",
      bytes: 2,
      modified_at: "2026-08-20T00:00:00.000Z",
    });
  });

  it("caps the listing while still counting every match", () => {
    const root = scratch();
    for (let index = 0; index < 5; index += 1) {
      writeAt(join(root, `t${index}.json`), "{}", new Date(2026, 0, index + 1));
    }
    const { scan, candidates } = scanTraceStore(pathTraceStore(root), {
      ...defaultDiscoveryBounds,
      maxFilesPerStore: 2,
    });
    expect(scan.matched).toBe(5);
    expect(scan.listed).toBe(2);
    expect(candidates).toHaveLength(2);
  });

  it("stops at the depth bound", () => {
    const root = scratch();
    const shallow = join(root, "a");
    const deep = join(root, "a", "b");
    mkdirSync(deep, { recursive: true });
    writeFileSync(join(shallow, "shallow.json"), "{}", "utf8");
    writeFileSync(join(deep, "deep.json"), "{}", "utf8");

    const { candidates } = scanTraceStore(pathTraceStore(root), {
      ...defaultDiscoveryBounds,
      maxDepth: 1,
    });
    expect(candidates.map((candidate) => candidate.path)).toEqual([join(shallow, "shallow.json")]);
  });

  it("stops at the entry budget and says the scan was truncated", () => {
    const root = scratch();
    for (let index = 0; index < 10; index += 1) {
      writeFileSync(join(root, `t${index}.json`), "{}", "utf8");
    }
    const { scan } = scanTraceStore(pathTraceStore(root), {
      ...defaultDiscoveryBounds,
      maxScanEntries: 3,
    });
    expect(scan.truncated).toBe(true);
    expect(scan.matched).toBeLessThanOrEqual(3);
  });

  it("never follows a symlink, whether file or directory", () => {
    const outside = scratch();
    writeFileSync(join(outside, "secret.json"), "{}", "utf8");
    const root = scratch();
    writeFileSync(join(root, "real.json"), "{}", "utf8");
    symlinkSync(join(outside, "secret.json"), join(root, "linked-file.json"));
    symlinkSync(outside, join(root, "linked-dir"));

    const { scan, candidates } = scanTraceStore(pathTraceStore(root));
    expect(candidates.map((candidate) => candidate.path)).toEqual([join(root, "real.json")]);
    expect(scan.skipped_symlinks).toBe(2);
  });

  it("refuses a store whose root is itself a symlink", () => {
    const target = scratch();
    writeFileSync(join(target, "trace.json"), "{}", "utf8");
    const holder = scratch();
    const link = join(holder, "linked-root");
    symlinkSync(target, link);

    const { scan, candidates } = scanTraceStore(pathTraceStore(link));
    expect(scan.present).toBe(false);
    expect(scan.skipped_symlinks).toBe(1);
    expect(candidates).toEqual([]);
  });
});

const atifDocument = {
  schema_version: "ATIF-v1.7",
  session_id: "openagents.com-2026-08-24T14:00:00.000Z",
  agent: { name: "openagents-coder", version: "0.3.5", model_name: "Ollama qwen" },
  steps: [
    { step_id: 1, timestamp: "2026-08-24T14:00:00.000Z", source: "user", message: "hello" },
    {
      step_id: 2,
      timestamp: "2026-08-24T14:00:05.000Z",
      source: "agent",
      message: "",
      model_name: "Ollama qwen",
      metrics: { prompt_tokens: 11, completion_tokens: 7 },
      tool_calls: [
        { tool_call_id: "call-1", function_name: "shell", arguments: { command: "ls" } },
      ],
      observation: { results: [{ source_call_id: "call-1", content: "README.md" }] },
    },
    {
      step_id: 3,
      timestamp: "2026-08-24T14:00:09.000Z",
      source: "agent",
      message: "done",
      model_name: "Ollama qwen",
      metrics: { prompt_tokens: 9, completion_tokens: 3 },
    },
  ],
  final_metrics: { total_prompt_tokens: 20, total_completion_tokens: 10, total_steps: 3 },
};

describe("trace summarization", () => {
  it("summarizes an ATIF document without dumping payloads", () => {
    const root = scratch();
    const path = join(root, "session-atif.json");
    writeFileSync(path, JSON.stringify(atifDocument), "utf8");

    const summary = summarizeTraceFile(path);
    expect(summary).toMatchObject({
      format: "atif",
      schema_version: "ATIF-v1.7",
      session_id: "openagents.com-2026-08-24T14:00:00.000Z",
      agent: { name: "openagents-coder", model: "Ollama qwen" },
      steps: 3,
      steps_by_source: { user: 1, agent: 2 },
      models: ["Ollama qwen"],
      tool_calls: 1,
      total_prompt_tokens: 20,
      total_completion_tokens: 10,
      first_timestamp: "2026-08-24T14:00:00.000Z",
      last_timestamp: "2026-08-24T14:00:09.000Z",
    });
    expect(JSON.stringify(summary)).not.toContain("README.md");
  });

  it("gives a foreign line-delimited log metadata only", () => {
    const root = scratch();
    const path = join(root, "rollout.jsonl");
    writeFileSync(path, '{"type":"message"}\n{"type":"message"}\n', "utf8");

    const summary = summarizeTraceFile(path);
    expect(summary).toMatchObject({ format: "jsonl", lines: 2 });
    expect(summary.steps).toBeUndefined();
  });
});

describe("trace redaction", () => {
  const home = "/Users/octavia";
  const rules = redactionRules(home);

  // These name the CATEGORY each shape is counted under, which the shared
  // fixture deliberately does not: the three redaction paths use different
  // category vocabularies, so the shared floor in `redaction-parity.test.ts`
  // asserts only that the secret body is gone. This list is the local check
  // that the report an operator reads names the right thing.
  const plantedSecrets: ReadonlyArray<{ category: string; text: string; secret: string }> = [
    {
      category: "bearer",
      text: "authorization: Bearer sec.ret-token.value-12345",
      secret: "sec.ret-token.value-12345",
    },
    {
      category: "provider_key",
      text: "used sk-abcdefghijklmnop1234 to call",
      secret: "sk-abcdefghijklmnop1234",
    },
    {
      category: "github_token",
      text: "pushed with ghp_abcdefghijklmnopqrst123456",
      secret: "ghp_abcdefghijklmnopqrst123456",
    },
    {
      category: "jwt",
      text: "session eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.abc123def456 expired",
      secret: "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.abc123def456",
    },
    {
      category: "env_value",
      text: "ran with DATABASE_URL=postgres://user:pw@host/db",
      secret: "postgres://user:pw@host/db",
    },
    {
      category: "env_value",
      text: `exported OPENAI_API_KEY="quoted-secret-value"`,
      secret: "quoted-secret-value",
    },
    {
      // The published BIP-39 test phrase, not anyone's seed. `openagents
      // identity` gives every machine one of these to keep, so a phrase pasted
      // into a session is now a shape a redacted export has to remove.
      category: "mnemonic",
      text: "my backup is abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about ok",
      secret: "abandon abandon",
    },
    {
      category: "private_key",
      text: "signing with nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5 today",
      secret: "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5",
    },
    // Our own token families. The `api_key` rule covers other people's
    // credentials -- sk-, ghp_, AKIA -- and stopped there, so this command
    // redacted a Stripe key and left an OpenAgents one in place while
    // reporting "Nothing matched the redaction rules."
    {
      category: "oa_token",
      text: "called the API with oa_pat_REALTOKEN456 just now",
      secret: "oa_pat_REALTOKEN456",
    },
    {
      category: "oa_agent_token",
      text: "the child ran as oa_agent_ABCDEF123456 here",
      secret: "oa_agent_ABCDEF123456",
    },
    {
      category: "x_code",
      text: "paired with oa-x-QQQQ1234 yesterday",
      secret: "oa-x-QQQQ1234",
    },
    {
      // Minted by computer pairing, and hyphenated, which every other token
      // rule stops at.
      category: "machine_token",
      text: "the machine token is smct_machine-secret today",
      secret: "smct_machine-secret",
    },
  ];

  it.each(plantedSecrets)("removes a planted $category", ({ category, secret, text }) => {
    const result = redactText(text, rules);
    expect(result.text).not.toContain(secret);
    expect(result.counts[category]).toBeGreaterThanOrEqual(1);
  });

  it("redacts secret-named JSON fields by name", () => {
    const result = redactText(
      JSON.stringify({ api_token: "abc", password: "hunter2", message: "keep me" }),
      rules,
    );
    expect(result.text).not.toContain("hunter2");
    expect(result.text).toContain("keep me");
    expect(result.counts["secret_field"]).toBe(2);
  });

  it("rewrites home paths to a tilde", () => {
    const result = redactText(`read ${home}/notes.txt and /home/friend/file`, rules);
    expect(result.text).toContain("read ~/notes.txt");
    expect(result.text).not.toContain("/home/friend");
    expect(result.counts["home_path"]).toBe(2);
  });

  it("leaves ordinary prose alone", () => {
    const result = redactText("The agent listed files and wrote a summary.", rules);
    expect(result.total).toBe(0);
    expect(result.text).toBe("The agent listed files and wrote a summary.");
  });

  it("keeps a twelve-word run of ordinary words, and keeps the public npub", () => {
    const prose =
      "the coder read every file that the reviewer had marked before the second pass began";
    const npub = "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7";
    const result = redactText(`${prose} for ${npub}`, rules);
    expect(result.text).toContain(prose);
    expect(result.text).toContain(npub);
    expect(result.counts["mnemonic"]).toBeUndefined();
    expect(result.counts["private_key"]).toBeUndefined();
  });

  it("shapes the sibling path for json, jsonl, and other names", () => {
    expect(redactedPathFor("/a/trace.json")).toBe("/a/trace.redacted.json");
    expect(redactedPathFor("/a/rollout.jsonl")).toBe("/a/rollout.redacted.jsonl");
    expect(redactedPathFor("/a/notes.txt")).toBe("/a/notes.txt.redacted.json");
  });

  it("writes a redacted sibling that still parses, and reports counts only", () => {
    const root = scratch();
    const path = join(root, "session-atif.json");
    const document = {
      ...atifDocument,
      steps: [
        {
          step_id: 1,
          timestamp: "2026-08-24T14:00:00.000Z",
          source: "user",
          message: `run with OPENAI_API_KEY=sk-abcdefghijklmnop1234 in ${home}/work`,
        },
      ],
    };
    writeFileSync(path, JSON.stringify(document), "utf8");

    const result = redactTraceFile(path, home);
    expect(result.output).toBe(join(root, "session-atif.redacted.json"));
    expect(result.valid_json).toBe(true);
    expect(result.total).toBeGreaterThanOrEqual(2);

    const written = readFileSync(result.output, "utf8");
    expect(written).not.toContain("sk-abcdefghijklmnop1234");
    expect(written).not.toContain(home);
    // The source file is never rewritten.
    expect(readFileSync(path, "utf8")).toContain("sk-abcdefghijklmnop1234");
    // The report carries counts, never the matched text.
    expect(JSON.stringify(result)).not.toContain("sk-abcdefghijklmnop1234");
  });
});
