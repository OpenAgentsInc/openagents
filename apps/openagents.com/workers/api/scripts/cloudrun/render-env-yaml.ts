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
  // #8594: Sarah path mount on this monolith (assets copied into the image).
  SARAH_UI_DIR: '/app/sarah-ui',
  SARAH_AGENT_DIR: '/app/sarah-agent',
  SARAH_PUBLIC_BASE_URL:
    target === 'production'
      ? 'https://openagents.com/sarah'
      : 'https://openagents.com/sarah',
  // D1-over-HTTP bridge coordinates for the not-yet-migrated CFG-4 domains
  // (the token itself is a Secret Manager secret; these two are public ids).
  ...(target === 'production'
    ? {
        CLOUDFLARE_ACCOUNT_ID: '54fac8b750a29fdda9f2fa0f0afaed90',
        CLOUDFLARE_D1_DATABASE_ID: '9644ea09-f682-4971-98de-e0c791cb67fb',
        // OB-1 (#8558): armed live CRM/Sarah sending on PROD ONLY (owner GO).
        // CRM_RESEND_SEND_ENABLED flips the send gate; the CRM-specific sender
        // identity keeps Sarah's outbound distinct from the shared Sites
        // transactional RESEND_FROM_EMAIL (OpenAgents <chris+sites@…>), which
        // stays untouched. Durable across redeploys — do not rely on
        // live-revision arming. STAGING deliberately omits these (no live CRM
        // sends off staging). The warm-up cap machinery in the send path stays
        // enforced; this only opens the gate.
        CRM_RESEND_SEND_ENABLED: '1',
        CRM_RESEND_FROM_EMAIL: 'Sarah <sarah@openagents.com>',
        CRM_RESEND_REPLY_TO_EMAIL: 'sarah@openagents.com',
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
    ? {
        KHALA_BACKING_MODEL: 'deepseek-v4-flash',
        // AC-1 (#8503) Seam A: ARM the cloud-gcp microVM lane on STAGING ONLY.
        // `live` flips isCloudGceProvisioningArmed(...)=true so the admin
        // dispatch trigger (/api/admin/khala/cloud/runtime-dispatch) actually
        // mints a token + POSTs a placement instead of failing closed 409.
        // PROD deliberately omits this (fail-closed default-off).
        OA_CODEX_GCE_PROVISIONER: 'live',
        // The microVM's ONE /v1/chat/completions turn must hit the SAME
        // monolith that minted the short-lived owner-linked execution token
        // (same khala_sync DB + matching no-meter secret) — the staging
        // Cloud Run URL, not the prod app origin.
        OA_CLOUD_RUNTIME_INFERENCE_BASE_URL:
          'https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app',
      }
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
