import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vite-plus/test";

const imageRoot = resolve(import.meta.dirname, "../deploy/agent-computer");
const manifest = JSON.parse(
  readFileSync(resolve(imageRoot, "agent-computer-image.manifest.json"), "utf8"),
) as {
  guestImage: {
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
    expect(manifest.guestImage.harnesses.status).toBe("source_wired_rebake_and_live_proof_pending");
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
});
