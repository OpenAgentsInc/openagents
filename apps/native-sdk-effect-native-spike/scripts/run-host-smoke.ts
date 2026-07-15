import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

import {
  decodeNativeSdkHostGate,
  nativeSdkAutomationProtocol,
  nativeSdkCommit,
  nativeSdkHostGateFormat,
  nativeSdkTargetRef,
} from "./host-gate.ts";

const packageRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(packageRoot, "../..");
const binary = path.join(packageRoot, "zig-out/bin/native-sdk-effect-native-spike");
const sidecarBundle = path.join(packageRoot, "sidecar/dist/native-sidecar-entry.mjs");
const automationDir = path.join(packageRoot, ".zig-cache/native-sdk-automation");
const evidenceDir =
  process.env.NATIVE_SDK_HOST_SMOKE_DIR?.trim() ||
  path.join(repositoryRoot, "var/native-sdk-effect-native-spike/host-smoke");
const snapshotPath = path.join(automationDir, "snapshot.txt");
const accessibilityPath = path.join(automationDir, "accessibility.txt");
const screenshotPath = path.join(automationDir, "screenshot-native-shell.png");
const expectedNodeVersion = "24.13.1";
const expectedZigVersion = "0.16.0";
const assuranceEnvironmentKeys = {
  manifestDigest: "OPENAGENTS_ASSURANCE_MANIFEST_DIGEST",
  environmentDigest: "OPENAGENTS_ASSURANCE_ENVIRONMENT_DIGEST",
  adapterLockDigest: "OPENAGENTS_ASSURANCE_ADAPTER_LOCK_DIGEST",
  targetDescriptorDigest: "OPENAGENTS_ASSURANCE_TARGET_DESCRIPTOR_DIGEST",
  targetSourceDigest: "OPENAGENTS_ASSURANCE_TARGET_SOURCE_DIGEST",
} as const;
const sourcePaths = [
  "apps/native-sdk-effect-native-spike/app.zon",
  "apps/native-sdk-effect-native-spike/build.zig",
  "apps/native-sdk-effect-native-spike/build.zig.zon",
  "apps/native-sdk-effect-native-spike/package.json",
  "apps/native-sdk-effect-native-spike/vite.config.ts",
  "apps/native-sdk-effect-native-spike/frontend/index.html",
  "apps/native-sdk-effect-native-spike/src/main.zig",
  "apps/native-sdk-effect-native-spike/src/tests.zig",
  "apps/native-sdk-effect-native-spike/frontend/src/main.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/native-bridge.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/native-sdk-component-adoption.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/production-command-parity.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/program.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/state-storage.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/style.css",
  "apps/native-sdk-effect-native-spike/scripts/host-gate.ts",
  "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
  "apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts",
  "apps/native-sdk-effect-native-spike/assurance/native-ac03-observation.ts",
  "apps/openagents-desktop/src/native-sidecar-contract.ts",
  "apps/openagents-desktop/src/native-sidecar-contract.test.ts",
  "apps/openagents-desktop/src/native-sidecar-entry.ts",
  "apps/openagents-desktop/src/desktop-command-contract.ts",
  "apps/openagents-desktop/package.json",
  "apps/openagents-desktop/src/chat-contract.ts",
  "apps/openagents-desktop/src/desktop-coding-catalog.ts",
  "apps/openagents-desktop/src/coding-catalog-contract.ts",
  "apps/openagents-desktop/src/desktop-workspace-admission.ts",
  "apps/openagents-desktop/src/desktop-sync-host.ts",
  "apps/openagents-desktop/src/desktop-sync-store.ts",
  "apps/openagents-desktop/src/workspace-contract.ts",
  "apps/openagents-desktop/src/workspace-service.ts",
  "apps/openagents-desktop/src/runtime-gateway-contract.ts",
  "apps/openagents-desktop/src/runtime-gateway.ts",
  "apps/openagents-desktop/src/renderer/app.css",
  "apps/openagents-desktop/src/renderer/command-registry.ts",
  "apps/openagents-desktop/src/renderer/portable.ts",
  "apps/openagents-desktop/src/renderer/shell.ts",
  "packages/assurance-spec/src/native-sdk-assurance-adapter.ts",
  "packages/assurance-spec/scripts/mvp-assurance-target.ts",
  "pnpm-lock.yaml",
] as const;

