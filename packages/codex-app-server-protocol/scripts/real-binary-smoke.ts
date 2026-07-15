import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { bundledCodexVersion, evaluateCodexBinaryCompatibility } from "../src/compatibility.ts";

const executable = process.env.CODEX_BIN;
if (!executable) throw new Error("CODEX_BIN must name the exact packaged Codex executable");
const versionOutput = await new Promise<string>((resolve, reject) => {
  const child = spawn(executable, ["--version"], { stdio: ["ignore", "pipe", "pipe"] });
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += String(chunk);
  });
  child.once("error", reject);
  child.once("close", (code) =>
    code === 0 ? resolve(output.trim()) : reject(new Error(`version exited ${code}`)),
  );
});
const version = /^(?:codex-cli|codex)\s+(\d+\.\d+\.\d+)$/.exec(versionOutput)?.[1] ?? "malformed";
const sha256 = createHash("sha256").update(readFileSync(executable)).digest("hex");
const target =
  process.platform === "darwin" && process.arch === "arm64"
    ? "aarch64-apple-darwin"
    : "unsupported";
const compatibility = evaluateCodexBinaryCompatibility({ version, target, sha256 });
if (compatibility._tag !== "Compatible") {
  throw new Error(`binary compatibility failed: ${compatibility.reason}`);
}
if (version !== bundledCodexVersion) throw new Error("version invariant failed");

const home = mkdtempSync(join(tmpdir(), "openagents-codex-smoke-"));
try {
  const child = spawn(executable, ["app-server"], {
    env: { ...process.env, CODEX_HOME: home },
    stdio: ["pipe", "pipe", "pipe"],
  });
  const response = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("initialize timed out")), 10_000);
    const lines = createInterface({ input: child.stdout });
    lines.on("line", (line) => {
      const message = JSON.parse(line) as { id?: unknown; result?: unknown; error?: unknown };
      if (message.id !== 1) return;
      clearTimeout(timer);
      if (message.error !== undefined)
        reject(new Error(`initialize failed: ${JSON.stringify(message.error)}`));
      else resolve();
    });
    child.once("error", reject);
  });
  child.stdin.write(
    `${JSON.stringify({
      id: 1,
      method: "initialize",
      params: {
        clientInfo: {
          name: "openagents_protocol_smoke",
          title: "OpenAgents Protocol Smoke",
          version: "0.1.0",
        },
        capabilities: { experimentalApi: false },
      },
    })}\n`,
  );
  await response;
  child.stdin.write(`${JSON.stringify({ method: "initialized", params: {} })}\n`);
  child.kill("SIGTERM");
  console.log(`Verified Codex ${version} initialize/initialized with experimentalApi=false.`);
} finally {
  rmSync(home, { recursive: true, force: true });
}
