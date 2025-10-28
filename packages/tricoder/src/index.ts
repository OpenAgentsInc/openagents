#!/usr/bin/env node
import chalk from "chalk"
import os from "node:os"
import { spawn, spawnSync } from "node:child_process"
import { existsSync, mkdirSync, copyFileSync, chmodSync, readdirSync, statSync, createWriteStream } from "node:fs"
import http from "node:http"
import https from "node:https"
import net from "node:net"
import { dirname, join } from "node:path"
import WebSocket from "ws"
import { buildTunnelArgs } from "./args.js"
import AdmZip from "adm-zip"
import qrcode from "qrcode-terminal"
import * as QR from "qrcode"
import fs from "node:fs"

// spawnSync already imported above
const VERBOSE = process.argv.includes("--verbose") || process.argv.includes("-v") || process.env.TRICODER_VERBOSE === "1";
const ASSUME_YES = process.argv.includes("--yes") || process.argv.includes("-y") || process.env.TRICODER_YES === "1";
const LOCAL_ONLY = process.argv.includes("--local-only") || process.env.TRICODER_LOCAL_ONLY === "1";
const NO_QR = process.argv.includes("--no-qr") || process.env.TRICODER_NO_QR === "1";
const QR_MODE = (() => {
  // --qr=deeplink | --qr=code (env: TRICODER_QR)
  // Default is 'code' (smaller QR), use --qr=deeplink for OS camera
  const arg = process.argv.find((a) => a.startsWith('--qr='));
  const val = (arg ? arg.split('=')[1] : (process.env.TRICODER_QR || '')).toLowerCase();
  return val === 'deeplink' ? 'deeplink' : 'code';
})();
let CONVEX_DL_PCT = -1;

function lite(s: string) { return chalk.hex('#9CA3AF')(s); }
let RESOURCES_PRINTED = false;
function printResourcesOnce() {
  if (RESOURCES_PRINTED) return;
  RESOURCES_PRINTED = true;
  console.log("");
  console.log(lite("Resources:"));
  console.log(lite(" - All code is open-source here: https://github.com/OpenAgentsInc/openagents"));
  console.log(lite(" - Download the iOS app on TestFlight: https://testflight.apple.com/join/dvQdns5B"));
  console.log(lite("   - Android coming soon"));
  console.log(lite(" - Any questions? Please @ us on X: https://x.com/OpenAgentsInc"));
  console.log(lite(" - Any bugs please open an issue: https://github.com/OpenAgentsInc/openagents/issues"));
}

