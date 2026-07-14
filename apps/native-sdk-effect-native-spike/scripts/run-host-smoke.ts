import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import {
  decodeNativeSdkHostGate,
  nativeSdkAutomationProtocol,
  nativeSdkCommit,
  nativeSdkHostGateFormat,
  nativeSdkHostGateSteps,
  nativeSdkTargetRef,
} from "./host-gate.ts";

const packageRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(packageRoot, "../..");
const binary = path.join(packageRoot, "zig-out/bin/native-sdk-effect-native-spike");
const automationDir = path.join(packageRoot, ".zig-cache/native-sdk-automation");
const evidenceDir = process.env.NATIVE_SDK_HOST_SMOKE_DIR?.trim() ||
  path.join(repositoryRoot, "var/native-sdk-effect-native-spike/host-smoke");
const snapshotPath = path.join(automationDir, "snapshot.txt");
const accessibilityPath = path.join(automationDir, "accessibility.txt");
const screenshotPath = path.join(automationDir, "screenshot-native-shell.png");
const expectedNodeVersion = "24.13.1";
const expectedZigVersion = "0.16.0";
const sourcePaths = [
  "apps/native-sdk-effect-native-spike/app.zon",
  "apps/native-sdk-effect-native-spike/build.zig",
  "apps/native-sdk-effect-native-spike/build.zig.zon",
  "apps/native-sdk-effect-native-spike/package.json",
  "apps/native-sdk-effect-native-spike/src/main.zig",
  "apps/native-sdk-effect-native-spike/src/tests.zig",
  "apps/native-sdk-effect-native-spike/frontend/src/main.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/native-bridge.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/program.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/state-storage.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/style.css",
  "apps/native-sdk-effect-native-spike/scripts/host-gate.ts",
  "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
] as const;

const sleep = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const sha256 = (bytes: Buffer | string): string =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const filesUnder = (directory: string): string[] => readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
  const absolute = path.join(directory, entry.name);
  return entry.isDirectory() ? filesUnder(absolute) : [absolute];
});

const fileSetDigest = (paths: ReadonlyArray<string>, base: string): string => sha256(JSON.stringify(
  [...paths].sort().map((absolute) => ({
    path: path.relative(base, absolute).split(path.sep).join("/"),
    digest: sha256(readFileSync(absolute)),
  })),
));

const publisherPid = (snapshot: string): number | null => {
  const value = snapshot.match(/\bpublisher_pid=(\d+)\b/u)?.[1];
  return value === undefined ? null : Number(value);
};

const requiredPublisherPid = (snapshot: string, expected: number): number => {
  const observed = publisherPid(snapshot);
  if (observed !== expected) throw new Error(`Expected snapshot publisher ${expected}; observed ${String(observed)}`);
  return observed;
};

const processIsLive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const assertNoLivePublisher = (): void => {
  try {
    const pid = publisherPid(readFileSync(snapshotPath, "utf8"));
    if (pid !== null && processIsLive(pid)) {
      throw new Error(`Native SDK automation directory is owned by live publisher ${pid}`);
    }
  } catch (error) {
    if (error instanceof Error && error.message.includes("owned by live publisher")) throw error;
  }
};

let commandSequence = 0;
const queueCommand = async (command: string, child: ChildProcessWithoutNullStreams): Promise<void> => {
  const commandPath = path.join(automationDir, `command-${++commandSequence}.txt`);
  writeFileSync(commandPath, `${command}\n`, { flag: "wx", mode: 0o600 });
  const deadline = Date.now() + 30_000;
  while (existsSync(commandPath)) {
    if (child.exitCode !== null) throw new Error(`Native SDK app exited while consuming: ${command}`);
    if (Date.now() >= deadline) throw new Error(`Native SDK automation command timed out: ${command}`);
    await sleep(50);
  }
};

const readSnapshot = (): string => readFileSync(snapshotPath, "utf8");

const waitForSnapshot = async (
  child: ChildProcessWithoutNullStreams,
  label: string,
  predicate: (snapshot: string) => boolean,
  timeoutMilliseconds = 30_000,
): Promise<string> => {
  const deadline = Date.now() + timeoutMilliseconds;
  let last = "";
  do {
    if (child.exitCode !== null) throw new Error(`Native SDK app exited before ${label}`);
    try {
      last = readSnapshot();
      const pid = publisherPid(last);
      if (pid === child.pid && last.includes("ready=true") && predicate(last)) return last;
    } catch { /* snapshot publication is atomic but may not exist yet */ }
    await sleep(75);
  } while (Date.now() < deadline);
  throw new Error(`Timed out waiting for ${label}; last snapshot digest ${sha256(last)}`);
};

