#!/usr/bin/env node
import chalk from 'chalk';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import os from 'node:os';
import fs from 'node:fs';
import path from 'node:path';
import { ensureBridgeBinary } from './utils/bridgeBinary.js';
import { webcrypto as nodeCrypto, randomBytes } from 'node:crypto';
import qrcode from 'qrcode-terminal';
import { findAvailablePort } from './ports.js';

const execFileP = promisify(execFile);

type TSStatus = any;

const TAILSCALE_PATHS = [
  'tailscale',
  '/Applications/Tailscale.app/Contents/MacOS/tailscale',
  'C\\\\Program Files (x86)\\\\Tailscale IPN\\\\tailscale.exe',
];

async function which(cmd: string): Promise<string | null> {
  const bin = process.platform === 'win32' ? 'where' : 'which';
  try {
    const { stdout } = await execFileP(bin, [cmd]);
    const p = String(stdout || '').split(/\r?\n/).find(Boolean)?.trim();
    return p || null;
  } catch {
    return null;
  }
}

async function findTailscale(): Promise<string | null> {
  for (const p of TAILSCALE_PATHS) {
    try {
      const { stdout } = await execFileP(p, ['version'], { windowsHide: true });
      if (stdout.toLowerCase().includes('tailscale')) return p;
    } catch {}
  }
  return null;
}

async function getTailscaleStatus(tsPath: string): Promise<TSStatus | null> {
  try {
    const { stdout } = await execFileP(tsPath, ['status', '--json'], { maxBuffer: 5 * 1024 * 1024 });
    return JSON.parse(stdout || '{}');
  } catch {
    return null;
  }
}

function chooseSelfIPv4(status: TSStatus): string | null {
  const self = status?.Self || status?.SelfNode || status?.SelfStatus || null;
  const ips: string[] = Array.isArray(self?.TailscaleIPs) ? self.TailscaleIPs : [];
  const v4 = ips.find((ip: string) => /^100\./.test(ip)) || ips.find((ip: string) => ip.includes('.')) || null;
  return v4 || null;
}

function isTailscaleActive(status: TSStatus): boolean {
  try {
    const s = String(status?.BackendState || status?.backendState || '').toLowerCase();
    // Only treat as active when the backend is running (connected to tailnet)
    return s === 'running';
  } catch { return false }
}

// Detect private IPv4 range
const PRIVATE_V4 = /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[0-1])\.)/;
function getLanIPv4Candidates(): string[] {
  const nets = os.networkInterfaces();
  const out: string[] = [];
  for (const name of Object.keys(nets)) {
    const addrs = nets[name] || [];
    for (const a of addrs) {
      // Node types are a bit loose across versions; use runtime checks
      const fam = (a as any).family || (a as any).addressFamily;
      const isV4 = fam === 4 || fam === 'IPv4';
      const addr = String((a as any).address || '');
      const internal = Boolean((a as any).internal);
      if (isV4 && !internal && PRIVATE_V4.test(addr)) out.push(addr);
    }
  }
  // Ensure stable selection order
  return Array.from(new Set(out)).sort();
}

function chooseLanIPv4(): string | null {
  const c = getLanIPv4Candidates();
  return c[0] || null;
}

function spawnP(cmd: string, args: string[], opts: any = {}): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: 'inherit', ...opts });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) resolve(); else reject(new Error(`${cmd} exited with code ${code}`));
    });
  });
}

function randToken(len = 48): string {
  try {
    const bytes = new Uint8Array(len);
    nodeCrypto.getRandomValues(bytes);
    const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_';
    let out = '';
    for (let i = 0; i < bytes.length; i++) out += chars[bytes[i] % chars.length];
    return out;
  } catch {
    return randomBytes(len).toString('base64url').replace(/=+$/g, '').slice(0, len);
  }
}