function findRepoRoot(startDir: string): string | null {
  let dir = startDir;
  const root = dirname(dir) === dir ? dir : undefined;
  while (true) {
    if (existsSync(join(dir, "Cargo.toml")) && existsSync(join(dir, "crates", "oa-tunnel"))) {
      return dir;
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

function hasCmd(cmd: string): boolean {
  try {
    const res = spawnSync(process.platform === 'win32' ? 'where' : 'which', [cmd], { stdio: 'pipe' });
    return res.status === 0;
  } catch {
    return false;
  }
}

function getVersion(cmd: string, args: string[] = ["--version"]): string | null {
  try {
    const res = spawnSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    if (res.status === 0) return String(res.stdout || res.stderr || '').trim().split(/\r?\n/)[0] || null;
    return null;
  } catch {
    return null;
  }
}

function parseSemver(s: string | null | undefined): [number, number, number] | null {
  if (!s) return null;
  const m = String(s).match(/(\d+)\.(\d+)\.(\d+)/);
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function cmpSemver(a: [number, number, number], b: [number, number, number]): number {
  if (a[0] !== b[0]) return a[0] - b[0];
  if (a[1] !== b[1]) return a[1] - b[1];
  return a[2] - b[2];
}

function printEnvAssessment(repoRoot: string | null) {
  const header = chalk.bold("Environment assessment");
  const ok = (s: string) => chalk.green(`✔ ${s}`);
  const warn = (s: string) => chalk.yellow(`◔ ${s}`);
  const bad = (s: string) => chalk.red(`✘ ${s}`);

  const platform = `${process.platform} ${process.arch}`;
  const rust = hasCmd('rustc') && hasCmd('cargo');
  const rustcV = rust ? getVersion('rustc') : null;
  const cargoV = rust ? getVersion('cargo') : null;
  const git = hasCmd('git');
  const bun = hasCmd('bun');
  const bunx = hasCmd('bunx');
  const npx = hasCmd('npx');
  const codex = hasCmd('codex');
  const codexV = codex ? getVersion('codex', ['--version']) : null;

  console.log("");
  console.log(header);
  console.log("- " + ok(`Platform ${platform}`));
  console.log("- " + (repoRoot ? ok(`OpenAgents repo found at ${repoRoot}`) : warn("OpenAgents repo not found (will not be required in future)")));
  if (rust) {
    console.log("- " + ok(`Rust toolchain present (${rustcV || 'rustc'}; ${cargoV || 'cargo'})`));
  } else {
    console.log("- " + warn("Rust toolchain not found (required today to run the bridge + tunnels)"));
  }
  console.log("- " + (git ? ok("git present") : warn("git not found (needed to bootstrap the repo if not present)")));
  if (bun) {
    console.log("- " + ok("bun present"));
  } else if (npx) {
    console.log("- " + warn("bun not found; will fall back to npx for Convex CLI where possible"));
  } else {
    console.log("- " + warn("bun/npx missing; Convex CLI bootstrap may be skipped"));
  }
  if (codex) {
    const minStr = process.env.TRICODER_MIN_CODEX || '0.50.0';
    const have = codexV || '';
    const haveT = parseSemver(codexV);
    const minT = parseSemver(minStr);
    const cmp = haveT && minT ? cmpSemver(haveT, minT) : 0;
    if (!have) {
      console.log("- " + warn("codex present (version unknown)"));
    } else if (cmp < 0) {
      const sev = haveT && cmpSemver(haveT, [0,30,0]) < 0 ? chalk.red : chalk.yellow;
      console.log("- " + sev(`codex ${have} detected — recommended >= ${minStr}. Please upgrade: https://developers.openai.com/codex/cli`));
    } else {
      console.log("- " + ok(`codex ${have}`));
    }
  } else {
    console.log("- " + bad("codex binary NOT detected"));
    console.log(chalk.red("Install Codex CLI: https://developers.openai.com/codex/cli"));
    process.exit(1);
  }
  console.log("");
}

async function main() {
  console.info(chalk.bold("OpenAgents Tricoder - Desktop Bridge"));
  // Destructive reset: remove local clones, downloaded binaries, and Convex state
  if (process.argv.includes("--delete")) {
    try { await destructiveReset(); } catch {}
    return;
  }
  let repoRoot = findRepoRoot(process.cwd());
  // Brief overview and resources before assessment
  console.log(chalk.cyanBright("\nSetup overview"));
  console.log(lite(" - Checks your environment (Rust, git, Bun/NPM, codex)"));
  console.log(lite(" - Clones/updates the OpenAgents repo if missing (~/.openagents/openagents)"));
  console.log(lite(" - Builds the Rust bridge and tunnel, then starts the bridge"));
  console.log(lite(" - Starts the local Convex backend and deploys functions (best effort)"));
  console.log(lite(" - Optionally opens public tunnels and prints a pairing code"));
  console.log(chalk.yellowBright("\nImportant: The mobile app and desktop features won’t fully work until the Convex backend is installed and healthy."));
  console.log(chalk.yellowBright("\nNote: First setup may take ~5 minutes on slower machines due to local Rust builds."));
  printResourcesOnce();
  // Always print a quick assessment so users see what's missing
  printEnvAssessment(repoRoot);

  if (!repoRoot) {
    // Attempt to clone the repo into ~/.openagents/openagents for a one-command experience
    const home = os.homedir();
    const target = join(home, ".openagents", "openagents");
    console.log(chalk.yellow("No local OpenAgents repo detected — cloning to ~/.openagents/openagents…"));
    if (!hasCmd('git')) {
      console.log(chalk.red("git is required to clone the repository. Please install git and re-run."));
      process.exit(1);
    }
    try {
      // mkdir -p ~/.openagents
      spawnSync(process.platform === 'win32' ? 'cmd' : 'mkdir', process.platform === 'win32' ? ['/c', 'mkdir', target] : ['-p', target], { stdio: 'ignore' });
    } catch {}
    // If target doesn't look like a repo, clone; else pull
    if (!existsSync(join(target, '.git'))) {
      const res = spawnSync('git', ['clone', '--depth', '1', 'https://github.com/OpenAgentsInc/openagents', target], { stdio: 'inherit' });
      if (res.status !== 0) {
        console.log(chalk.red("Failed to clone the OpenAgents repository."));
        process.exit(1);
      }
    } else {
      console.log(chalk.dim("Updating existing ~/.openagents/openagents…"));
      spawnSync('git', ['-C', target, 'pull', '--ff-only'], { stdio: VERBOSE ? 'inherit' : 'ignore' });
    }
    repoRoot = target;
  }

  // Ensure Convex backend binary early with visible progress (best effort)
  try { ensureConvexBinaryWithProgress(); } catch { }

  // Ensure Rust toolchain
  ensureRustToolchain();

  // Optionally prebuild with conservative parallelism (Linux defaults to 2 jobs)
  prebuildCrates(repoRoot);

  // Ensure the Rust bridge is running locally (best effort)
  ensureBridgeRunning(repoRoot);

  // Launch Bridge tunnel (local 8787)
  const child = LOCAL_ONLY ? null : spawn("cargo", buildTunnelArgs(8787, "bore.pub"), {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let bridgeUrl: string | null = null;
  let convexUrl: string | null = null;
  let printedCombined = false;
  child?.stdout?.setEncoding("utf8");
  child?.stdout?.on("data", (chunk: string) => {
    const lines = chunk.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      const isUrl = line.startsWith("ws://") || line.startsWith("wss://");
      if (isUrl && !bridgeUrl) {
        bridgeUrl = line.trim();
        if (VERBOSE) console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
        // After bridge URL, launch Convex tunnel
        if (!LOCAL_ONLY) launchConvexTunnel(repoRoot, (url) => {
          convexUrl = url;
          if (VERBOSE) console.log(chalk.dim(`[convex-public] ${convexUrl}`));
          maybePrintPairCode(bridgeUrl!, convexUrl!);
          // Public probes
          if (VERBOSE) { try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch { } }
          if (VERBOSE) { try { if (convexUrl) probePublicConvex(convexUrl); } catch { } }
          // Connectivity summary
          if (VERBOSE) console.log(chalk.dim(`[pair] bridge=${bridgeUrl} convex=${convexUrl}`));
          // Seed a demo thread via bridge controls to ensure history appears
          if (VERBOSE) { try { seedDemoViaBridgeControl(); } catch { } }
        });
        // Start local health probes (status changes only)
        if (VERBOSE) startLocalProbes(repoRoot);
      }
    }
  });
  child?.stderr?.setEncoding("utf8");
  // Aggregate noisy bore logs
  let bridgeConnNew = 0, bridgeConnExit = 0;
  setInterval(() => {
    if (VERBOSE && (bridgeConnNew || bridgeConnExit)) {
      console.log(chalk.dim(`[bridge-tunnel] ${bridgeConnNew} new, ${bridgeConnExit} exited (last 10s)`));
      bridgeConnNew = 0; bridgeConnExit = 0;
    }
  }, 10000);
  child?.stderr?.on("data", (chunk: string) => {
    if (!VERBOSE) return;
    for (const line of String(chunk).split(/\r?\n/).filter(Boolean)) {
      if (/bore_cli::client: new connection/.test(line)) { bridgeConnNew++; continue; }
      if (/bore_cli::client: connection exited/.test(line)) { bridgeConnExit++; continue; }
      console.error(chalk.dim(`[bridge-tunnel] ${line}`));
    }
  });

  function maybePrintPairCode(b: string, c: string) {
    if (printedCombined) return;
    if (!b || !c) return;
    printedCombined = true;
    const payload = {
      v: 1,
      type: "openagents-bridge",
      provider: "bore",
      bridge: b,
      convex: c,
      token: readBridgeToken() as string | null,
    };
    const code = encodePairCode(payload);
    const deeplink = `openagents://connect?j=${code}`;
    console.log("\nPaste this code into the mobile app Settings → Bridge Code, or scan the QR below:\n");
    console.log(chalk.greenBright(code));
    console.log("");
    if (!NO_QR) {
      const qrPayload = (QR_MODE === 'deeplink') ? deeplink : code;
      try {
        printBrailleQR(qrPayload);
      } catch {
        try { (qrcode as any).setErrorLevel?.('L') } catch {}
        qrcode.generate(qrPayload, { small: true });
      }
      console.log("");
    }
    console.log(lite("Deep link:"), chalk.cyan(deeplink));
    // Security notice
    console.log(chalk.yellowBright("\nWarning: This code is your private bridge token — never share it with anyone."));
    console.log("\nTunnel is active. Leave this running to stay connected.\n");
    console.log(chalk.yellowBright("Heads up: The app depends on the local Convex backend. If you don’t see threads/messages updating yet, wait until Convex reports healthy and functions finish deploying."));
    // Helpful resources (light gray, once)
    printResourcesOnce();
    // Check for codex binary presence — required for assistant responses
    try {
      const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['codex'], { stdio: 'pipe' });
      const ok = probe.status === 0;
      if (VERBOSE) console.log(ok ? chalk.dim('[codex] codex binary found in PATH') : chalk.yellow('[codex] codex binary NOT found — assistant responses will not stream'));
      // Bridge status via WS
      if (VERBOSE) { try { bridgeStatus(); } catch { } }
      // Start persistent WS tail to print bridge + codex events
      if (VERBOSE) { try { startBridgeEventTail(); } catch { } }
    } catch { }
    // In quiet mode, provide a concise Convex setup guide while backend initializes
    if (!VERBOSE) {
      try { monitorConvexSetupOnce(repoRoot as string) } catch { }
    }
  }
}

function ensureRustToolchain() {
  const rust = hasCmd('rustc') && hasCmd('cargo');
  if (rust) return;
  if (process.platform === 'win32') {
    console.log(chalk.yellow("Rust toolchain not found. Please install Rust from https://rustup.rs and re-run."));
    process.exit(1);
  }
  if (!ASSUME_YES) {
    const rl = require('node:readline').createInterface({ input: process.stdin, output: process.stdout });
    const ask = (q: string) => new Promise<string>(res => rl.question(q, (a: string) => res(a)));
    ask("Rust toolchain not found. Install rustup now? [y/N] ").then((ans: string) => {
      rl.close();
      if (!/^y(es)?$/i.test(ans.trim())) {
        console.log("Aborting; Rust is required.");
        process.exit(1);
      }
      installRustup();
    });
  } else {
    installRustup();
  }
}

function installRustup() {
  console.log(chalk.dim("Installing Rust via rustup (non-interactive)…"));
  const cmd = spawnSync('sh', ['-c', 'curl https://sh.rustup.rs -sSf | sh -s -- -y'], { stdio: 'inherit' });
  if (cmd.status !== 0) {
    console.log(chalk.red("Failed to install rustup. Please install from https://rustup.rs and re-run."));
    process.exit(1);
  }
  // Prepend ~/.cargo/bin to PATH for this process
  try {
    const cargoBin = join(os.homedir(), '.cargo', 'bin');
    process.env.PATH = `${cargoBin}:${process.env.PATH || ''}`;
  } catch {}
}

function prebuildCrates(repoRoot: string) {
  const env = { ...process.env } as Record<string, string>;
  if (process.platform === 'linux' && !env.CARGO_BUILD_JOBS) env.CARGO_BUILD_JOBS = '2';
  const run = (args: string[]) => spawnSync('cargo', args, { cwd: repoRoot, stdio: VERBOSE ? 'inherit' : 'ignore', env });
  // Visual separation, and match the Convex setup cyan
  console.log("");
  console.log(chalk.cyanBright("Checking/building required Rust crates (codex-bridge, oa-tunnel)…"));
  // Build codex-bridge first
  let r = run(['build', '-p', 'codex-bridge']);
  if (r.status !== 0) {
    console.log(chalk.yellow("cargo build -p codex-bridge failed; will rely on on-demand build during run."));
  }
  // Then oa-tunnel
  r = run(['build', '-p', 'oa-tunnel']);
  if (r.status !== 0) {
    console.log(chalk.yellow("cargo build -p oa-tunnel failed; will rely on on-demand build during run."));
  }
}

function ensureConvexBinaryWithProgress() {
  const home = os.homedir();
  const outDir = join(home, ".openagents", "bin");
  const outBin = join(outDir, process.platform === 'win32' ? 'local_backend.exe' : 'local_backend');
  if (existsSync(outBin)) return;
  const haveBunx = hasCmd('bunx');
  const haveNpx = hasCmd('npx');
  console.log("");
  console.log(chalk.cyanBright("Downloading Convex local backend (first-time only)…"));
  // Try direct download first; fallback to bunx/npx only if that fails
  directDownloadConvexBackend(outBin).then((ok) => {
    if (ok) return;
    if (!haveBunx && !haveNpx) return; // will fall back to bridge bootstrap later
    let lastPct = -1;
    const args = ["convex", "dev", "--configure", "--dev-deployment", "local", "--once", "--skip-push", "--local-force-upgrade"];
    const env = { ...process.env, CI: '1' } as Record<string,string>;
    const child = haveBunx
      ? spawn("bunx", args, { stdio: ["ignore", "pipe", "pipe"], env })
      : spawn("npx", ["-y", ...args], { stdio: ["ignore", "pipe", "pipe"], env });
    const show = (p: number) => {
      if (p < 0 || p > 100) return;
      if (p <= lastPct) return;
      lastPct = p; CONVEX_DL_PCT = p;
      try { process.stdout.write("\r" + chalk.cyanBright(`⬇️  Convex backend download: ${p}%`)); } catch {}
    };
    const maybeParse = (s: string) => {
      const m = s.match(/(\d{1,3})%/);
      if (m) { const pct = Math.max(0, Math.min(100, parseInt(m[1], 10))); show(pct); }
    };
    child.stdout?.setEncoding('utf8'); child.stdout?.on('data', (d) => { String(d).split(/\r?\n/).forEach(maybeParse); });
    child.stderr?.setEncoding('utf8'); child.stderr?.on('data', (d) => { String(d).split(/\r?\n/).forEach(maybeParse); });
    const done = () => {
      try { process.stdout.write("\r\x1b[K"); } catch {}
      const cacheRoot = join(home, '.cache', 'convex', 'binaries');
      const candidate = findNewestBackendBinary(cacheRoot);
      if (candidate) {
        try { mkdirSync(outDir, { recursive: true }); copyFileSync(candidate, outBin); try { chmodSync(outBin, 0o755); } catch {}; console.log(chalk.greenBright("✔ Convex backend installed.")); } catch (e: any) { console.log(chalk.yellow(`Convex backend cached but copy failed: ${e?.message || e}`)); }
      } else {
        console.log(chalk.yellow("Convex CLI finished but backend binary not found in cache (will let the bridge retry)."));
      }
    };
    child.on('exit', () => done());
    setTimeout(() => { try { child.kill(); } catch {} }, 180000);
  }).catch(() => { /* ignore; bridge will retry */ });
}

function findNewestBackendBinary(root: string): string | null {
  try {
    const entries = readdirSync(root, { encoding: 'utf8' }) as unknown as string[];
    let best: { path: string, mtime: number } | null = null;
    for (const dir of entries) {
      const d = join(root, dir);
      try {
        const files = readdirSync(d, { encoding: 'utf8' }) as unknown as string[];
        for (const f of files) {
          if (!/local_backend(\.exe)?$/.test(f) && !/convex-local-backend(\.exe)?$/.test(f)) continue;
          const p = join(d, f);
          const st = statSync(p);
          if (!best || st.mtimeMs > best.mtime) best = { path: p, mtime: st.mtimeMs };
        }
      } catch { }
    }
    return best?.path || null;
  } catch { return null; }
}

async function directDownloadConvexBackend(outBin: string): Promise<boolean> {
  try {
    const triple = (() => {
      switch (process.platform) {
        case 'darwin':
          return process.arch === 'arm64' ? 'aarch64-apple-darwin' : (process.arch === 'x64' ? 'x86_64-apple-darwin' : null);
        case 'linux':
          return process.arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : (process.arch === 'x64' ? 'x86_64-unknown-linux-gnu' : null);
        case 'win32':
          return 'x86_64-pc-windows-msvc';
        default: return null;
      }
    })();
    if (!triple) return false;
    const filename = `convex-local-backend-${triple}.zip`;
    const version = await findLatestConvexVersionWithBinary(filename);
    if (!version) return false;
    const url = `https://github.com/get-convex/convex-backend/releases/download/${version}/${filename}`;
    const tmpZip = join(os.tmpdir(), `convex-${Date.now()}.zip`);
    await downloadWithProgress(url, tmpZip, (pct) => { CONVEX_DL_PCT = pct; try { process.stdout.write("\r" + chalk.cyanBright(`⬇️  Convex backend download: ${pct}%`)); } catch {} });
    const zip = new AdmZip(tmpZip);
    const entries = zip.getEntries();
    const entry = entries.find(e => /convex-local-backend(\.exe)?$/.test(e.entryName));
    if (!entry) return false;
    const outDir = dirname(outBin); mkdirSync(outDir, { recursive: true });
    zip.extractEntryTo(entry, outDir, false, true);
    const extractedPath = join(outDir, entry.entryName);
    if (extractedPath !== outBin) {
      try { copyFileSync(extractedPath, outBin); } catch {}
    }
    try { chmodSync(outBin, 0o755); } catch {}
    try { process.stdout.write("\r\x1b[K"); } catch {}
    console.log(chalk.greenBright("✔ Convex backend installed."));
    // Opportunistically start the backend now so the spinner can turn healthy
    try { await tryStartConvexBackendIfNeeded(outBin); } catch {}
    return true;
  } catch {
    return false;
  }
}

async function findLatestConvexVersionWithBinary(filename: string): Promise<string | null> {
  let nextUrl: string | '' = 'https://api.github.com/repos/get-convex/convex-backend/releases?per_page=30';
  while (nextUrl) {
    const res = await fetch(nextUrl as any, { headers: { 'User-Agent': 'openagents-tricoder' } } as any);
    if (!res.ok) return null;
    const releases = await res.json() as any[];
    for (const r of releases) {
      if (r.prerelease || r.draft) continue;
      const assets = Array.isArray(r.assets) ? r.assets : [];
      if (assets.find((a: any) => a.name === filename)) {
        return r.tag_name as string;
      }
    }
    const link = res.headers.get('Link') || res.headers.get('link');
    if (!link) break;
    const m = /<([^>]+)>;\s*rel="next"/.exec(link);
    nextUrl = m ? (m[1] as string) : '';
  }
  return null;
}

async function downloadWithProgress(url: string, dest: string, onPct: (pct: number) => void): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const req = https.get(url, { headers: { 'User-Agent': 'openagents-tricoder' } }, (res) => {
      if (res.statusCode && res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        downloadWithProgress(res.headers.location, dest, onPct).then(resolve, reject); return;
      }
      if (!res.statusCode || res.statusCode < 200 || res.statusCode >= 300) { reject(new Error(`HTTP ${res.statusCode}`)); return; }
      const total = Number(res.headers['content-length'] || 0);
      let done = 0;
      const out = createWriteStream(dest);
      res.on('data', (chunk) => { done += chunk.length; if (total > 0) { const pct = Math.min(100, Math.max(0, Math.round((done / total) * 100))); onPct(pct); } });
      res.pipe(out);
      out.on('finish', () => { try { out.close(); } catch {}; onPct(100); resolve(); });
      res.on('error', (e) => { try { out.destroy(); } catch {}; reject(e); });
    });
    req.on('error', reject);
  });
}

async function tryStartConvexBackendIfNeeded(binPath: string): Promise<void> {
  const healthy = await new Promise<boolean>((resolve) => {
    const req = http.get({ host: '127.0.0.1', port: 7788, path: '/instance_version', timeout: 1200 }, (res) => {
      resolve(!!res.statusCode && res.statusCode >= 200 && res.statusCode < 300);
      res.resume();
    });
    req.on('error', () => resolve(false));
  });
  if (healthy) return;
  const db = join(os.homedir(), '.openagents', 'convex', 'data.sqlite3');
  const storage = join(os.homedir(), '.openagents', 'convex', 'storage');
  try { mkdirSync(join(os.homedir(), '.openagents', 'convex'), { recursive: true }); } catch {}
  const args = [
    db,
    '--db', 'sqlite',
    '--interface', '0.0.0.0',
    '--port', '7788',
    '--local-storage', storage,
    '--disable-beacon',
  ];
  try { spawn(binPath, args, { stdio: 'ignore' }); } catch {}
}

function launchConvexTunnel(repoRoot: string, onUrl: (url: string) => void) {
  // Run a second tunnel for Convex (local 7788). We'll emit an HTTP URL for override.
  const child = spawn("cargo", buildTunnelArgs(7788, "bore.pub"), {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let printed = false;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    const lines = chunk.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      const m = line.match(/^ws:\/\/([^:]+):(\d+)\/ws$/);
      if (m && !printed) {
        printed = true;
        const host = m[1];
        const port = m[2];
        const httpUrl = `http://${host}:${port}`;
        onUrl(httpUrl);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  let convexConnNew = 0, convexConnExit = 0;
  setInterval(() => {
    if (VERBOSE && (convexConnNew || convexConnExit)) {
      console.log(chalk.dim(`[convex-tunnel] ${convexConnNew} new, ${convexConnExit} exited (last 10s)`));
      convexConnNew = 0; convexConnExit = 0;
    }
  }, 10000);
  child.stderr.on("data", (chunk: string) => {
    if (!VERBOSE) return;
    for (const line of String(chunk).split(/\r?\n/).filter(Boolean)) {
      if (/bore_cli::client: new connection/.test(line)) { convexConnNew++; continue; }
      if (/bore_cli::client: connection exited/.test(line)) { convexConnExit++; continue; }
      console.error(chalk.dim(`[convex-tunnel] ${line}`));
    }
  });
}

function encodePairCode(obj: any): string {
  const json = JSON.stringify(obj);
  const b64 = Buffer.from(json, "utf8").toString("base64");
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function ensureBridgeRunning(repoRoot: string) {
  const sock = net.createConnection({ host: "127.0.0.1", port: 8787 });
  let connected = false;
  sock.once("connect", async () => {
    connected = true; try { sock.end(); } catch { }
    const force = (process.env.TRICODER_FORCE_RESTART || '1') !== '0';
    // Probe whether current bridge supports echo. If not, or if forced, restart it.
    const supports = await probeBridgeEchoOnce(700).catch(() => false);
    if (force || !supports) {
      if (VERBOSE) console.log(chalk.dim(`Restarting local bridge with debug enabled (${force ? 'forced' : 'no echo support'})…`));
      try { restartBridgeProcess(repoRoot); } catch { }
    }
  });
  sock.once("error", () => {
    // Not listening; start it
    startBridgeProcess(repoRoot);
  });
  // timeout after brief period
  setTimeout(() => { try { if (!connected) sock.destroy(); } catch { } }, 500);
}

function startLocalProbes(repoRoot: string) {
  let lastBridgeOk: boolean | null = null;
  let lastConvexOk: boolean | null = null;
  const probeBridge = () => {
    const s = net.createConnection({ host: "127.0.0.1", port: 8787 });
    let ok = false;
    s.once("connect", () => { ok = true; try { s.end(); } catch { } });
    s.once("error", () => { ok = false; });
    s.once("close", () => {
      if (VERBOSE && lastBridgeOk !== ok) {
        lastBridgeOk = ok;
        console.log(ok ? chalk.dim("[bridge-local] 127.0.0.1:8787 reachable") : chalk.dim("[bridge-local] 127.0.0.1:8787 not reachable"));
      }
    });
    setTimeout(() => { try { s.destroy(); } catch { } }, 500);
  };
  const probeConvex = () => {
    const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 700 }, (res) => {
      const ok = res.statusCode !== undefined && res.statusCode >= 200 && res.statusCode < 300;
      if (VERBOSE && lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(ok ? chalk.dim("[convex-local] http://127.0.0.1:7788 healthy") : chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
      res.resume();
      // On first healthy, try function push once via root script (best effort)
      if (ok && (probeConvex as any)._firstDone !== true) {
        (probeConvex as any)._firstDone = true;
        tryPushConvexFunctions(repoRoot, !VERBOSE);
      }
    });
    req.on("error", () => {
      const ok = false;
      if (VERBOSE && lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
    });
    req.setTimeout(800, () => { try { req.destroy(); } catch { } });
  };
  // Kick immediately and then poll
  probeBridge();
  probeConvex();
  setInterval(probeBridge, 5000);
  setInterval(probeConvex, 5000);
}

function monitorConvexSetupOnce(repoRoot: string) {
  const backendPath = join(os.homedir(), ".openagents", "bin", "local_backend")
  const exists = existsSync(backendPath)
  const startMsg = !exists
    ? "Setting up local Convex backend (first run downloads a small binary)…"
    : "Starting local Convex backend…"
  // Add a blank line before the status and show a spinner while waiting
  console.log("")
  const frames = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]
  let fi = 0
  let spun = false
  const spin = setInterval(() => {
    spun = true
    const extra = CONVEX_DL_PCT >= 0 ? chalk.cyanBright(` ⬇️ ${CONVEX_DL_PCT}%`) : ""
    try { process.stdout.write("\r" + chalk.cyanBright(`⏳ ${startMsg} ${frames[fi]}`) + extra) } catch {}
    fi = (fi + 1) % frames.length
  }, 120)
  const start = Date.now()
  const finish = (okSeconds: number) => {
    try { clearInterval(spin) } catch {}
    // Clear spinner line
    if (spun) { try { process.stdout.write("\r\x1b[K") } catch {} }
    console.log(chalk.greenBright(`✔ Convex backend ready in ${okSeconds}s.`))
  }
  const timeoutMs = 120000; // 2 minutes
  const startAt = Date.now();
  const check = () => {
    const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 1500 }, (res) => {
      if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
        finish(Math.round((Date.now() - start) / 1000))
      } else {
        if (Date.now() - startAt > timeoutMs) return onTimeout();
        setTimeout(check, 1000)
      }
      res.resume()
    })
    req.on("error", () => {
      if (Date.now() - startAt > timeoutMs) return onTimeout();
      setTimeout(check, 1200)
    })
  }
  check()

  const onTimeout = () => {
    try { clearInterval(spin) } catch {}
    if (spun) { try { process.stdout.write("\r\x1b[K") } catch {} }
    console.log(chalk.redBright("✘ Convex backend did not become healthy within 2 minutes."))
    // Try a direct start using the installed binary if present
    try {
      const bin = join(os.homedir(), ".openagents", "bin", process.platform === 'win32' ? 'local_backend.exe' : 'local_backend')
      if (existsSync(bin)) {
        console.log(chalk.cyanBright("Attempting to start the Convex backend directly…"))
        const db = join(os.homedir(), ".openagents", "convex", "data.sqlite3")
        const storage = join(os.homedir(), ".openagents", "convex", "storage")
        const child = spawn(bin, [
          db,
          "--db", "sqlite",
          "--interface", "0.0.0.0",
          "--port", "7788",
          "--local-storage", storage,
          "--disable-beacon",
        ], { stdio: "ignore" })
        setTimeout(() => {
          const req2 = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 1500 }, (res2) => {
            if (res2.statusCode && res2.statusCode >= 200 && res2.statusCode < 300) {
              console.log(chalk.greenBright("✔ Convex backend started."))
            } else {
              printConvexHelp()
            }
            res2.resume()
          })
          req2.on('error', () => printConvexHelp())
        }, 1500)
      } else {
        printConvexHelp()
      }
    } catch {
      printConvexHelp()
    }
  }

  function printConvexHelp() {
    console.log(chalk.yellow("If this persists, try one of the following:"))
    console.log(lite(" - Ensure Bun or Node/NPM are installed so tricoder can fetch the backend"))
    console.log(lite(" - Run: bunx convex dev  (or: npx -y convex dev) once to install the local backend"))
    console.log(lite(" - Then re-run: npx tricoder"))
  }
}

