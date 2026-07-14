import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash } from "node:crypto";
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

const packageRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(packageRoot, "../..");
const binary = path.join(packageRoot, "zig-out/bin/native-sdk-effect-native-spike");
const automationDir = path.join(packageRoot, ".zig-cache/native-sdk-automation");
const evidenceDir = process.env.NATIVE_SDK_HOST_SMOKE_DIR?.trim() ||
  path.join(repositoryRoot, "var/native-sdk-effect-native-spike/host-smoke");
const snapshotPath = path.join(automationDir, "snapshot.txt");
const accessibilityPath = path.join(automationDir, "accessibility.txt");
const screenshotPath = path.join(automationDir, "screenshot-native-shell.png");

const sleep = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const sha256 = (bytes: Buffer | string): string =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const publisherPid = (snapshot: string): number | null => {
  const value = snapshot.match(/\bpublisher_pid=(\d+)\b/u)?.[1];
  return value === undefined ? null : Number(value);
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

const copySnapshot = (name: string, snapshot: string): void => {
  writeFileSync(path.join(evidenceDir, `${name}.snapshot.txt`), snapshot, { mode: 0o600 });
};

if (!existsSync(binary)) throw new Error(`Automation-enabled Native SDK binary is missing: ${binary}`);
assertNoLivePublisher();
rmSync(automationDir, { recursive: true, force: true });
rmSync(evidenceDir, { recursive: true, force: true });
mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

const logs: string[] = [];
let child: ChildProcessWithoutNullStreams | null = null;
try {
  child = launch(logs);
  let snapshot = await waitForSnapshot(child, "initial Effect projection", (value) =>
    value.includes("dispatch_errors=0") &&
    value.includes("gpu_nonblank=true") &&
    value.includes('url="zero://app/index.html"') &&
    value.includes('name="Effect state synchronized"')
  );
  let revision = projectionRevision(snapshot);
  copySnapshot("01-initial", snapshot);
  copyFileSync(accessibilityPath, path.join(evidenceDir, "01-initial.accessibility.txt"));

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Native parity pass")}`, child);
  snapshot = await waitForSnapshot(child, "canonical Effect fixture state", (value) =>
    /name="Native parity pass".*state=\[[^\]]*selected/u.test(value) &&
    value.includes(`name="2 messages · revision ${revision + 1}"`)
  );
  revision += 1;

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
  revision = projectionRevision(snapshot);
  copySnapshot("05-process-restart", snapshot);

  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "button", "New chat")}`, child);
  snapshot = await waitForSnapshot(child, "Effect-confirmed new chat after restart", (value) =>
    value.includes(`name="0 messages · revision ${revision + 1}"`) &&
    !/name="(?:Native parity pass|Renderer boundary|SDK adoption audit)".*state=\[[^\]]*selected/u.test(value)
  );
  copySnapshot("06-new-chat", snapshot);

  const evidence = readdirSync(evidenceDir).sort().map((name) => {
    const bytes = readFileSync(path.join(evidenceDir, name));
    return { name, digest: sha256(bytes), bytes: bytes.length };
  });
  writeFileSync(path.join(evidenceDir, "host-gate.json"), `${JSON.stringify({
    formatVersion: "openagents.native-sdk.host-gate.v1",
    targetRef: "openagents.desktop.native-sdk.spike",
    automationProtocol: 6,
    frontendAuthority: "effect-native",
    result: "passed",
    steps: [
      "initial-projection",
      "session-selection",
      "workspace-round-trip",
      "native-canvas-screenshot",
      "renderer-reload-restored",
      "process-restart-restored",
      "new-chat-after-restart",
    ],
    evidence,
  }, null, 2)}\n`, { mode: 0o600 });
  console.log(`[native-sdk-effect-native-spike smoke] OK evidence=${evidenceDir}`);
} finally {
  if (child !== null) await stop(child);
  writeFileSync(path.join(evidenceDir, "native-host.log"), logs.join("").slice(-4 * 1024 * 1024), { mode: 0o600 });
}
