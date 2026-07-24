import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vite-plus/test";

const imageRoot = resolve(import.meta.dirname, "../deploy/agent-computer");
const manifest = JSON.parse(
  readFileSync(resolve(imageRoot, "agent-computer-image.manifest.json"), "utf8"),
) as {
  guestImage: {
    codex: Record<string, unknown>;
    harnesses: Record<string, Record<string, unknown> | string>;
  };
  isolation: {
    credentialScannerRequired: boolean;
    providerCredentialPolicy: string;
  };
  runtime: { agents: string[] };
};
const lock = JSON.parse(
  readFileSync(resolve(imageRoot, "harnesses/package-lock.json"), "utf8"),
) as {
  lockfileVersion: number;
  packages: Record<string, { version?: string }>;
};
const bake = readFileSync(resolve(imageRoot, "build-agent-computer-rootfs.sh"), "utf8");

describe("Agent Computer seven-harness image pins (#9193)", () => {
  test("declares all seven runtime harnesses and keeps runtime-only credential policy", () => {
    expect(manifest.runtime.agents).toEqual([
      "codex",
      "claude-code",
      "cursor",
      "goose",
      "opencode",
      "pi",
      "grok",
    ]);
    expect(manifest.isolation).toMatchObject({
      credentialScannerRequired: true,
      providerCredentialPolicy: "broker_only",
    });
    expect(manifest.guestImage.harnesses.status).toBe(
      "six_of_seven_runtime_qualified_owner_reauthentication_required_for_codex",
    );
    for (const harnessId of ["cursor", "goose", "opencode", "pi", "grok"]) {
      expect(manifest.guestImage.harnesses[harnessId]).toMatchObject({
        executionState: "runtime_secret_and_real_writeback_qualified",
        qualification: {
          changedFileCount: expect.any(Number),
          cleanupReceipt: expect.stringMatching(/^sha256:[a-f0-9]{64}$/u),
          commit: expect.stringMatching(/^[a-f0-9]{40}$/u),
          exitCode: 0,
          leaseTerminalOutcome: "managed_cloud_turn_completed",
          parentCommit: expect.stringMatching(/^[a-f0-9]{40}$/u),
          turnRef: expect.stringMatching(/^turn\./u),
        },
      });
    }
    expect(manifest.guestImage.codex).toMatchObject({
      executionState: "owner_reauthentication_required",
    });
    expect(manifest.guestImage.harnesses.claudeCode).toMatchObject({
      executionState: "runtime_secret_and_real_writeback_qualified",
      qualification: {
        exitCode: 0,
        leaseTerminalOutcome: "managed_cloud_turn_completed",
        usage: {
          truth: "exact",
        },
      },
    });
  });

  test("image-local npm lock fixes Claude Code, Pi, and OpenCode versions", () => {
    expect(lock.lockfileVersion).toBe(3);
    expect(lock.packages["node_modules/@anthropic-ai/claude-code"]?.version).toBe("2.1.218");
    expect(lock.packages["node_modules/@earendil-works/pi-coding-agent"]?.version).toBe("0.81.1");
    expect(lock.packages["node_modules/opencode-ai"]?.version).toBe("1.18.4");
  });

  test("native release digests match the manifest and the bake has no secret input", () => {
    for (const digest of [
      manifest.guestImage.harnesses.goose?.tarballSha256,
      manifest.guestImage.harnesses.cursor?.tarballSha256,
      manifest.guestImage.harnesses.grok?.binarySha256,
    ]) {
      expect(typeof digest).toBe("string");
      expect(bake).toContain(String(digest));
    }
    expect(bake).not.toContain("--api-key");
    expect(bake).not.toContain("--token");
    expect(bake).not.toContain("GEMINI_API_KEY=");
    expect(bake).not.toContain("XAI_API_KEY=");
    expect(bake).toContain('credentialMaterial": "runtime_only"');
  });

  test("turn-runner bundle is self-contained and executes a typed bake probe", () => {
    expect(bake).toContain("--deps.always-bundle '.*'");
    expect(bake).toContain("turn-runner-bake-probe.json");
    expect(bake).toContain('.schemaVersion == "openagents.agent_computer.turn_result.v1"');
    expect(bake).toContain('.failureReasonRef == "agent_computer.turn_failed"');
  });

  test("reserves enough guest space for a real repository checkout and verification", () => {
    expect(bake).toContain("SIZE_MIB=12288");
    expect(bake).toContain("MIN_RUNTIME_FREE_KIB=$((6 * 1024 * 1024))");
    expect(bake).toContain('"runtimeFreeKib": $RUNTIME_FREE_KIB');
  });
});
