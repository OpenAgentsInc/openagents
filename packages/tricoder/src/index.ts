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

function buildBridgeCode(host: string, port: number, token: string, secure = false) {
  const scheme = secure ? 'wss' : 'ws';
  const bridge = `${scheme}://${host}:${port}/ws`;
  const payload = { v: 1, type: 'bridge', provider: 'openagents', bridge, token };
  const code = b64url(JSON.stringify(payload));
  const deeplink = `openagents://connect?j=${code}`;
  return { payload, code, deeplink, bridge } as const;
}

function openagentsHome(): string {
  const base = process.env.OPENAGENTS_HOME || path.join(os.homedir(), '.openagents');
  return base;
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
  const autorun = args.has('--run-bridge') || args.has('-r');
  const verbose = args.has('--verbose') || process.env.DEBUG === '1';
  const prefer = (process.env.TRICODER_PREFER || '').toLowerCase(); // 'tailscale' | 'lan'
  const rotateToken = args.has('--rotate-token') || args.has('-R') || process.env.TRICODER_ROTATE_TOKEN === '1';

  // Determine IP to advertise: prefer tailscale when available unless TRICODER_PREFER=lan
  let hostIp: string | null = null;
  let mode: 'tailscale' | 'lan' = 'lan';
  const tsPath = await findTailscale();
  if (prefer !== 'lan' && tsPath) {
    const status = await getTailscaleStatus(tsPath);
    const selfIPv4 = status ? chooseSelfIPv4(status) : null;
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

  const bridgePort = Number(process.env.TRICODER_BRIDGE_PORT || 8787);
  // Ensure bind uses selected port if TRICODER_BRIDGE_BIND not set
  if (!process.env.TRICODER_BRIDGE_BIND) {
    process.env.TRICODER_BRIDGE_BIND = `0.0.0.0:${bridgePort}`;
  }

  // Durable token: reuse ~/.openagents/bridge.json when present
  let token: string | null = null;
  if (!rotateToken) token = readPersistedToken();
  const generated = !token;
  if (!token) {
    token = randToken(48);
    writePersistedToken(token);
  }

  const { deeplink, bridge } = buildBridgeCode(hostIp, bridgePort, token!, false);
  console.log(mode === 'tailscale'
    ? chalk.green(`Desktop IP (Tailscale): ${hostIp}`)
    : chalk.green(`Desktop IP (LAN): ${hostIp}`));
  console.log(chalk.bold('Scan this QR in the OpenAgents mobile app:'));
  try { qrcode.generate(deeplink, { small: true }); } catch {}
  console.log(chalk.gray('Deep link: '), chalk.white(deeplink));
  console.log(chalk.gray('WS URL:   '), chalk.white(bridge));
  console.log(chalk.gray('Token:    '), chalk.white(token!));

  if (!autorun) {
    console.log('\nTo launch the desktop bridge automatically:');
    console.log(chalk.cyan('  tricoder --run-bridge'));
    return;
  }

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

  // Prefer a prebuilt oa-bridge binary (downloaded/cached) and fall back to cargo if needed
  const preferBinary = process.env.TRICODER_PREFER_BIN !== '0';
  let code = 0;
  if (preferBinary) {
    try {
      const { binaryPath, source } = await ensureBridgeBinary();
      console.log(chalk.cyan(`Starting bridge (${source})…`));
      code = await runBridgeBinary(binaryPath, bridgeEnv);
      process.exit(code);
      return;
    } catch (e) {
      console.warn(chalk.yellow('Falling back to cargo: could not obtain prebuilt oa-bridge binary.'));
      if (e instanceof Error) console.warn(chalk.dim(e.message));
    }
  }

  // Cargo fallback path
  const repoDir = await ensureRepo(process.env.OPENAGENTS_REPO_DIR);
  const hasRust = await ensureRust();
  if (!hasRust) {
    console.log('\nAfter installing Rust, rerun: tricoder --run-bridge');
    process.exit(1);
  }
  const bind = process.env.TRICODER_BRIDGE_BIND || `0.0.0.0:${bridgePort}`;
  console.log(chalk.cyan(`Starting bridge via cargo (bind ${bind})…`));
  code = await runCargoBridge(repoDir, bridgeEnv);
  process.exit(code);
}

main().catch((e) => {
  console.error(chalk.red('Unexpected error:'));
  if (e instanceof Error) console.error(chalk.dim(e.stack || e.message));
  process.exit(1);
});
