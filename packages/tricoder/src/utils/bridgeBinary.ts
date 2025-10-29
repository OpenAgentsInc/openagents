import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import AdmZip from 'adm-zip';

type EnsureResult = { version: string; binaryPath: string; source: 'cache' | 'download' | 'path' | 'home-bin' };

const OWNER = process.env.TRICODER_BRIDGE_OWNER || 'OpenAgentsInc';
const REPO = process.env.TRICODER_BRIDGE_REPO || 'openagents';

export async function ensureBridgeBinary(): Promise<EnsureResult> {
  // 1) PATH or home bin override
  const preferPath = process.env.TRICODER_USE_PATH_BRIDGE === '1';
  const exe = process.platform === 'win32' ? 'oa-bridge.exe' : 'oa-bridge';
  if (preferPath) {
    const found = await which(exe);
    if (found) return { version: 'path', binaryPath: found, source: 'path' };
  }
  const homeBin = path.join(os.homedir(), '.openagents', 'bin', exe);
  if (await exists(homeBin)) return { version: 'home', binaryPath: homeBin, source: 'home-bin' };

  // 2) Official cache (~/.cache/openagents or platform equiv)
  const cached = await findLatestCachedBinary();
  if (cached) return { version: cached.version, binaryPath: cached.path, source: 'cache' };

  // 3) Download from GitHub Releases
  const artifact = artifactNameForThisPlatform();
  const desiredVersion = process.env.TRICODER_BRIDGE_VERSION || null;
  const version = desiredVersion || (await findLatestWithAsset(artifact));
  const downloaded = await downloadAndInstall(version, artifact);
  await makeExecutable(downloaded);
  return { version, binaryPath: downloaded, source: 'download' };
}

function artifactNameForThisPlatform(): string {
  // Align with typical Rust target triples used for release assets
  if (process.platform === 'darwin') {
    return process.arch === 'arm64'
      ? 'oa-bridge-aarch64-apple-darwin.zip'
      : 'oa-bridge-x86_64-apple-darwin.zip';
  }
  if (process.platform === 'linux') {
    return process.arch === 'arm64'
      ? 'oa-bridge-aarch64-unknown-linux-gnu.zip'
      : 'oa-bridge-x86_64-unknown-linux-gnu.zip';
  }
  if (process.platform === 'win32') {
    return 'oa-bridge-x86_64-pc-windows-msvc.zip';
  }
  throw new Error(`Unsupported platform ${process.platform} ${process.arch}`);
}

async function findLatestCachedBinary(): Promise<{ version: string; path: string } | null> {
  const base = getOAStore();
  const dir = path.join(base, 'binaries');
  if (!(await exists(dir))) return null;
  const versions = (await fsp.readdir(dir)).filter(Boolean).sort().reverse();
  const exe = process.platform === 'win32' ? 'oa-bridge.exe' : 'oa-bridge';
  for (const v of versions) {
    const p = path.join(dir, v, exe);
    if (await exists(p)) {
      await makeExecutable(p);
      return { version: v, path: p };
    }
  }
  return null;
}

async function findLatestWithAsset(artifact: string): Promise<string> {
  const listUrl = `https://api.github.com/repos/${OWNER}/${REPO}/releases?per_page=30`;
  let url: string | null = listUrl;
  let latestStable: string | null = null;
  while (url) {
    const res = await fetch(url, { headers: { 'User-Agent': 'tricoder-bridge-fetcher' } });
    if (!res.ok) throw new Error(`GitHub API ${res.status}`);
    const releases: any[] = await res.json();
    for (const r of releases) {
      if (!latestStable && !r.draft && !r.prerelease) latestStable = r.tag_name;
      if (!r.draft && !r.prerelease) {
        if (Array.isArray(r.assets) && r.assets.some((a: any) => a?.name === artifact)) {
          return r.tag_name;
        }
      }
    }
    url = parseLink(res.headers.get('Link'))?.next || null;
  }
  if (!latestStable) throw new Error('No stable releases found for OpenAgents repo');
  throw new Error(`No release contained ${artifact}`);
}

function parseLink(h: string | null): { next?: string } | null {
  if (!h) return null;
  const out: Record<string, string> = {};
  for (const part of h.split(',')) {
    const [u, rel] = part.split(';');
    if (!u || !rel) continue;
    const url = u.trim().replace(/^<|>$/g, '');
    const r = rel.trim().replace(/^rel="|"$/g, '');
    out[r] = url;
  }
  return out;
}

async function downloadAndInstall(version: string, artifact: string): Promise<string> {
  const url = `https://github.com/${OWNER}/${REPO}/releases/download/${version}/${artifact}`;
  const res = await fetch(url);
  if (res.status !== 200) throw new Error(`Asset not found: ${url}`);
  const tmpRoot = await fsp.mkdtemp(path.join(os.tmpdir(), 'oa-bridge-'));
  const zipPath = path.join(tmpRoot, 'bridge.zip');
  await streamToFile(res, zipPath);
  const unzipDir = path.join(tmpRoot, 'unzipped');
  await fsp.mkdir(unzipDir, { recursive: true });
  const zip = new AdmZip(zipPath);
  zip.extractAllTo(unzipDir, true);
  const exe = process.platform === 'win32' ? 'oa-bridge.exe' : 'oa-bridge';
  const src = path.join(unzipDir, exe);
  const dest = getVersionedBinaryPath(version);
  await fsp.mkdir(path.dirname(dest), { recursive: true });
  await fsp.copyFile(src, dest);
  return dest;
}

async function streamToFile(res: Response, destPath: string): Promise<void> {
  const file = fs.createWriteStream(destPath);
  const body = res.body as any;
  await new Promise<void>((resolve, reject) => {
    body.pipe(file);
    body.on('error', reject);
    file.on('finish', resolve);
    file.on('error', reject);
  });
}

function getVersionedBinaryPath(version: string): string {
  const base = getOAStore();
  const exe = process.platform === 'win32' ? 'oa-bridge.exe' : 'oa-bridge';
  return path.join(base, 'binaries', version, exe);
}

function getOAStore(): string {
  if (process.platform === 'win32') {
    const lad = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    return path.join(lad, 'OpenAgents');
  }
  if (process.platform === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Caches', 'openagents');
  }
  const xdg = process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache');
  return path.join(xdg, 'openagents');
}

async function which(cmd: string): Promise<string | null> {
  const bin = process.platform === 'win32' ? 'where' : 'which';
  try {
    const { stdout } = await (await import('node:child_process')).execFileSync
      ? await import('node:child_process')
      : ({} as any);
  } catch {}
  try {
    const { execFile } = await import('node:child_process');
    const { promisify } = await import('node:util');
    const execFileP = promisify(execFile);
    const { stdout } = await execFileP(bin, [cmd]);
    const p = String(stdout || '').split(/\r?\n/).find(Boolean)?.trim();
    return p || null;
  } catch {
    return null;
  }
}

async function exists(p: string): Promise<boolean> {
  try { await fsp.access(p, fs.constants.F_OK); return true; } catch { return false; }
}

async function makeExecutable(p: string) {
  if (process.platform !== 'win32') {
    try { await fsp.chmod(p, 0o755); } catch {}
  }
}