function b64url(s: string): string {
  return Buffer.from(s, 'utf8').toString('base64').replace(/=+$/g, '').replace(/\+/g, '-').replace(/\//g, '_');
}

function buildBridgeCode(host: string, port: number, token: string, secure = false, hosts?: string[]) {
  const scheme = secure ? 'wss' : 'ws';
  const bridge = `${scheme}://${host}:${port}/ws`;
  const payload: any = { v: 1, type: 'bridge', provider: 'openagents', bridge, token };
  if (Array.isArray(hosts) && hosts.length > 0) {
    payload.hosts = hosts;
  }
  const code = b64url(JSON.stringify(payload));
  const deeplink = `openagents://connect?j=${code}`;
  return { payload, code, deeplink, bridge } as const;
}

function openagentsHome(): string {
  const base = process.env.OPENAGENTS_HOME || path.join(os.homedir(), '.openagents');
  return base;
}

function detectClaudeBin(): string | null {
  // Prefer Claude Code’s canonical local install if present
  const local = path.join(os.homedir(), '.claude', 'local', process.platform === 'win32' ? 'claude.exe' : 'claude');
  try { if (fs.existsSync(local)) return local; } catch {}
  return null;
}

function readPersistedToken(): string | null {
  try {
    const p = path.join(openagentsHome(), 'bridge.json');
    if (!fs.existsSync(p)) return null;
    const raw = JSON.parse(fs.readFileSync(p, 'utf8')) as { token?: string };
    const t = String(raw?.token || '').trim();
    return t || null;
  } catch { return null; }
}

function writePersistedToken(token: string): boolean {
  try {
    const base = openagentsHome();
    fs.mkdirSync(base, { recursive: true });
    const p = path.join(base, 'bridge.json');
    fs.writeFileSync(p, JSON.stringify({ token }, null, 2));
    return true;
  } catch { return false; }
}

async function ensureRepo(targetDir?: string): Promise<string> {
  const dest = targetDir || path.join(os.homedir(), 'code', 'openagents');
  try { fs.mkdirSync(path.dirname(dest), { recursive: true }); } catch {}
  if (fs.existsSync(path.join(dest, 'Cargo.toml'))) return dest;
  console.log(chalk.cyan(`Cloning OpenAgents repo -> ${dest}`));
  try {
    await spawnP('git', ['clone', 'https://github.com/OpenAgentsInc/openagents.git', dest]);
  } catch (e) {
    console.error(chalk.red('git clone failed. Please install git and try again.'));
    throw e;
  }
  return dest;
}

async function ensureRust(): Promise<boolean> {
  const cargo = await which('cargo');
  if (cargo) return true;
  console.error(chalk.red('Rust toolchain (cargo) not found.'));
  if (process.platform === 'darwin' || process.platform === 'linux') {
    console.error(chalk.yellow('Install with: curl https://sh.rustup.rs -sSf | sh -s -- -y'));
  } else if (process.platform === 'win32') {
    console.error(chalk.yellow('Install Rust from: https://rustup.rs/'));
  }
  return false;
}

function runCargoBridge(repoDir: string, env: NodeJS.ProcessEnv): Promise<number> {
  return new Promise((resolve) => {
    const cmd = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
    // Respect TRICODER_BRIDGE_BIND if provided; otherwise default to port/env
    const port = Number(process.env.TRICODER_BRIDGE_PORT || 8787);
    const bind = process.env.TRICODER_BRIDGE_BIND || `0.0.0.0:${port}`;
    const child = spawn(cmd, ['run', '-p', 'oa-bridge', '--', '--bind', bind], {
      cwd: repoDir,
      stdio: 'inherit',
      env,
    });
    child.on('exit', (code) => resolve(code ?? 0));
  });
}

function runBridgeBinary(binPath: string, env: NodeJS.ProcessEnv, extraArgs: string[] = []): Promise<number> {
  return new Promise((resolve) => {
    const port = Number(process.env.TRICODER_BRIDGE_PORT || 8787);
    const args = ['--bind', process.env.TRICODER_BRIDGE_BIND || `0.0.0.0:${port}`, ...extraArgs];
    const child = spawn(binPath, args, { stdio: 'inherit', env });
    child.on('exit', (code) => resolve(code ?? 0));
  });
}

async function main() {
  const args = new Set(process.argv.slice(2));
  // Default: run the bridge. Allow opting out with --no-run
  const autorun = !args.has('--no-run');
  const verbose = args.has('--verbose') || process.env.DEBUG === '1';
  const prefer = (process.env.TRICODER_PREFER || '').toLowerCase(); // 'tailscale' | 'lan'
  const rotateToken = args.has('--rotate-token') || args.has('-R') || process.env.TRICODER_ROTATE_TOKEN === '1';

  // Determine IP to advertise: prefer tailscale when available unless TRICODER_PREFER=lan
  let hostIp: string | null = null;
  let mode: 'tailscale' | 'lan' = 'lan';
  const tsPath = await findTailscale();
  let tsStatus: TSStatus | null = null;
  if (prefer !== 'lan' && tsPath) {
    tsStatus = await getTailscaleStatus(tsPath);
    const selfIPv4 = tsStatus && isTailscaleActive(tsStatus) ? chooseSelfIPv4(tsStatus) : null;
    if (selfIPv4) { hostIp = selfIPv4; mode = 'tailscale'; }
  }
  if (!hostIp) {
    hostIp = chooseLanIPv4();
    mode = 'lan';
  }
  if (!hostIp) {
    // Last resort: advise user how to proceed
    console.log(chalk.red('Could not determine a LAN IPv4 address.'));
    console.log(chalk.yellow('Ensure you are connected to a network and try again.'));
    process.exit(1);
  }

  // Choose an available port, falling back to the next free if preferred port is busy
  const preferPort = Number(process.env.TRICODER_BRIDGE_PORT || 8787);
  const bridgePort = await findAvailablePort(preferPort, 50, '0.0.0.0');
  if (verbose) {
    if (bridgePort !== preferPort) console.log(chalk.yellow(`Port ${preferPort} busy; selected ${bridgePort}`));
    console.log(chalk.gray(`Bind: 0.0.0.0:${bridgePort}`));
  }
  // Persist chosen port/bind for downstream processes
  process.env.TRICODER_BRIDGE_PORT = String(bridgePort);
  process.env.TRICODER_BRIDGE_BIND = process.env.TRICODER_BRIDGE_BIND || `0.0.0.0:${bridgePort}`;

  // Durable token: reuse ~/.openagents/bridge.json when present
  let token: string | null = null;
  if (!rotateToken) token = readPersistedToken();
  const generated = !token;
  if (!token) {
    token = randToken(48);
    writePersistedToken(token);
    if (verbose) console.log(chalk.gray('Token: generated and persisted to ~/.openagents/bridge.json'));
  } else if (verbose) {
    console.log(chalk.gray('Token: using persisted token from ~/.openagents/bridge.json'));
  }

  // Compute candidate hosts for QR payload in priority order
  const lanCandidates = getLanIPv4Candidates().map((ip) => `${ip}:${bridgePort}`);
  let tsCandidates: string[] = [];
  if (prefer !== 'lan' && tsPath && tsStatus && isTailscaleActive(tsStatus)) {
    const ip = chooseSelfIPv4(tsStatus);
    tsCandidates = ip ? [`${ip}:${bridgePort}`] : [];
  }
  const hosts = (mode === 'lan') ? [...lanCandidates, ...tsCandidates] : [...tsCandidates, ...lanCandidates];
  const { deeplink, bridge } = buildBridgeCode(hostIp, bridgePort, token!, false, hosts);
  console.log(mode === 'tailscale'
    ? chalk.green(`Desktop IP (Tailscale): ${hostIp}`)
    : chalk.green(`Desktop IP (LAN): ${hostIp}`));
  console.log(chalk.bold('Scan this QR in the OpenAgents mobile app:'));
  try { qrcode.generate(deeplink, { small: true }); } catch {}
  console.log(chalk.gray('Deep link: '), chalk.white(deeplink));
  console.log(chalk.gray('WS URL:   '), chalk.white(bridge));
  console.log(chalk.gray('Token:    '), chalk.white(token!));
  if (hosts.length > 1) {
    console.log(chalk.gray('Hosts:    '), chalk.white(hosts.join(', ')));
  }

  // If not autorun, we still show the QR/deeplink and exit
  if (!autorun) { return; }

  // Prepare environment for the bridge process (Convex fast-start defaults)
  const exposeLan = process.env.TRICODER_EXPOSE_LAN === '1';
  const bridgeEnv: NodeJS.ProcessEnv = {
    ...process.env,
    // Only set OPENAGENTS_BRIDGE_TOKEN when we generated a new one this run.
    // Otherwise, let the bridge read ~/.openagents/bridge.json so the app and
    // server stay in sync without forcing rescans.
    ...(generated ? { OPENAGENTS_BRIDGE_TOKEN: token! } : {}),
    OPENAGENTS_CONVEX_STATE: process.env.OPENAGENTS_CONVEX_STATE || 'convex',
    OPENAGENTS_CONVEX_INTERFACE: process.env.OPENAGENTS_CONVEX_INTERFACE || (exposeLan ? '0.0.0.0' : '127.0.0.1'),
    OPENAGENTS_CONVEX_INSTANCE: process.env.OPENAGENTS_CONVEX_INSTANCE || 'openagents',
    OPENAGENTS_CONVEX_PREFER_CACHE: process.env.OPENAGENTS_CONVEX_PREFER_CACHE || '1',
    OPENAGENTS_CONVEX_DEBUG: verbose ? '1' : (process.env.OPENAGENTS_CONVEX_DEBUG || ''),
  };
  // If a local Claude binary exists, prefer it explicitly so shell aliases don't interfere
  try {
    if (!bridgeEnv.CLAUDE_BIN) {
      const claudeBin = detectClaudeBin();
      if (claudeBin) {
        bridgeEnv.CLAUDE_BIN = claudeBin;
        if (verbose) console.log(chalk.cyan(`Claude CLI: ${claudeBin}`));
      }
    }
  } catch {}

  // Prefer a prebuilt oa-bridge binary (downloaded/cached) and fall back to cargo if needed
  const preferBinary = process.env.TRICODER_PREFER_BIN !== '0';
  let code = 0;
  if (preferBinary) {
    try {
      const { binaryPath, source, version } = await ensureBridgeBinary() as any;
      // Minimum bridge version that includes recent fixes
      const MIN_BRIDGE = process.env.TRICODER_MIN_BRIDGE || 'v0.2.2';
      const isTag = typeof version === 'string' && /^v?\d+\.\d+\.\d+/.test(version);
      const olderThanMin = isTag && compareSemver(version, MIN_BRIDGE) < 0;
      if (olderThanMin) {
        console.warn(chalk.yellow(`Prebuilt bridge ${version} is older than required ${MIN_BRIDGE}; falling back to cargo build of latest.`));
      } else {
        if (verbose) console.log(chalk.gray(`Bridge binary: ${binaryPath}`));
        console.log(chalk.cyan(`Starting bridge (${source}${isTag ? ` ${version}` : ''})…`));
        code = await runBridgeBinary(binaryPath, bridgeEnv);
        process.exit(code);
        return;
      }
    } catch (e) {
      console.warn(chalk.yellow('Falling back to cargo: could not obtain prebuilt oa-bridge binary.'));
      if (e instanceof Error) console.warn(chalk.dim(e.message));
    }
  }

  // Cargo fallback path
  const repoDir = await ensureRepo(process.env.OPENAGENTS_REPO_DIR);
  const hasRust = await ensureRust();
  if (!hasRust) {
    console.log('\nAfter installing Rust, rerun: tricoder');
    process.exit(1);
  }
  const bind = process.env.TRICODER_BRIDGE_BIND || `0.0.0.0:${bridgePort}`;
  if (verbose) console.log(chalk.gray(`Repo: ${repoDir}`));
  console.log(chalk.cyan(`Starting bridge via cargo (bind ${bind})…`));
  code = await runCargoBridge(repoDir, bridgeEnv);
  process.exit(code);
}

function compareSemver(a: string, b: string): number {
  const parse = (s: string) => (s.replace(/^v/i, '').split('.') as string[]).map((x) => parseInt(x, 10) || 0);
  const [a1, a2, a3] = parse(a);
  const [b1, b2, b3] = parse(b);
  if (a1 !== b1) return a1 - b1;
  if (a2 !== b2) return a2 - b2;
  return a3 - b3;
}

main().catch((e) => {
  console.error(chalk.red('Unexpected error:'));
  if (e instanceof Error) console.error(chalk.dim(e.stack || e.message));
  process.exit(1);
});