function tryPushConvexFunctions(repoRoot: string, announce?: boolean) {
  try {
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Deploying functions…`));
    const haveBun = hasCmd('bun');
    const haveNpx = hasCmd('npx');
    const localUrl = 'http://127.0.0.1:7788';
    const admin = process.env.CONVEX_ADMIN_KEY || process.env.CONVEX_SELF_HOSTED_ADMIN_KEY || 'carnitas|017c5405aba48afe1d1681528424e4528026e69e3b99e400ef23f2f3741a11db225497db09';
    const convexEnv = {
      ...process.env,
      CONVEX_URL: localUrl,
      CONVEX_SELF_HOSTED_URL: localUrl,
      CONVEX_ADMIN_KEY: admin,
      CONVEX_SELF_HOSTED_ADMIN_KEY: admin,
      CI: '1',
    } as Record<string, string>;
    if (haveBun) {
      const child = spawn("bun", ["run", "convex:dev:once"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"], env: convexEnv });
      child.stdout?.setEncoding("utf8");
      child.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      child.stderr?.setEncoding("utf8");
      child.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      child.on("exit", (code) => {
        if (code !== 0) {
          if (VERBOSE) console.log(chalk.dim(`[convex-bootstrap] script missing or failed; trying 'bunx convex dev' one-shot non-interactive…`));
          // Do NOT skip push — we need functions present
          const fallback = spawn("bunx", ["convex", "dev", "--configure", "--dev-deployment", "local", "--once", "--local-force-upgrade"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"], env: convexEnv });
          fallback.stdout?.setEncoding("utf8");
          fallback.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
          fallback.stderr?.setEncoding("utf8");
          fallback.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
          fallback.on("exit", (c2) => { if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${c2 ?? 0})`)) });
        } else {
          if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${code ?? 0})`));
        }
      });
      return;
    }
    if (haveNpx) {
      if (VERBOSE) console.log(chalk.dim(`[convex-bootstrap] bun not found; trying 'npx convex dev' one-shot non-interactive…`));
      const fallback = spawn("npx", ["-y", "convex", "dev", "--configure", "--dev-deployment", "local", "--once", "--local-force-upgrade"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"], env: convexEnv });
      fallback.stdout?.setEncoding("utf8");
      fallback.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      fallback.stderr?.setEncoding("utf8");
      fallback.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      fallback.on("exit", (c2) => { if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${c2 ?? 0})`)) });
      return;
    }
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Skipping functions deploy: bun/npx not available`));
  } catch (e: any) {
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy skipped: ${e?.message || e}`));
  }
}