const sleep = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const sha256 = (bytes: Buffer | string): string =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const assuranceBinding = (): Readonly<
  Record<keyof typeof assuranceEnvironmentKeys, string>
> | null => {
  const entries = Object.entries(assuranceEnvironmentKeys).map(
    ([field, key]) => [field, process.env[key]?.trim()] as const,
  );
  if (entries.every(([, value]) => value === undefined || value === "")) return null;
  if (entries.some(([, value]) => value === undefined || value === "")) {
    throw new Error("Native host gate assurance binding must be supplied completely or omitted.");
  }
  return Object.fromEntries(entries) as Record<keyof typeof assuranceEnvironmentKeys, string>;
};

const filesUnder = (directory: string): string[] =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(absolute) : [absolute];
  });

const fileSetDigest = (paths: ReadonlyArray<string>, base: string): string =>
  sha256(
    JSON.stringify(
      [...paths].sort().map((absolute) => ({
        path: path.relative(base, absolute).split(path.sep).join("/"),
        digest: sha256(readFileSync(absolute)),
      })),
    ),
  );

const publisherPid = (snapshot: string): number | null => {
  const value = snapshot.match(/\bpublisher_pid=(\d+)\b/u)?.[1];
  return value === undefined ? null : Number(value);
};

const requiredPublisherPid = (snapshot: string, expected: number): number => {
  const observed = publisherPid(snapshot);
  if (observed !== expected)
    throw new Error(`Expected snapshot publisher ${expected}; observed ${String(observed)}`);
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
const queueCommand = async (
  command: string,
  child: ChildProcessWithoutNullStreams,
): Promise<void> => {
  const commandPath = path.join(automationDir, `command-${++commandSequence}.txt`);
  writeFileSync(commandPath, `${command}\n`, { flag: "wx", mode: 0o600 });
  const deadline = Date.now() + 30_000;
  while (existsSync(commandPath)) {
    if (child.exitCode !== null)
      throw new Error(`Native SDK app exited while consuming: ${command}`);
    if (Date.now() >= deadline)
      throw new Error(`Native SDK automation command timed out: ${command}`);
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
    } catch {
      /* snapshot publication is atomic but may not exist yet */
    }
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
    throw new Error(
      `Expected exactly one ${role} named ${accessibleName}; observed ${matches.length}`,
    );
  }
  return matches[0]!;
};

const projectionRevision = (snapshot: string): number => {
  const value = snapshot.match(/name="\d+ messages · revision (\d+)"/u)?.[1];
  if (value === undefined)
    throw new Error("Native snapshot omitted the Effect projection revision");
  return Number(value);
};

const sidecarIsReady = (snapshot: string, generation: number): boolean =>
  snapshot.includes(
    `name="Desktop runtime gateway v11 · Node 24.13.1 · generation ${generation} · private sidecar ready"`,
  );

const directSidecarPids = (parentPid: number): number[] => {
  const rows = spawnSync("ps", ["-axo", "pid=,ppid=,command="], { encoding: "utf8" });
  if (rows.status !== 0) throw new Error("Could not inspect the Native host process tree");
  return rows.stdout.split("\n").flatMap((line) => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/u);
    if (match?.[1] === undefined || match[2] === undefined || match[3] === undefined) return [];
    return Number(match[2]) === parentPid && match[3].includes("native-sidecar-entry.mjs")
      ? [Number(match[1])]
      : [];
  });
};