const widgetId = (
  snapshot: string,
  role: "button" | "listitem",
  accessibleName: string,
): string => {
  const matches = snapshot.split("\n").flatMap((line) => {
    if (!line.includes(`role=${role}`) || !line.includes(`name="${accessibleName}"`)) return [];
    const id = line.match(/\bwidget [^#]+#(\d+)\b/u)?.[1];
    return id === undefined ? [] : [id];
  });
  if (matches.length !== 1) {
    throw new Error(`Expected exactly one ${role} named ${accessibleName}; observed ${matches.length}`);
  }
  return matches[0]!;
};

const projectionRevision = (snapshot: string): number => {
  const value = snapshot.match(/name="\d+ messages · revision (\d+)"/u)?.[1];
  if (value === undefined) throw new Error("Native snapshot omitted the Effect projection revision");
  return Number(value);
};

const launch = (logs: string[]): ChildProcessWithoutNullStreams => {
  const child = spawn(binary, [], {
    cwd: packageRoot,
    env: process.env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk: Buffer) => logs.push(chunk.toString("utf8")));
  child.stderr.on("data", (chunk: Buffer) => logs.push(chunk.toString("utf8")));
  return child;
};

const stop = async (child: ChildProcessWithoutNullStreams): Promise<void> => {
  if (child.exitCode !== null) return;
  const exited = new Promise<void>((resolve) => child.once("exit", () => resolve()));
  child.kill("SIGTERM");
  const clean = await Promise.race([exited.then(() => true), sleep(5_000).then(() => false)]);
  if (!clean) {
    child.kill("SIGKILL");
    await exited;
  }
};

const compositedWindowCapture = async (child: ChildProcessWithoutNullStreams, outputPath: string): Promise<void> => {
  if (child.pid === undefined) throw new Error("Native SDK child PID is unavailable");
  const deadline = Date.now() + 10_000;
  let windowId = "";
  do {
    const query = spawnSync("swift", ["-e", [
      "import CoreGraphics",
      `let target = Int32(${child.pid})`,
      "let rows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []",
      "for row in rows where (row[kCGWindowOwnerPID as String] as? Int32) == target {",
      "  if let number = row[kCGWindowNumber as String] as? Int { print(number); break }",
      "}",
    ].join("\n")], { encoding: "utf8" });
    if (query.status === 0) windowId = query.stdout.trim();
    if (/^\d+$/u.test(windowId)) break;
    await sleep(100);
  } while (Date.now() < deadline);
  if (!/^\d+$/u.test(windowId)) throw new Error("Could not resolve the Native SDK macOS window id");
  const capture = spawnSync("screencapture", ["-x", `-l${windowId}`, outputPath], { encoding: "utf8" });
  if (capture.status !== 0 || !existsSync(outputPath) || statSync(outputPath).size < 10_000) {
    throw new Error(`Composited Native SDK window capture failed: ${capture.stderr.trim()}`);
  }
};

const copySnapshot = (name: string, snapshot: string): void => {
  writeFileSync(path.join(evidenceDir, `${name}.snapshot.txt`), snapshot, { mode: 0o600 });
};

if (!existsSync(binary)) throw new Error(`Automation-enabled Native SDK binary is missing: ${binary}`);
if (process.platform !== "darwin" || process.arch !== "arm64" || process.versions.node !== expectedNodeVersion) {
  throw new Error(`Native host gate runtime mismatch: expected darwin/arm64/Node ${expectedNodeVersion}, observed ${process.platform}/${process.arch}/Node ${process.versions.node}`);
}
const zig = spawnSync("zig", ["version"], { encoding: "utf8" });
if (zig.status !== 0 || zig.stdout.trim() !== expectedZigVersion) {
  throw new Error(`Native host gate Zig mismatch: expected ${expectedZigVersion}, observed ${zig.stdout.trim() || "unavailable"}`);
}
assertNoLivePublisher();
rmSync(automationDir, { recursive: true, force: true });
rmSync(evidenceDir, { recursive: true, force: true });
mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

const logs: string[] = [];
const runNonce = randomUUID();
let initialProcess: { pid: number; publisherPid: number; stopped: true } | null = null;
let restartedProcess: { pid: number; publisherPid: number; stopped: true } | null = null;
let child: ChildProcessWithoutNullStreams | null = null;
try {
  child = launch(logs);
  let snapshot = await waitForSnapshot(child, "initial Effect projection", (value) =>
    value.includes("dispatch_errors=0") &&
    value.includes("gpu_nonblank=true") &&
    value.includes('url="zero://app/index.html"') &&
    value.includes('name="Effect state synchronized"')
  );
  if (child.pid === undefined) throw new Error("Initial Native SDK process has no PID");
  initialProcess = { pid: child.pid, publisherPid: requiredPublisherPid(snapshot, child.pid), stopped: true };
  let revision = projectionRevision(snapshot);
  copySnapshot("01-initial", snapshot);
  copyFileSync(accessibilityPath, path.join(evidenceDir, "01-initial.accessibility.txt"));

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Native parity pass")}`, child);
  snapshot = await waitForSnapshot(child, "canonical Effect fixture state", (value) =>
    /name="Native parity pass".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="2 messages · revision ${revision + 1}"`)
  );
  revision += 1;
  await compositedWindowCapture(child, path.join(evidenceDir, "01-composited-window.png"));

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Renderer boundary")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed session selection", (value) =>
    /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="0 messages · revision ${revision + 1}"`) &&
    value.includes('name="Effect state synchronized"')
  );
  revision += 1;
  copySnapshot("02-session-selected", snapshot);

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Workspace")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed workspace selection", (value) =>
    /name="Workspace".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="0 messages · revision ${revision + 1}"`)
  );
  revision += 1;
  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Chat")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed chat return", (value) =>
    /name="Chat".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="0 messages · revision ${revision + 1}"`)
  );
  revision += 1;
  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Renderer boundary")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed session restore", (value) =>
    /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="0 messages · revision ${revision + 1}"`)
  );
  revision += 1;
  copySnapshot("03-workspace-round-trip", snapshot);

  await queueCommand("screenshot native-shell 1", child);
  if (!existsSync(screenshotPath) || statSync(screenshotPath).size < 1_000) {
    throw new Error("Native SDK retained-canvas screenshot was missing or empty");
  }
  copyFileSync(screenshotPath, path.join(evidenceDir, "03-native-shell.png"));

  await queueCommand("native-command openagents.spike.reload-effect", child);
  snapshot = await waitForSnapshot(child, "Effect state after WebView reload", (value) =>
    /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="0 messages · revision ${revision + 1}"`) &&
    value.includes('name="Effect state synchronized"')
  );
  revision += 1;
  copySnapshot("04-renderer-reload", snapshot);

  await stop(child);
  child = null;
  rmSync(automationDir, { recursive: true, force: true });
  commandSequence = 0;

  child = launch(logs);
  snapshot = await waitForSnapshot(child, "Effect state after native process restart", (value) =>
    value.includes("dispatch_errors=0") &&
    /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
    projectionRevision(value) > revision &&
    value.includes('name="Effect state synchronized"')
  );
  if (child.pid === undefined) throw new Error("Restarted Native SDK process has no PID");
  restartedProcess = { pid: child.pid, publisherPid: requiredPublisherPid(snapshot, child.pid), stopped: true };
  revision = projectionRevision(snapshot);
  copySnapshot("05-process-restart", snapshot);

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "button", "New chat")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed new chat after restart", (value) =>
    value.includes(`name="0 messages · revision ${revision + 1}"`) &&
    !/name="(?:Native parity pass|Renderer boundary|SDK adoption audit)".*state=\[[^\]]*selected/u.test(value)
  );
  copySnapshot("06-new-chat", snapshot);

  await stop(child);
  child = null;
  if (initialProcess === null || restartedProcess === null) throw new Error("Native host process attestations are incomplete");

  const evidence = readdirSync(evidenceDir).sort().map((name) => {
    const bytes = readFileSync(path.join(evidenceDir, name));
    return { name, digest: sha256(bytes), bytes: bytes.length };
  });
  const hostGate = decodeNativeSdkHostGate({
    formatVersion: nativeSdkHostGateFormat,
    targetRef: nativeSdkTargetRef,
    runNonce,
    automationProtocol: nativeSdkAutomationProtocol,
    frontendAuthority: "effect-native",
    result: "passed",
    runtime: {
      os: process.platform,
      architecture: process.arch,
      node: process.versions.node,
      zig: zig.stdout.trim(),
      nativeSdkCommit,
    },
    inputs: {
      commandDigest: sha256(JSON.stringify({
        build: ["zig", "build", "-Dautomation=true"],
        execute: ["node", "--import", "tsx", "scripts/run-host-smoke.ts"],
      })),
      binaryDigest: sha256(readFileSync(binary)),
      frontendDigest: fileSetDigest(filesUnder(path.join(packageRoot, "frontend/dist")), packageRoot),
      sourceDigest: fileSetDigest(sourcePaths.map((entry) => path.join(repositoryRoot, entry)), repositoryRoot),
    },
    processes: { initial: initialProcess, restarted: restartedProcess },
    steps: nativeSdkHostGateSteps.map((id) => ({ id, result: "passed", evidence: id === "composited-window-capture" ? ["01-composited-window.png"] : [] })),
    evidence,
  });
  writeFileSync(path.join(evidenceDir, "host-gate.json"), `${JSON.stringify(hostGate, null, 2)}\n`, { mode: 0o600 });
  console.log(`[native-sdk-effect-native-spike smoke] OK evidence=${evidenceDir}`);
} finally {
  if (child !== null) await stop(child);
  writeFileSync(path.join(evidenceDir, "native-host.log"), logs.join("").slice(-4 * 1024 * 1024), { mode: 0o600 });
}