function probePublicConvex(base: string) {
  try {
    const u = new URL(base);
    const opts: http.RequestOptions = { host: u.hostname, port: Number(u.port || 80), path: "/instance_version", timeout: 2000 };
    const req = http.get(opts, (res) => {
      const code = res.statusCode || 0;
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (c) => (body += c));
      res.on("end", () => {
        const snippet = body.trim().slice(0, 80).replace(/\s+/g, " ");
        if (VERBOSE) console.log(chalk.dim(`[convex-public-check] GET ${u.hostname}:${u.port}/instance_version -> ${code} ${snippet ? `body: ${snippet}` : ""}`));
      });
    });
    req.on("error", (e: any) => { if (VERBOSE) console.log(chalk.dim(`[convex-public-check] error: ${e?.message || e}`)); });
    req.setTimeout(2500, () => { try { req.destroy(); } catch { } });
  } catch (e: any) {
    if (VERBOSE) console.log(chalk.dim(`[convex-public-check] invalid URL: ${String(e?.message || e)}`));
  }
}

function probePublicBridge(wsUrl: string) {
  try {
    const u = new URL(wsUrl.replace(/^ws:/, "http:").replace(/^wss:/, "https:"));
    const host = u.hostname;
    const port = Number(u.port || (u.protocol === "https:" ? 443 : 80));
    const path = u.pathname || "/ws";
    const key = Buffer.from(Math.random().toString()).toString("base64");
    const headers =
      `GET ${path} HTTP/1.1\r\n` +
      `Host: ${host}\r\n` +
      `Upgrade: websocket\r\n` +
      `Connection: Upgrade\r\n` +
      `Sec-WebSocket-Key: ${key}\r\n` +
      `Sec-WebSocket-Version: 13\r\n` +
      `\r\n`;
    const s = net.createConnection({ host, port }, () => {
      s.write(headers);
    });
    let buf = "";
    s.on("data", (chunk) => {
      buf += String(chunk);
      if (buf.includes("\r\n\r\n")) {
        const first = buf.split(/\r?\n/)[0] || "";
        if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] ${first}`));
        try { s.destroy(); } catch { }
      }
    });
    s.on("error", (e: any) => { if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] error: ${e?.message || e}`)); });
    setTimeout(() => { try { s.destroy(); } catch { } }, 2500);
  } catch (e: any) {
    if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] invalid URL: ${String(e?.message || e)}`));
  }
}

function seedDemoViaBridgeControl() {
  const ws = new WebSocket("ws://127.0.0.1:8787/ws");
  let done = false;
  const timer = setTimeout(() => { try { ws.close(); } catch { } }, 4000);
  ws.on("open", () => {
    try {
      ws.send(JSON.stringify({ control: "convex.create_demo" }));
      ws.send(JSON.stringify({ control: "convex.create_demo_thread" }));
      ws.send(JSON.stringify({ control: "convex.create_threads" }));
      ws.send(JSON.stringify({ control: "convex.status" }));
    } catch { }
  });
  ws.on("message", (data: WebSocket.RawData) => {
    const s = String(data || "").trim();
    if (!s.startsWith("{")) return;
    try {
      const obj = JSON.parse(s);
      if (obj?.type === "bridge.convex_status") {
        const url = (obj.url || obj.convex_url || "") as string;
        if (VERBOSE) console.log(chalk.dim(`[bridge-control] convex.status -> ${obj.healthy ? "healthy" : "unhealthy"} url=${url}`));
      }
      if (obj?.type === "bridge.status") {
        if (VERBOSE) console.log(chalk.dim(`[bridge-status] bind=${obj.bind} convex_healthy=${obj.convex_healthy} codex_pid=${obj.codex_pid || 'none'}`));
      }
      if (obj?.type === "bridge.projects") {
        if (VERBOSE) console.log(chalk.dim(`[bridge-control] projects -> ${Array.isArray(obj.items) ? obj.items.length : 0} items`));
      }
    } catch { }
  });
  ws.on("close", () => { if (!done) clearTimeout(timer); done = true; });
  ws.on("error", () => { if (!done) clearTimeout(timer); done = true; });
}

function bridgeStatus() {
  const ws = new WebSocket("ws://127.0.0.1:8787/ws");
  const timer = setTimeout(() => { try { ws.close(); } catch { } }, 1800);
  ws.on("open", () => {
    try { ws.send(JSON.stringify({ control: 'bridge.status' })); } catch { }
  });
  ws.on("error", () => { try { clearTimeout(timer); ws.close(); } catch { } });
  ws.on("close", () => { try { clearTimeout(timer); } catch { } });
}

function startBridgeEventTail() {
  let closed = false;
  const connect = () => {
    if (closed) return;
    const ws = new WebSocket("ws://127.0.0.1:8787/ws");
    ws.on("open", () => {
      // no-op: we just tail broadcast feed
    });
    ws.on("message", (data: WebSocket.RawData) => {
      const s = String(data || "").trim();
      if (!s) return;
      if (s.startsWith("{")) {
        try {
          const obj = JSON.parse(s);
          const t = obj?.type || obj?.msg?.type || '';
          if (VERBOSE && String(t).startsWith('bridge.')) {
            // Print richer details for common debug events
            if (t === 'bridge.control' && typeof obj.raw === 'string') {
              console.log(chalk.dim(`[bridge-control] raw=${obj.raw}`));
              return;
            }
            if (t === 'bridge.convex_write') {
              const op = obj.op || '';
              const ok = obj.ok ? 'ok' : 'fail';
              const kind = obj.kind || '';
              const len = obj.len || 0;
              const itemId = obj.itemId || '';
              console.log(chalk.dim(`[convex-write] ${op} ${ok} kind=${kind} item=${itemId} len=${len}`));
              return;
            }
            if (t === 'bridge.ws_in' && typeof obj.preview === 'string') {
              console.log(chalk.dim(`[bridge-in] ${obj.preview}`));
              return;
            }
            if (t === 'bridge.run_submit') {
              console.log(chalk.dim(`[bridge] bridge.run_submit threadDocId=${obj.threadDocId || ''} len=${obj.len || 0}`));
              return;
            }
            if (t === 'bridge.client_connected' || t === 'bridge.client_disconnected') {
              console.log(chalk.dim(`[bridge] ${t}`));
              return;
            }
            if (t === 'bridge.echo') {
              console.log(chalk.dim(`[bridge-echo] tag=${obj.tag || ''} payload=${obj.payload || ''}`));
              return;
            }
            console.log(chalk.dim(`[bridge] ${t}`));
            return;
          }
          // surface codex JSONL events succinctly
          if (VERBOSE && t && /agent_message|assistant|message|reason|exec_begin|exec/.test(String(t))) {
            console.log(chalk.dim(`[codex] ${t}`));
            return;
          }
          if (VERBOSE && t === 'bridge.codex_raw' && typeof obj.line === 'string') {
            console.log(chalk.dim(`[codex-raw] ${obj.line}`));
            return;
          }
        } catch { /* ignore non-json */ }
      }
    });
    ws.on("close", () => { if (!closed) setTimeout(connect, 1200); });
    ws.on("error", () => { try { ws.close(); } catch { }; if (!closed) setTimeout(connect, 1500); });
  };
  connect();
  // Return a stopper in case we want to end later (not used for dev)
  return () => { closed = true; };
}

function startBridgeProcess(repoRoot: string) {
  if (VERBOSE) console.log(chalk.dim("Starting local bridge (cargo run -p codex-bridge)…"));
  const child = spawn("cargo", ["run", "-p", "codex-bridge", "--", "--bind", "0.0.0.0:8787"], {
    cwd: repoRoot,
    stdio: VERBOSE ? "inherit" : ["ignore", "ignore", "ignore"],
    env: {
      ...process.env,
      // Quiet verbose deps; keep our bridge at info
      RUST_LOG: process.env.RUST_LOG || (VERBOSE ? "info,convex=warn,convex::base_client=warn,tungstenite=warn,notify=warn" : "warn"),
      // Disable FS→Convex sync to avoid large startup imports/noise in dev
      OPENAGENTS_CONVEX_SYNC: process.env.OPENAGENTS_CONVEX_SYNC || "0",
      BRIDGE_DEBUG_WS: process.env.BRIDGE_DEBUG_WS || (VERBOSE ? "1" : "0"),
      BRIDGE_DEBUG_CODEX: process.env.BRIDGE_DEBUG_CODEX || (VERBOSE ? "1" : "0"),
    },
  });
  child.on("error", () => { });
}

function restartBridgeProcess(repoRoot: string) {
  // Try graceful: send a QUIT via lsof->pid
  try {
    const out = spawnSync(process.platform === 'darwin' || process.platform === 'linux' ? 'lsof' : 'netstat',
      process.platform === 'darwin' || process.platform === 'linux'
        ? ['-i', ':8787', '-sTCP:LISTEN', '-t']
        : [], { encoding: 'utf8' })
    const pids = String(out.stdout || '').split(/\s+/).filter(Boolean)
    for (const pid of pids) {
      try { process.kill(Number(pid), 'SIGTERM') } catch { }
    }
  } catch { }
  setTimeout(() => startBridgeProcess(repoRoot), 400);
}

async function probeBridgeEchoOnce(timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const ws = new WebSocket("ws://127.0.0.1:8787/ws");
    let done = false;
    const timer = setTimeout(() => { if (!done) { done = true; try { ws.close(); } catch { }; resolve(false); } }, timeoutMs);
    ws.on("open", () => {
      try { ws.send(JSON.stringify({ control: 'echo', tag: 'probe', payload: 'ok' })) } catch { }
    });
    ws.on("message", (data) => {
      const s = String(data || '').trim();
      if (!s.startsWith('{')) return;
      try {
        const obj = JSON.parse(s);
        if (obj?.type === 'bridge.echo' && (obj?.tag === 'probe')) { if (!done) { done = true; clearTimeout(timer); try { ws.close(); } catch { }; resolve(true); } }
      } catch { }
    });
    ws.on("error", () => { if (!done) { done = true; clearTimeout(timer); resolve(false); } });
    ws.on("close", () => { if (!done) { done = true; clearTimeout(timer); resolve(false); } });
  });
}

main();

function destructiveReset(): Promise<void> {
  const home = os.homedir();
  const paths: Array<{ path: string; desc: string }> = [
    { path: join(home, ".openagents", "openagents"), desc: "OpenAgents repo clone" },
    { path: join(home, ".openagents", "bin", process.platform === 'win32' ? 'local_backend.exe' : 'local_backend'), desc: "Convex local backend binary" },
    { path: join(home, ".openagents", "convex"), desc: "Convex local data + storage" },
  ];
  console.log(chalk.yellow("\nDanger: This will delete local OpenAgents clones, the Convex local backend binary, and local Convex data."));
  // First, attempt to stop listeners on common ports so reruns are clean
  try { killListeners(8787) } catch {}
  try { killListeners(7788) } catch {}

  if (!ASSUME_YES) {
    return promptYesNoTTY("Proceed with full reset? [y/N] ").then((ans) => {
      if (!ans) { console.log("Aborted."); try { process.stdin.pause(); } catch {}; process.exit(0); return; }
      runDelete(paths);
    });
  } else {
    runDelete(paths);
    return Promise.resolve();
  }
}

function promptYesNoTTY(question: string): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    const rl = tryOpenTTYReadline();
    if (rl) {
      const anyRl = rl as any;
      rl.question(question, (ans: string) => {
        try { rl.close(); } catch {}
        try { anyRl.__ttyIn?.destroy?.(); anyRl.__ttyOut?.end?.(); anyRl.__ttyOut?.destroy?.(); } catch {}
        resolve(/^y(es)?$/i.test(String(ans || '').trim()));
      });
      return;
    }
    // Hard fallback: use stdio even if not a TTY, and keep the process alive
    try { process.stdout.write(question); } catch {}
    try { process.stdin.setEncoding('utf8'); } catch {}
    try { (process.stdin as any).resume?.(); } catch {}
    const onData = (buf: Buffer | string) => {
      const s = String(buf || '').trim();
      try { process.stdin.pause(); } catch {}
      resolve(/^y(es)?$/i.test(s));
    };
    process.stdin.once('data', onData);
  });
}

function tryOpenTTYReadline(): any | null {
  try {
    const fs = require('node:fs');
    const readline = require('node:readline');
    if (process.platform !== 'win32') {
      try {
        const ttyIn = fs.createReadStream('/dev/tty');
        const ttyOut = fs.createWriteStream('/dev/tty');
        const rl = readline.createInterface({ input: ttyIn, output: ttyOut });
        (rl as any).__ttyIn = ttyIn; (rl as any).__ttyOut = ttyOut;
        return rl;
      } catch { /* ignore */ }
    }
    if (process.stdin && process.stdout) {
      return readline.createInterface({ input: process.stdin, output: process.stdout });
    }
    return null;
  } catch {
    return null;
  }
}

function runDelete(paths: Array<{ path: string; desc: string }>) {
  for (const p of paths) {
    try {
      if (!existsSync(p.path)) { if (VERBOSE) console.log(chalk.dim(`[delete] skip (missing): ${p.path}`)); continue; }
      const st = statSync(p.path);
      if (st.isDirectory()) {
        rmrf(p.path);
      } else {
        try { require('node:fs').unlinkSync(p.path); } catch {}
      }
      console.log(chalk.green(`✔ Deleted ${p.desc}: ${p.path}`));
    } catch (e: any) {
      console.log(chalk.yellow(`⚠ Failed to delete ${p.path}: ${e?.message || e}`));
    }
  }
  console.log(chalk.greenBright("Done. You can re-run `npx tricoder` for a fresh setup."));
  try { process.stdin.pause(); } catch {}
  process.exit(0);
}

function killListeners(port: number) {
  try {
    if (process.platform === 'darwin' || process.platform === 'linux') {
      const out = spawnSync('lsof', ['-i', `:${port}`, '-sTCP:LISTEN', '-t'], { encoding: 'utf8' });
      const pids = String(out.stdout || '').split(/\s+/).filter(Boolean);
      for (const pid of pids) {
        try { process.kill(Number(pid), 'SIGTERM'); if (VERBOSE) console.log(chalk.dim(`[delete] killed pid ${pid} on :${port}`)) } catch {}
      }
    }
  } catch {}
}

function rmrf(target: string) {
  try {
    const fs = require('node:fs');
    const path = require('node:path');
    const entries = fs.readdirSync(target, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(target, entry.name);
      if (entry.isDirectory()) rmrf(full); else { try { fs.unlinkSync(full) } catch {} }
    }
    try { fs.rmdirSync(target) } catch {}
  } catch {}
}

function readBridgeToken(): string | null {
  try {
    const envTok = String(process.env.OPENAGENTS_BRIDGE_TOKEN || '').trim();
    if (envTok) return envTok;
  } catch {}
  try {
    const home = os.homedir();
    const p = join(home, '.openagents', 'bridge.json');
    if (!fs.existsSync(p)) return null;
    const raw = fs.readFileSync(p, 'utf8');
    const obj = JSON.parse(raw);
    const t = obj?.token;
    if (typeof t === 'string' && t.length > 0) return t;
  } catch {}
  return null;
}
// Render a very compact QR using Unicode braille (2x4 dots per cell)
function printBrailleQR(text: string) {
  const qr: any = (QR as any).create(String(text || ''), { errorCorrectionLevel: 'L' });
  const mods = qr.modules || {};
  const size: number = mods.size || (Array.isArray(mods.data) ? (Array.isArray(mods.data[0]) ? mods.data.length : Math.sqrt(mods.data.length)) : 0);
  if (!size) throw new Error('qr size unknown');
  const at = (x: number, y: number): boolean => {
    if (x < 0 || y < 0 || x >= size || y >= size) return false;
    if (Array.isArray(mods.data?.[0])) return !!mods.data[y][x];
    return !!mods.data[y * size + x];
  };
  const margin = 1;
  let out = '';
  for (let y = -margin; y < size + margin; y += 4) {
    for (let x = -margin; x < size + margin; x += 2) {
      let bits = 0;
      if (at(x + 0, y + 0)) bits |= 1 << 0; // 1
      if (at(x + 0, y + 1)) bits |= 1 << 1; // 2
      if (at(x + 0, y + 2)) bits |= 1 << 2; // 3
      if (at(x + 0, y + 3)) bits |= 1 << 6; // 7
      if (at(x + 1, y + 0)) bits |= 1 << 3; // 4
      if (at(x + 1, y + 1)) bits |= 1 << 4; // 5
      if (at(x + 1, y + 2)) bits |= 1 << 5; // 6
      if (at(x + 1, y + 3)) bits |= 1 << 7; // 8
      out += bits ? String.fromCodePoint(0x2800 + bits) : ' ';
    }
    out += '\n';
  }
  console.log(out.trimEnd());
}
