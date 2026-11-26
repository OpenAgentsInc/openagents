// Firm reason: 6 (offline @workos/oagen TypeScript CLI driving coaxEmitter; checked-in Coax output requires zero Node in normal builds).
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateFiles, parseSpec, registerEmitter } from '@workos/oagen';
import * as policy from '@workos/openapi-spec/policy';
import { createCoaxEmitter } from './emitter.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '../../..');
const defaultOut = join(repoRoot, 'coax/lib/workos/gen');

const args = process.argv.slice(2);
const checkOnly = args.includes('--check');
const outIdx = args.indexOf('--out');
const outDir = outIdx >= 0 && args[outIdx + 1] ? resolve(args[outIdx + 1]) : defaultOut;

const req = createRequire(import.meta.url);
const specPath = req.resolve('@workos/openapi-spec/spec');
const specPkg = JSON.parse(readFileSync(req.resolve('@workos/openapi-spec/package.json'), 'utf8')) as {
  version: string;
};
const specBytes = readFileSync(specPath);
const specSha256 = createHash('sha256').update(specBytes).digest('hex');

// Upstream commit SHA pinned in workos/openapi-spec/.last-synced-sha for version 0.160.0.
const upstreamSha = '7641b3077aa34b0bcd700de3a588e0f740cbc96b';

const spec = await parseSpec(specPath, {
  operationIdTransform: policy.nestjsOperationIdTransform,
  schemaNameTransform: policy.schemaNameTransform,
  operationHints: policy.operationHints,
  transformSpec: policy.transformSpec,
});

const emitter = createCoaxEmitter({
  specVersion: specPkg.version,
  specSha256,
  upstreamSha,
});
registerEmitter(emitter);

const { files: generated } = generateFiles(spec, emitter, {
  namespace: 'WorkOS',
  outputDir: outDir,
  operationHints: policy.operationHints,
  mountRules: policy.mountRules,
});

if (checkOnly) {
  let drift = 0;
  const expected = new Set(generated.map((f) => f.path));
  for (const f of generated) {
    const dest = join(outDir, f.path);
    if (!existsSync(dest)) {
      console.error(`oagen-coax --check: missing ${f.path}`);
      drift++;
      continue;
    }
    const actual = readFileSync(dest, 'utf8');
    if (actual !== f.content) {
      console.error(`oagen-coax --check: drifted ${f.path}`);
      drift++;
    }
  }
  if (existsSync(outDir)) {
    for (const entry of readdirSync(outDir)) {
      if (!expected.has(entry)) {
        console.error(`oagen-coax --check: unexpected stale file ${entry}`);
        drift++;
      }
    }
  }
  if (drift > 0) {
    console.error(`oagen-coax --check: FAILED (${drift} file(s) out of sync)`);
    process.exit(1);
  }
  console.log(`oagen-coax --check: OK (${generated.length} files match @workos/openapi-spec ${specPkg.version})`);
  process.exit(0);
}

mkdirSync(outDir, { recursive: true });
const keep = new Set(generated.map((f) => f.path));
for (const entry of readdirSync(outDir)) {
  if (!keep.has(entry)) rmSync(join(outDir, entry), { recursive: true, force: true });
}
for (const f of generated) {
  const dest = join(outDir, f.path);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, f.content, 'utf8');
}
console.log(`oagen-coax: generated ${generated.length} files into ${outDir} (spec ${specPkg.version}, sha256=${specSha256.slice(0, 12)})`);
