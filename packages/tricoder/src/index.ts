#!/usr/bin/env node
import chalk from 'chalk';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import os from 'node:os';
import fs from 'node:fs';
import path from 'node:path';
import { ensureBridgeBinary } from './utils/bridgeBinary.js';

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

function spawnP(cmd: string, args: string[], opts: any = {}): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: 'inherit', ...opts });
    child.on('error', reject);
    child.on('exit', (code) => {
      if (code === 0) resolve(); else reject(new Error(`${cmd} exited with code ${code}`));
    });
  });
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
    const child = spawn(cmd, ['run', '-p', 'oa-bridge', '--', '--bind', '0.0.0.0:8787'], {
      cwd: repoDir,
      stdio: 'inherit',
      env,
    });
    child.on('exit', (code) => resolve(code ?? 0));
  });
}

function runBridgeBinary(binPath: string, env: NodeJS.ProcessEnv, extraArgs: string[] = []): Promise<number> {
  return new Promise((resolve) => {
    const args = ['--bind', process.env.TRICODER_BRIDGE_BIND || '0.0.0.0:8787', ...extraArgs];
    const child = spawn(binPath, args, { stdio: 'inherit', env });
    child.on('exit', (code) => resolve(code ?? 0));
  });
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const autorun = args.has('--run-bridge') || args.has('-r');
  const verbose = args.has('--verbose') || process.env.DEBUG === '1';
  const tsPath = await findTailscale();
  if (!tsPath) {
    console.log(chalk.red('Tailscale CLI not found.'));
    if (process.platform === 'darwin') console.log(chalk.yellow('Install: brew install tailscale'));
    else if (process.platform === 'win32') console.log(chalk.yellow('Install: winget install Tailscale.Tailscale'));
    else console.log(chalk.yellow('Install: curl -fsSL https://tailscale.com/install.sh | sh'));
    process.exit(1);
  }
  const status = await getTailscaleStatus(tsPath);
  if (!status) {
    console.log(chalk.red('tailscale status failed. Are you logged in?'));
    console.log(chalk.yellow('Run: tailscale up'));
    process.exit(1);
  }
  const selfIPv4 = chooseSelfIPv4(status);
  if (!selfIPv4) {
    console.log(chalk.red('No Tailscale 100.x IPv4 found for this device.'));
    console.log(chalk.yellow('Ensure Tailscale is connected on this desktop and try again.'));
    process.exit(1);
  }

  console.log(chalk.green(`Desktop IP (Tailscale): ${selfIPv4}`));
  const bridgePort = Number(process.env.TRICODER_BRIDGE_PORT || 8787);
  console.log(chalk.bold(`In the mobile app, set Desktop IP to ${selfIPv4} and port ${bridgePort}.`));

  if (!autorun) {
    console.log('\nTo launch the desktop bridge automatically:');
    console.log(chalk.cyan('  tricoder --run-bridge'));
    return;
  }

  // Prepare environment for the bridge process (Convex fast-start defaults)
  const exposeLan = process.env.TRICODER_EXPOSE_LAN === '1';
  const bridgeEnv: NodeJS.ProcessEnv = {
    ...process.env,
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
  console.log(chalk.cyan('Starting bridge via cargo (bind 0.0.0.0:8787)…'));
  code = await runCargoBridge(repoDir, bridgeEnv);
  process.exit(code);
}

main().catch((e) => {
  console.error(chalk.red('Unexpected error:'));
  if (e instanceof Error) console.error(chalk.dim(e.stack || e.message));
  process.exit(1);
});
