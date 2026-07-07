#!/usr/bin/env bun
/**
 * CFG-9 (#8524): render the Cloud Run --env-vars-file YAML for the monolith
 * from wrangler.jsonc, so the committed wrangler `vars` blocks stay the
 * single source of truth for non-secret configuration.
 *
 * Usage: bun scripts/cloudrun/render-env-yaml.ts (production|staging) [out]
 *
 * Adds the monolith-specific overrides on top (LiveHub URL, cron/env
 * markers); SECRETS ARE NEVER RENDERED HERE — they ride --set-secrets in
 * scripts/deploy-cloudrun.sh.
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import path from 'node:path'

const stripJsonComments = (input: string): string => {
  let out = ''
  let inString = false
  let inLine = false
  let inBlock = false
  for (let i = 0; i < input.length; i++) {
    const ch = input[i]!
    const next = input[i + 1]
    if (inLine) {
      if (ch === '\n') {
        inLine = false
        out += ch
      }
      continue
    }
    if (inBlock) {
      if (ch === '*' && next === '/') {
        inBlock = false
        i++
      }
      continue
    }
    if (inString) {
      out += ch
      if (ch === '\\') {
        out += next ?? ''
        i++
      } else if (ch === '"') {
        inString = false
      }
      continue
    }
    if (ch === '"') {
      inString = true
      out += ch
      continue
    }
    if (ch === '/' && next === '/') {
      inLine = true
      continue
    }
    if (ch === '/' && next === '*') {
      inBlock = true
      i++
      continue
    }
    out += ch
  }
  // Trailing commas.
  return out.replace(/,\s*([}\]])/g, '$1')
}

const target = process.argv[2]
if (target !== 'production' && target !== 'staging') {
  console.error('usage: render-env-yaml.ts (production|staging) [outfile]')
  process.exit(2)
}

const apiDir = path.resolve(import.meta.dir, '..', '..')
const wrangler = JSON.parse(
  stripJsonComments(readFileSync(path.join(apiDir, 'wrangler.jsonc'), 'utf8')),
) as {
  vars?: Record<string, string>
  env?: { staging?: { vars?: Record<string, string> } }
}

const vars: Record<string, string> =
  target === 'production'
    ? { ...(wrangler.vars ?? {}) }
    : { ...(wrangler.env?.staging?.vars ?? {}) }

if (target === 'staging') {
  // The staging Worker never carried the Resend vars (email untested there),
  // but the monolith mounts RESEND_API_KEY — config validation then requires
  // the from-address too (config.ts resendConfig). Inherit prod's values.
  for (const key of ['RESEND_FROM_EMAIL', 'RESEND_REPLY_TO_EMAIL']) {
    if (vars[key] === undefined && wrangler.vars?.[key] !== undefined) {
      vars[key] = wrangler.vars[key]
    }
  }
}

// Monolith-specific overrides (CFG-9). Non-secret only.
const liveHubUrl =
  target === 'production'
    ? 'https://khala-live-hub-ezxz4mgdsq-uc.a.run.app'
    : 'https://khala-live-hub-staging-ezxz4mgdsq-uc.a.run.app'

Object.assign(vars, {
  KHALA_SYNC_LIVE_HUB_URL: liveHubUrl,
  OPENAGENTS_RUNTIME: 'cloudrun-monolith',
  // D1-over-HTTP bridge coordinates for the not-yet-migrated CFG-4 domains
  // (the token itself is a Secret Manager secret; these two are public ids).
  ...(target === 'production'
    ? {
        CLOUDFLARE_ACCOUNT_ID: '54fac8b750a29fdda9f2fa0f0afaed90',
        CLOUDFLARE_D1_DATABASE_ID: '9644ea09-f682-4971-98de-e0c791cb67fb',
      }
    : {}),
  // The CFG-10 LB (and staging smoke tooling) forwards the original host in
  // X-Forwarded-Host; the worker routes by hostname (issuer vs app).
  OPENAGENTS_TRUST_FORWARDED_HOST: '1',
  // Cloud coding sessions (the org cloud executor control plane) — the URL
  // is a public service address; the bearer rides Secret Manager.
  CLOUD_CODING_SESSIONS_ENABLED: 'true',
  OA_CLOUD_CONTROL_URL: 'https://oa-cloud-run-bridge-ezxz4mgdsq-uc.a.run.app',
  // AC-1 (#8503): staging serves Khala on the Fireworks lane so the org-cloud
  // microVM inference path has a working 200 supply. Prod keeps its
  // wrangler.jsonc KHALA_BACKING_MODEL (the Hydralisk GLM fleet); staging pins
  // deepseek-v4-flash here (paired with dropping the staging OpenRouter secret
  // in deploy-cloudrun.sh so the zero-balance OpenRouter primary can't shadow
  // it). Durable across redeploys — do not rely on live-revision arming.
  ...(target === 'staging'
    ? { KHALA_BACKING_MODEL: 'deepseek-v4-flash' }
    : {}),
})

// wrangler staging pins KHALA_SYNC_LIVE_HUB_URL too; the assign above wins
// deterministically with the same value.

const yaml = `${Object.entries(vars)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([key, value]) => `${key}: ${JSON.stringify(String(value))}`)
  .join('\n')}\n`

const outFile = process.argv[3]
  ? path.resolve(process.argv[3])
  : path.join(apiDir, 'dist-cloudrun', `env-${target}.yaml`)
mkdirSync(path.dirname(outFile), { recursive: true })
writeFileSync(outFile, yaml)
console.log(`wrote ${outFile} (${Object.keys(vars).length} vars)`)
