// Unit tests for the trace redaction service (#6219). Known-secret fixtures must
// be redacted; allowlisted values must be preserved; the engine must be
// deterministic; the report must count categories; a whole ATIF-shaped value
// must scrub through the Effect service.

import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  redactString,
  redactValue,
  TraceRedactor,
  type RedactionResult,
} from "./redaction";

const red = (s: string): RedactionResult<string> => redactString(s);

describe("redactString — known secrets are redacted", () => {
  test("OpenAI-style sk- key", () => {
    const r = red("export OPENAI_KEY then sk-abcdefghijklmnop0123456789ABCD use it");
    expect(r.value).not.toContain("sk-abcdefghijklmnop");
    expect(r.value).toContain("[REDACTED:provider_key]");
    expect(r.report.counts.provider_key).toBe(1);
  });

  test("OpenRouter sk-or- key", () => {
    const r = red("sk-or-v1-0011223344556677889900aabbccddeeff00112233");
    expect(r.value).toBe("[REDACTED:provider_key]");
    expect(r.report.counts.provider_key).toBe(1);
  });

  test("oa_agent_ token", () => {
    const r = red("token oa_agent_AbCdEf123456789xyz done");
    expect(r.value).not.toContain("oa_agent_AbCdEf");
    expect(r.report.counts.oa_agent_token).toBe(1);
  });

  test("generic oa_ token", () => {
    const r = red("auth oa_live_abcdef0123456789abcdef next");
    expect(r.value).toContain("[REDACTED:oa_token]");
    expect(r.report.counts.oa_token).toBe(1);
  });

  test("Bearer token keeps the scheme word, redacts the credential", () => {
    const r = red("Authorization: Bearer abcdef0123456789ABCDEFxyz");
    expect(r.value).toContain("Bearer [REDACTED:bearer]");
    expect(r.value).not.toContain("abcdef0123456789ABCDEFxyz");
  });

  test("JWT", () => {
    const jwt =
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    const r = red(`token=${jwt}`);
    expect(r.value).toContain("[REDACTED:jwt]");
    expect(r.value).not.toContain("SflKxwRJSMeKKF2QT4");
  });

  test("private key block", () => {
    const pem =
      "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEABBBBBBBB\nmoredata==\n-----END OPENSSH PRIVATE KEY-----";
    const r = red(`here ${pem} done`);
    expect(r.value).toContain("[REDACTED:private_key]");
    expect(r.value).not.toContain("BEGIN OPENSSH");
    expect(r.report.counts.private_key).toBe(1);
  });

  test("12-word mnemonic / seed phrase", () => {
    const r = red(
      "seed: legal winner thank year wave sausage worth useful legal winner thank yellow end",
    );
    expect(r.value).toContain("[REDACTED:mnemonic]");
    expect(r.value).not.toContain("legal winner thank");
    expect(r.report.counts.mnemonic).toBe(1);
  });

  test("24-word mnemonic", () => {
    const words = Array(24).fill("alpha").join(" ");
    const r = red(`recover ${words} now`);
    expect(r.value).toContain("[REDACTED:mnemonic]");
    expect(r.report.counts.mnemonic).toBe(1);
  });

  test(".env-style KEY=value secret keeps the key", () => {
    const r = red('MDK_TREASURY_ACCESS_TOKEN="supersecretvalue123"');
    expect(r.value).toContain("MDK_TREASURY_ACCESS_TOKEN=[REDACTED:env_secret]");
    expect(r.value).not.toContain("supersecretvalue123");
  });

  test(".secrets/ path", () => {
    const r = red("read .secrets/openagents-mdk-treasury.env for the mnemonic");
    expect(r.value).toContain("[REDACTED:secrets_path]");
    expect(r.value).not.toContain("openagents-mdk-treasury.env");
  });

  test("absolute home path redacts the username, keeps the shape", () => {
    const r = red("/Users/christopherdavid/work/openagents/file.ts");
    expect(r.value).toBe("/Users/[REDACTED:home]/work/openagents/file.ts");
    expect(r.value).not.toContain("christopherdavid");
    expect(r.report.counts.home_path).toBe(1);
  });

  test("email PII", () => {
    const r = red("contact chris@openagents.com about it");
    expect(r.value).toContain("[REDACTED:email]");
    expect(r.value).not.toContain("chris@openagents.com");
  });

  test("owner identifier github:<digits>", () => {
    const r = red("owner github:12345678 claimed it");
    expect(r.value).toContain("github:[REDACTED:owner_id]");
    expect(r.value).not.toContain("12345678");
  });

  test("X verification code oa-x-…", () => {
    const r = red("Verifying my agent. Code: oa-x-9f2bc-defG");
    expect(r.value).toContain("[REDACTED:x_code]");
    expect(r.value).not.toContain("9f2bc-defG");
  });

  test("internal/tailscale IP", () => {
    const r1 = red("ssh 100.101.102.103");
    expect(r1.value).toContain("[REDACTED:ip]");
    const r2 = red("host 192.168.1.42");
    expect(r2.value).toContain("[REDACTED:ip]");
  });

  test("long hex blob catch-all", () => {
    const r = red("digest 0123456789abcdef0123456789abcdef01234567 here");
    expect(r.value).toContain("[REDACTED:long_blob]");
  });
});

