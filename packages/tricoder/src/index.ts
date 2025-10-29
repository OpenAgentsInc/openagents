#!/usr/bin/env node
import chalk from 'chalk';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import os from 'node:os';
import fs from 'node:fs';
import path from 'node:path';

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

function runCargoBridge(repoDir: string): Promise<number> {
  return new Promise((resolve) => {
    const cmd = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
    const child = spawn(cmd, ['run', '-p', 'codex-bridge', '--', '--bind', '0.0.0.0:8787'], {
      cwd: repoDir,
      stdio: 'inherit',
    });
    child.on('exit', (code) => resolve(code ?? 0));
  });
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const autorun = args.has('--run-bridge') || args.has('-r');
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

  // Ensure repo and Rust, then run the bridge
  const repoDir = await ensureRepo(process.env.OPENAGENTS_REPO_DIR);
  const hasRust = await ensureRust();
  if (!hasRust) {
    console.log('\nAfter installing Rust, rerun: tricoder --run-bridge');
    process.exit(1);
  }
  console.log(chalk.cyan('Starting bridge via cargo (bind 0.0.0.0:8787)…'));
  const code = await runCargoBridge(repoDir);
  process.exit(code);
}

main().catch((e) => {
  console.error(chalk.red('Unexpected error:'));
  if (e instanceof Error) console.error(chalk.dim(e.stack || e.message));
  process.exit(1);
});