const waitForSidecarPid = async (child: ChildProcessWithoutNullStreams): Promise<number> => {
  if (child.pid === undefined) throw new Error("Native SDK process has no PID");
  const deadline = Date.now() + 10_000;
  do {
    const pids = directSidecarPids(child.pid).filter(processIsLive);
    if (pids.length === 1) return pids[0]!;
    if (pids.length > 1) throw new Error("Native host has multiple live Desktop sidecars");
    await sleep(50);
  } while (Date.now() < deadline);
  throw new Error("Native host did not retain one live Desktop sidecar");
};

const waitForProcessExit = async (pid: number): Promise<void> => {
  const deadline = Date.now() + 10_000;
  while (processIsLive(pid) && Date.now() < deadline) await sleep(50);
  if (processIsLive(pid))
    throw new Error(`Sidecar ${pid} remained live after Native host teardown`);
};

type WorkAdmission = Readonly<{
  grantRef: string;
  projectRef: string;
  repositoryRef: string;
  worktreeRef: string;
  workContextRef: string;
  sessionRef: string;
}>;

const workIdentity = (
  snapshot: string,
): Readonly<{
  catalogSessionCount: number;
  requestSequence: number;
  admission: WorkAdmission;
}> => {
  const match = snapshot.match(
    /name="Catalog (\d+) · request (\d+) · Grant ([A-Za-z0-9._:-]+) · Project ([A-Za-z0-9._:-]+) · Repository ([A-Za-z0-9._:-]+) · Worktree ([A-Za-z0-9._:-]+) · WorkContext ([A-Za-z0-9._:-]+) · Session ([A-Za-z0-9._:-]+)"/u,
  );
  if (
    match?.[1] === undefined ||
    match[2] === undefined ||
    match[3] === undefined ||
    match[4] === undefined ||
    match[5] === undefined ||
    match[6] === undefined ||
    match[7] === undefined ||
    match[8] === undefined
  ) {
    throw new Error("Native snapshot omitted the durable coding admission");
  }
  return {
    catalogSessionCount: Number(match[1]),
    requestSequence: Number(match[2]),
    admission: {
      grantRef: match[3],
      projectRef: match[4],
      repositoryRef: match[5],
      worktreeRef: match[6],
      workContextRef: match[7],
      sessionRef: match[8],
    },
  };
};