describe("allowlist — false positives are preserved", () => {
  test("openagents/khala model id is kept", () => {
    const r = red("model openagents/khala served the request");
    expect(r.value).toContain("openagents/khala");
    expect(r.report.total).toBe(0);
  });

  test("public openagents.com URL is kept", () => {
    const r = red("see https://openagents.com/trace/abc-123 for details");
    expect(r.value).toContain("https://openagents.com/trace/abc-123");
    expect(r.report.total).toBe(0);
  });

  test("public github.com/OpenAgentsInc URL is kept", () => {
    const r = red("repo https://github.com/OpenAgentsInc/openagents/issues/6219");
    expect(r.value).toContain("https://github.com/OpenAgentsInc/openagents/issues/6219");
  });

  test("public issue ref #6219 is kept", () => {
    const r = red("Closes #6219 and refs #6220");
    expect(r.value).toContain("#6219");
    expect(r.value).toContain("#6220");
    expect(r.report.total).toBe(0);
  });

  test("ATIF *_tokens numeric metric fields are untouched (numbers pass through)", () => {
    const v = { metrics: { prompt_tokens: 1234, completion_tokens: 56, cached_tokens: 7 } };
    const r = redactValue(v);
    expect(r.value).toEqual(v);
    expect(r.report.total).toBe(0);
  });

  test("an allowlisted URL embedded next to a secret keeps the URL, redacts the secret", () => {
    const r = red("visit https://openagents.com/admin with key sk-abcdefghijklmnop0123456789");
    expect(r.value).toContain("https://openagents.com/admin");
    expect(r.value).toContain("[REDACTED:provider_key]");
  });
});

describe("username leakage outside /Users/ paths", () => {
  test("slug form -Users-<name>- redacts the username", () => {
    const r = red("/private/tmp/claude-501/-Users-alice-work/session.jsonl");
    expect(r.value).not.toContain("alice");
    expect(r.value).toContain("-Users-[REDACTED:home]-");
  });

  test("explicit usernames option redacts the bare username (ls owner column)", () => {
    const r = redactString("drwxr-xr-x@ 3 alice  staff  96 Jun 24 dir", { usernames: ["alice"] });
    expect(r.value).not.toContain("alice");
    expect(r.value).toContain("[REDACTED:home]");
    expect(r.report.counts.username).toBe(1);
  });

  test("redactValue auto-derives the username from a /Users/ path elsewhere in the value", () => {
    const v = {
      a: "wrote /Users/alice/work/x.ts",
      b: "drwxr-xr-x@ 3 alice  staff  96 file", // bare username, no path
      c: "/private/tmp/-Users-alice-work/log",
    };
    const r = redactValue(v);
    const json = JSON.stringify(r.value);
    expect(json).not.toContain("alice");
    expect(r.report.counts.username).toBeGreaterThanOrEqual(1);
  });
});

describe("determinism", () => {
  test("same input → same output", () => {
    const input =
      "key sk-abcdefghijklmnop0123456789 home /Users/alice/x email a@b.com keep openagents/khala #42";
    const a = red(input);
    const b = red(input);
    expect(a.value).toBe(b.value);
    expect(a.report).toEqual(b.report);
  });
});

describe("redactValue — deep walk over a trajectory-shaped object", () => {
  test("scrubs string leaves at any depth, leaves numbers, returns a merged report", () => {
    const trajectory = {
      schema_version: "ATIF-v1.7",
      agent: { name: "x", version: "1", model_name: "openagents/khala" },
      steps: [
        { step_id: 1, source: "user", message: "run with key sk-abcdefghijklmnop0123456789" },
        {
          step_id: 2,
          source: "agent",
          message: "wrote to /Users/bob/work and emailed bob@host.io",
          metrics: { prompt_tokens: 10, completion_tokens: 2, cost_usd: 0 },
          tool_calls: [
            {
              tool_call_id: "call_2",
              function_name: "bash",
              arguments: { cmd: "cat .secrets/x.env", token: "oa_agent_ZZZ123456789abc" },
            },
          ],
        },
      ],
    };
    const r = redactValue(trajectory);
    const json = JSON.stringify(r.value);
    expect(json).not.toContain("sk-abcdefghijklmnop");
    expect(json).not.toContain("/Users/bob");
    expect(json).not.toContain("bob@host.io");
    expect(json).not.toContain(".secrets/x.env");
    expect(json).not.toContain("oa_agent_ZZZ");
    // model id preserved
    expect(json).toContain("openagents/khala");
    // numeric metric fields untouched
    expect(r.value.steps[1].metrics?.prompt_tokens).toBe(10);
    // report has multiple categories
    expect(r.report.total).toBeGreaterThanOrEqual(5);
    expect(r.report.counts.provider_key).toBe(1);
    expect(r.report.counts.home_path).toBe(1);
    expect(r.report.counts.email).toBe(1);
    expect(r.report.counts.secrets_path).toBe(1);
    expect(r.report.counts.oa_agent_token).toBe(1);
  });
});

describe("TraceRedactor Effect service", () => {
  test("redact via the Default layer matches the pure engine", () => {
    const value = { msg: "Bearer abcdef0123456789ABCDEF home /Users/carol/x" };
    const prog = Effect.gen(function* () {
      const svc = yield* TraceRedactor;
      return yield* svc.redact(value);
    });
    const out = Effect.runSync(prog.pipe(Effect.provide(TraceRedactor.Default)));
    expect(out).toEqual(redactValue(value));
    expect(JSON.stringify(out.value)).not.toContain("/Users/carol");
    expect(JSON.stringify(out.value)).toContain("[REDACTED:bearer]");
  });

  test("redactString via the service", () => {
    const prog = Effect.gen(function* () {
      const svc = yield* TraceRedactor;
      return yield* svc.redactString("email d@e.com");
    });
    const out = Effect.runSync(prog.pipe(Effect.provide(TraceRedactor.Default)));
    expect(out.value).toContain("[REDACTED:email]");
  });
});