const launch = (
  logs: string[],
  runNonce: string,
  sidecarGeneration: number,
  stateRoot: string,
): ChildProcessWithoutNullStreams => {
  const ambientGeneration = sidecarGeneration === 1 ? "alpha" : "beta";
  const child = spawn(binary, [], {
    cwd: packageRoot,
    env: {
      ...process.env,
      NATIVE_SDK_ASSURANCE_RUN_NONCE: runNonce,
      NATIVE_SDK_SIDECAR_GENERATION: String(sidecarGeneration),
      OPENAGENTS_NATIVE_NODE_PATH: process.execPath,
      OPENAGENTS_NATIVE_SIDECAR_PATH: sidecarBundle,
      OPENAGENTS_NATIVE_STATE_ROOT: stateRoot,
      OPENAGENTS_NATIVE_AMBIENT_HOSTNAME: `host-native-${ambientGeneration}`,
      OPENAGENTS_NATIVE_AMBIENT_PORT: `port-${sidecarGeneration === 1 ? "43101" : "53202"}`,
      OPENAGENTS_NATIVE_AMBIENT_PROVIDER_THREAD: `provider-thread-${ambientGeneration}`,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk: Buffer) => logs.push(chunk.toString("utf8")));
  child.stderr.on("data", (chunk: Buffer) => logs.push(chunk.toString("utf8")));
  return child;
};

type ProcessTermination = Readonly<{
  exitCode: number | null;
  signal: string | null;
  forcedKill: boolean;
}>;

const stop = async (child: ChildProcessWithoutNullStreams): Promise<ProcessTermination> => {
  if (child.exitCode !== null || child.signalCode !== null) {
    return { exitCode: child.exitCode, signal: child.signalCode, forcedKill: false };
  }
  const exited = new Promise<void>((resolve) => child.once("exit", () => resolve()));
  child.kill("SIGTERM");
  const clean = await Promise.race([exited.then(() => true), sleep(5_000).then(() => false)]);
  if (!clean) {
    child.kill("SIGKILL");
    await exited;
  }
  return { exitCode: child.exitCode, signal: child.signalCode, forcedKill: !clean };
};

const compositedWindowCapture = async (
  child: ChildProcessWithoutNullStreams,
  outputPath: string,
): Promise<void> => {
  if (child.pid === undefined) throw new Error("Native SDK child PID is unavailable");
  const deadline = Date.now() + 10_000;
  let windowId = "";
  do {
    const query = spawnSync(
      "swift",
      [
        "-e",
        [
          "import CoreGraphics",
          `let target = Int32(${child.pid})`,
          "let rows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []",
          "for row in rows where (row[kCGWindowOwnerPID as String] as? Int32) == target {",
          '  guard (row[kCGWindowName as String] as? String) == "OpenAgents Native parity spike" else { continue }',
          "  if let number = row[kCGWindowNumber as String] as? Int { print(number); break }",
          "}",
        ].join("\n"),
      ],
      { encoding: "utf8" },
    );
    if (query.status === 0) windowId = query.stdout.trim();
    if (/^\d+$/u.test(windowId)) break;
    await sleep(100);
  } while (Date.now() < deadline);
  if (!/^\d+$/u.test(windowId)) throw new Error("Could not resolve the Native SDK macOS window id");
  const capture = spawnSync("screencapture", ["-x", `-l${windowId}`, outputPath], {
    encoding: "utf8",
  });
  if (capture.status !== 0 || !existsSync(outputPath) || statSync(outputPath).size < 10_000) {
    const detail = capture.error?.message ?? capture.stderr?.trim() ?? "no diagnostic output";
    throw new Error(`Composited Native SDK window capture failed: ${detail}`);
  }
};

const copySnapshot = (name: string, snapshot: string): void => {
  writeFileSync(path.join(evidenceDir, `${name}.snapshot.txt`), snapshot, { mode: 0o600 });
};

if (!existsSync(binary))
  throw new Error(`Automation-enabled Native SDK binary is missing: ${binary}`);
if (!existsSync(sidecarBundle))
  throw new Error(`Bundled Desktop runtime sidecar is missing: ${sidecarBundle}`);
if (
  process.platform !== "darwin" ||
  process.arch !== "arm64" ||
  process.versions.node !== expectedNodeVersion
) {
  throw new Error(
    `Native host gate runtime mismatch: expected darwin/arm64/Node ${expectedNodeVersion}, observed ${process.platform}/${process.arch}/Node ${process.versions.node}`,
  );
}
const zig = spawnSync("zig", ["version"], { encoding: "utf8" });
if (zig.status !== 0 || zig.stdout.trim() !== expectedZigVersion) {
  throw new Error(
    `Native host gate Zig mismatch: expected ${expectedZigVersion}, observed ${zig.stdout.trim() || "unavailable"}`,
  );
}
assertNoLivePublisher();
rmSync(automationDir, { recursive: true, force: true });
rmSync(evidenceDir, { recursive: true, force: true });
mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

const logs: string[] = [];
const runNonce = randomUUID();
const assurance = assuranceBinding();
const privateRoot = path.join(packageRoot, ".zig-cache/native-sdk-private", runNonce);
const stateRoot = path.join(privateRoot, "state");
const grantedRepository = path.join(privateRoot, "assurance-repository");
const repositoryAlias = path.join(privateRoot, "assurance-repository-alias");
mkdirSync(grantedRepository, { recursive: true, mode: 0o700 });
const initializedRepository = spawnSync("git", ["init", "--quiet", grantedRepository], {
  encoding: "utf8",
});
if (initializedRepository.status !== 0)
  throw new Error("Could not initialize the private Git grant fixture");
symlinkSync(grantedRepository, repositoryAlias, "dir");
type ProcessIdentity = Readonly<{ pid: number; publisherPid: number }>;
type ProcessAttestation = ProcessIdentity & ProcessTermination & Readonly<{ stopped: true }>;
let initialIdentity: ProcessIdentity | null = null;
let restartedIdentity: ProcessIdentity | null = null;
let initialProcess: ProcessAttestation | null = null;
let restartedProcess: ProcessAttestation | null = null;
type SidecarAttestation = Readonly<{
  pid: number;
  generation: 1 | 2;
  liveDuringHost: true;
  liveAfterHost: false;
}>;
let initialSidecar: SidecarAttestation | null = null;
let restartedSidecar: SidecarAttestation | null = null;
let initialWorkIdentity: ReturnType<typeof workIdentity> | null = null;
let restartedWorkIdentity: ReturnType<typeof workIdentity> | null = null;
let child: ChildProcessWithoutNullStreams | null = null;
try {
  child = launch(logs, runNonce, 1, stateRoot);
  let snapshot = await waitForSnapshot(
    child,
    "initial Effect projection",
    (value) =>
      value.includes("dispatch_errors=0") &&
      value.includes("gpu_nonblank=true") &&
      value.includes(
        `url="zero://app/index.html#surface=effect-native&assurance-run=${runNonce}"`,
      ) &&
      value.includes('name="Production Desktop shell synchronized"') &&
      sidecarIsReady(value, 1),
  );
  if (child.pid === undefined) throw new Error("Initial Native SDK process has no PID");
  initialIdentity = { pid: child.pid, publisherPid: requiredPublisherPid(snapshot, child.pid) };
  const initialSidecarPid = await waitForSidecarPid(child);
  if (!processIsLive(initialSidecarPid))
    throw new Error("Persistent Native sidecar was not live after bootstrap");
  let revision = projectionRevision(snapshot);
  copySnapshot("01-initial", snapshot);
  copyFileSync(accessibilityPath, path.join(evidenceDir, "01-initial.accessibility.txt"));

  await queueCommand(
    `widget-action native-shell ${widgetId(snapshot, "button", "Grant repository")} drop-files ${grantedRepository}`,
    child,
  );
  snapshot = await waitForSnapshot(child, "production repository admission", (value) =>
    value.includes('name="Catalog 1 · request 2 · Grant '),
  );
  initialWorkIdentity = workIdentity(snapshot);
  if (initialWorkIdentity.catalogSessionCount !== 1 || initialWorkIdentity.requestSequence !== 2) {
    throw new Error(
      "Initial Native repository admission did not create exactly one catalog session",
    );
  }
  writeFileSync(
    path.join(evidenceDir, "02-cw-ac-03-initial.json"),
    `${JSON.stringify({
      schema: "openagents.native-sdk.cw-ac-03.v1",
      generation: 1,
      catalogSessionCount: initialWorkIdentity.catalogSessionCount,
      admission: initialWorkIdentity.admission,
    })}\n`,
    { mode: 0o600 },
  );

  await queueCommand(
    `widget-click native-shell ${widgetId(snapshot, "listitem", "Native parity pass")}`,
    child,
  );
  snapshot = await waitForSnapshot(
    child,
    "canonical Effect fixture state",
    (value) =>
      /name="Native parity pass".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="2 messages · revision ${revision + 1}"`),
  );
  revision += 1;
  await compositedWindowCapture(child, path.join(evidenceDir, "01-composited-window.png"));

  await queueCommand(
    `widget-click native-shell ${widgetId(snapshot, "listitem", "Renderer boundary")}`,
    child,
  );
  snapshot = await waitForSnapshot(
    child,
    "Effect-confirmed session selection",
    (value) =>
      /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="0 messages · revision ${revision + 1}"`) &&
      value.includes('name="Production Desktop shell synchronized"'),
  );
  revision += 1;
  copySnapshot("02-session-selected", snapshot);

  await queueCommand(
    `widget-click native-shell ${widgetId(snapshot, "listitem", "Workspace")}`,
    child,
  );
  snapshot = await waitForSnapshot(
    child,
    "Effect-confirmed workspace selection",
    (value) =>
      /name="Workspace".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="0 messages · revision ${revision + 1}"`),
  );
  revision += 1;
  await queueCommand(`widget-click native-shell ${widgetId(snapshot, "listitem", "Chat")}`, child);
  snapshot = await waitForSnapshot(
    child,
    "Effect-confirmed chat return",
    (value) =>
      /name="Chat".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="0 messages · revision ${revision + 1}"`),
  );
  revision += 1;
  await queueCommand(
    `widget-click native-shell ${widgetId(snapshot, "listitem", "Renderer boundary")}`,
    child,
  );
  snapshot = await waitForSnapshot(
    child,
    "Effect-confirmed session restore",
    (value) =>
      /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="0 messages · revision ${revision + 1}"`),
  );
  revision += 1;
  copySnapshot("03-workspace-round-trip", snapshot);

  await queueCommand("screenshot native-shell 1", child);
  if (!existsSync(screenshotPath) || statSync(screenshotPath).size < 1_000) {
    throw new Error("Native SDK retained-canvas screenshot was missing or empty");
  }
  copyFileSync(screenshotPath, path.join(evidenceDir, "03-native-shell.png"));

  await queueCommand("native-command openagents.spike.reload-effect", child);
  snapshot = await waitForSnapshot(
    child,
    "Effect state after WebView reload",
    (value) =>
      /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
      value.includes(`name="0 messages · revision ${revision + 1}"`) &&
      value.includes('name="Production Desktop shell synchronized"'),
  );
  revision += 1;
  copySnapshot("04-renderer-reload", snapshot);

  initialProcess = { ...initialIdentity, ...(await stop(child)), stopped: true };
  await waitForProcessExit(initialSidecarPid);
  initialSidecar = {
    pid: initialSidecarPid,
    generation: 1,
    liveDuringHost: true,
    liveAfterHost: false,
  };
  child = null;
  rmSync(automationDir, { recursive: true, force: true });
  commandSequence = 0;

  child = launch(logs, runNonce, 2, stateRoot);
  snapshot = await waitForSnapshot(
    child,
    "Effect state after native process restart",
    (value) =>
      value.includes("dispatch_errors=0") &&
      /name="Renderer boundary".*state=\[[^\]]*selected/u.test(value) &&
      projectionRevision(value) > revision &&
      value.includes('name="Production Desktop shell synchronized"') &&
      sidecarIsReady(value, 2) &&
      value.includes('name="Catalog 1 · request 1 · Grant '),
  );
  if (child.pid === undefined) throw new Error("Restarted Native SDK process has no PID");
  restartedIdentity = { pid: child.pid, publisherPid: requiredPublisherPid(snapshot, child.pid) };
  const restartedSidecarPid = await waitForSidecarPid(child);
  if (!processIsLive(restartedSidecarPid))
    throw new Error("Restarted persistent Native sidecar was not live");
  if (initialSidecar.pid === restartedSidecarPid)
    throw new Error("Native sidecar process identity did not advance across restart");
  restartedWorkIdentity = workIdentity(snapshot);
  if (
    JSON.stringify(restartedWorkIdentity.admission) !==
    JSON.stringify(initialWorkIdentity.admission)
  ) {
    throw new Error("Durable repository identity drifted across the Native process restart");
  }
  await queueCommand(
    `widget-action native-shell ${widgetId(snapshot, "button", "Grant repository")} drop-files ${repositoryAlias}`,
    child,
  );
  snapshot = await waitForSnapshot(child, "canonical alias repository admission", (value) =>
    value.includes('name="Catalog 1 · request 2 · Grant '),
  );
  const aliasedWorkIdentity = workIdentity(snapshot);
  if (
    aliasedWorkIdentity.catalogSessionCount !== 1 ||
    JSON.stringify(aliasedWorkIdentity.admission) !== JSON.stringify(initialWorkIdentity.admission)
  ) {
    throw new Error("Symlink alias created ambient or duplicate repository identity");
  }
  restartedWorkIdentity = aliasedWorkIdentity;
  writeFileSync(
    path.join(evidenceDir, "06-cw-ac-03-restarted.json"),
    `${JSON.stringify({
      schema: "openagents.native-sdk.cw-ac-03.v1",
      generation: 2,
      catalogSessionCount: restartedWorkIdentity.catalogSessionCount,
      admission: restartedWorkIdentity.admission,
      aliasCanonicalized: true,
    })}\n`,
    { mode: 0o600 },
  );
  revision = projectionRevision(snapshot);
  copySnapshot("05-process-restart", snapshot);

  await queueCommand("menu-command chat.new", child);
  snapshot = await waitForSnapshot(
    child,
    "production-resolved native New Chat command after restart",
    (value) =>
      value.includes(`name="0 messages · revision ${revision + 1}"`) &&
      /name="Applied chat\.new → DesktopNewChat · native_menu · sequence [1-9]\d*"/u.test(value) &&
      !/name="(?:Native parity pass|Renderer boundary|SDK adoption audit)".*state=\[[^\]]*selected/u.test(
        value,
      ),
  );
  copySnapshot("06-new-chat", snapshot);

  restartedProcess = { ...restartedIdentity, ...(await stop(child)), stopped: true };
  await waitForProcessExit(restartedSidecarPid);
  restartedSidecar = {
    pid: restartedSidecarPid,
    generation: 2,
    liveDuringHost: true,
    liveAfterHost: false,
  };
  child = null;
  if (
    initialProcess === null ||
    restartedProcess === null ||
    initialSidecar === null ||
    restartedSidecar === null
  ) {
    throw new Error("Native host or sidecar process attestations are incomplete");
  }
  if (initialProcess.forcedKill || restartedProcess.forcedKill)
    throw new Error("Native host process required forced termination");
  if (initialWorkIdentity === null || restartedWorkIdentity === null) {
    throw new Error("Native work-identity observations are incomplete");
  }
  const bindingPath = path.join(stateRoot, "coding-bindings.json");
  const bindingMode = (statSync(bindingPath).mode & 0o777).toString(8).padStart(4, "0");
  if (
    bindingMode !== "0600" ||
    !readFileSync(bindingPath, "utf8").includes(realpathSync(grantedRepository))
  ) {
    throw new Error(
      "The durable repository binding was not confined to its owner-private host file",
    );
  }
  writeFileSync(
    path.join(evidenceDir, "07-sidecar-restart.json"),
    `${JSON.stringify({
      initial: initialSidecar,
      restarted: restartedSidecar,
    })}\n`,
    { mode: 0o600 },
  );
  writeFileSync(
    path.join(evidenceDir, "08-clean-teardown.json"),
    `${JSON.stringify({
      initial: initialProcess,
      restarted: restartedProcess,
      publishersLive: [
        processIsLive(initialProcess.publisherPid),
        processIsLive(restartedProcess.publisherPid),
      ],
    })}\n`,
    { mode: 0o600 },
  );

  const evidence = readdirSync(evidenceDir)
    .sort()
    .map((name) => {
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
      commandDigest: sha256(
        JSON.stringify({
          build: [
            [
              "vp",
              "pack",
              "../openagents-desktop/src/native-sidecar-entry.ts",
              "--platform",
              "node",
              "--target",
              "node24",
            ],
            ["zig", "build", "-Dautomation=true"],
          ],
          execute: ["node", "--import", "tsx", "scripts/run-host-smoke.ts"],
        }),
      ),
      binaryDigest: sha256(readFileSync(binary)),
      sidecarBundleDigest: sha256(readFileSync(sidecarBundle)),
      frontendDigest: fileSetDigest(
        filesUnder(path.join(packageRoot, "frontend/dist")),
        packageRoot,
      ),
      sourceDigest: fileSetDigest(
        sourcePaths.map((entry) => path.join(repositoryRoot, entry)),
        repositoryRoot,
      ),
    },
    assurance,
    processes: { initial: initialProcess, restarted: restartedProcess },
    sidecars: { initial: initialSidecar, restarted: restartedSidecar },
    criterionObservations: {
      schema: "openagents.native-sdk.cw-ac-03.v1",
      criterionRef: "CW-AC-03",
      grantSource: "native_canvas_file_drop",
      initial: {
        generation: 1,
        catalogSessionCount: initialWorkIdentity.catalogSessionCount,
        admission: initialWorkIdentity.admission,
      },
      restarted: {
        generation: 2,
        catalogSessionCount: restartedWorkIdentity.catalogSessionCount,
        admission: restartedWorkIdentity.admission,
      },
      aliasCanonicalized: true,
      ambientInputsExcluded: true,
      ambientFalsifiers: {
        initial: {
          hostname: "host-native-alpha",
          port: "port-43101",
          providerThread: "provider-thread-alpha",
        },
        restarted: {
          hostname: "host-native-beta",
          port: "port-53202",
          providerThread: "provider-thread-beta",
        },
      },
      privateBindingMode: bindingMode,
    },
    steps: [
      {
        id: "initial-projection",
        result: "passed",
        evidence: ["01-initial.snapshot.txt", "01-initial.accessibility.txt"],
      },
      {
        id: "runtime-sidecar-bootstrap",
        result: "passed",
        evidence: [
          "01-initial.snapshot.txt",
          "05-process-restart.snapshot.txt",
          "07-sidecar-restart.json",
        ],
      },
      { id: "repository-grant-admitted", result: "passed", evidence: ["02-cw-ac-03-initial.json"] },
      { id: "composited-window-capture", result: "passed", evidence: ["01-composited-window.png"] },
      { id: "session-selection", result: "passed", evidence: ["02-session-selected.snapshot.txt"] },
      {
        id: "workspace-round-trip",
        result: "passed",
        evidence: ["03-workspace-round-trip.snapshot.txt"],
      },
      { id: "native-canvas-screenshot", result: "passed", evidence: ["03-native-shell.png"] },
      {
        id: "renderer-reload-restored",
        result: "passed",
        evidence: ["04-renderer-reload.snapshot.txt"],
      },
      {
        id: "process-restart-restored",
        result: "passed",
        evidence: ["05-process-restart.snapshot.txt"],
      },
      {
        id: "repository-identity-restored",
        result: "passed",
        evidence: ["06-cw-ac-03-restarted.json"],
      },
      { id: "new-chat-after-restart", result: "passed", evidence: ["06-new-chat.snapshot.txt"] },
      { id: "clean-teardown", result: "passed", evidence: ["08-clean-teardown.json"] },
    ],
    evidence,
  });
  writeFileSync(
    path.join(evidenceDir, "host-gate.json"),
    `${JSON.stringify(hostGate, null, 2)}\n`,
    { mode: 0o600 },
  );
  console.log(`[native-sdk-effect-native-spike smoke] OK evidence=${evidenceDir}`);
} finally {
  if (child !== null) await stop(child);
  writeFileSync(path.join(evidenceDir, "native-host.log"), logs.join("").slice(-4 * 1024 * 1024), {
    mode: 0o600,
  });
  rmSync(privateRoot, { recursive: true, force: true });
}
