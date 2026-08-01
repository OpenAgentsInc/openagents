import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

import { shouldBundleCloudRunDependency } from '../../vite.config'
import { externalRuntimeSpecifiers } from './assert-self-contained-bundle.mjs'
import {
  LIVEKIT_ADMISSION_KEYS,
  admissionDrift,
  parseCommittedAdmissionState,
  resolveServingRevision,
} from './check-livekit-admission-drift.mjs'

describe('Cloud Run Vite Plus bundle contract', () => {
  test.each(['env-production.yaml', 'env-staging.yaml'])(
    '%s has unique top-level keys and exactly one enabled Desktop usage gate',
    fileName => {
      const contents = readFileSync(
        fileURLToPath(new URL(fileName, import.meta.url)),
        'utf8',
      )
      const entries = contents
        .split(/\r?\n/u)
        .filter(
          line => line.trim().length > 0 && !line.trimStart().startsWith('#'),
        )
        .map(line => {
          const match = /^([A-Z][A-Z0-9_]*):\s*(.*)$/u.exec(line)
          expect(
            match,
            `expected a flat top-level YAML mapping entry: ${line}`,
          ).not.toBeNull()
          return { key: match![1], value: match![2] }
        })

      const counts = new Map<string, number>()
      for (const { key } of entries) counts.set(key, (counts.get(key) ?? 0) + 1)
      expect(
        [...counts.entries()].filter(([, count]) => count !== 1),
        'duplicate YAML keys are ambiguous and must fail closed',
      ).toEqual([])
      expect(
        entries.filter(
          ({ key }) => key === 'DESKTOP_CODEX_USAGE_INGEST_ENABLED',
        ),
      ).toEqual([{ key: 'DESKTOP_CODEX_USAGE_INGEST_ENABLED', value: '"1"' }])
    },
  )

  test('packs only the self-contained Node server bundle', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )

    expect(deployScript).toContain('vp pack src/cloudrun/server.ts')
    expect(deployScript).toContain('pnpm run build:start')
    expect(deployScript).toContain('CI=true pnpm run build:start')
    expect(deployScript).toContain('OPENAGENTS_SKIP_START_BUILD')
    expect(deployScript).toContain('apps/start/dist/cloudrun/server.mjs')
    expect(deployScript).not.toContain('build:astro')
    expect(deployScript).toContain(
      'pnpm install --frozen-lockfile --ignore-scripts',
    )
    expect(deployScript.match(/--config\.ignore-scripts=true/g)).toHaveLength(2)
    expect(deployScript).not.toContain('astro-ui')
    expect(deployScript).toContain('! -f dist-cloudrun/server.mjs')
    expect(deployScript).not.toContain('preload.mjs')
    expect(deployScript).not.toContain('cloudflare-workers-stub')
    expect(deployScript).toContain('assert-self-contained-bundle.mjs')
    expect(deployScript.match(/--config\.node-linker=hoisted/g)).toHaveLength(2)
    expect(deployScript).toContain('--filter @openagentsinc/api-worker deploy')
    expect(deployScript).toContain('pnpm install --frozen-lockfile')
    expect(deployScript).toContain('cd "$REPO_ROOT"')
    expect(deployScript).not.toContain('--deps.never-bundle')
    expect(deployScript).toContain('gcloud run services update-traffic "$SERVICE"')
    expect(deployScript).toContain('--to-latest')
    expect(deployScript).toContain('status.latestReadyRevisionName')
  })

  test('keeps the owned Forge read route on the monolith network', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )
    const productionEnvironment = readFileSync(
      fileURLToPath(new URL('env-production.yaml', import.meta.url)),
      'utf8',
    )

    expect(deployScript).toContain('--network "$NETWORK"')
    expect(deployScript).toContain('--subnet "$SUBNETWORK"')
    expect(deployScript).toContain('--vpc-egress all-traffic')
    expect(productionEnvironment).toContain(
      'OPENAGENTS_FORGE_READ_BASE_URL: "https://forge-git-ezxz4mgdsq-uc.a.run.app"',
    )
    expect(productionEnvironment).toContain(
      'OPENAGENTS_FORGE_GIT_SERVICE_BASE_URL: "https://forge-git-ezxz4mgdsq-uc.a.run.app"',
    )
    expect(productionEnvironment).toContain(
      'OPENAGENTS_FORGE_GITHUB_MIRROR_REGISTRY:',
    )
    expect(productionEnvironment).toContain(
      '"destinationGithubRef":"refs/heads/forge/main"',
    )
    expect(productionEnvironment).toContain(
      '"destinationGithubRef":"refs/tags/forge-omega-import-2026-07-26"',
    )
    expect(productionEnvironment).toContain(
      'OPENAGENTS_FORGE_PUBLIC_WEB_READ_REPOSITORIES: "tenant.openagents/omega"',
    )
  })

  test('enables and authenticates Sarah LiveKit only in production', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )
    const productionEnvironment = readFileSync(
      fileURLToPath(new URL('env-production.yaml', import.meta.url)),
      'utf8',
    )
    const stagingEnvironment = readFileSync(
      fileURLToPath(new URL('env-staging.yaml', import.meta.url)),
      'utf8',
    )
    const productionSecrets =
      /if \[\[ "\$TARGET" == "production" \]\]; then\s+SET_SECRETS\+=\(\s+([\s\S]*?)\n  \)/u.exec(
        deployScript,
      )?.[1]

    expect(productionEnvironment).toContain(
      'SARAH_LIVEKIT_URL: "wss://livekit.openagents.com"',
    )
    // EP263-LK H3 (#9282): the committed value is the alpha's intended steady
    // state and must equal what production serves. It was "false" while
    // production served "true", so `main` described a closed alpha that was
    // open. check-livekit-admission-drift.mjs is the live half of this pin.
    expect(productionEnvironment).toContain(
      'SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED: "true"',
    )
    // The bounded provider-disconnect acceptance boundary stays committed-
    // closed. Arming is a deploy-time override for one approved window, never
    // a committed "true".
    expect(productionEnvironment).toContain(
      'SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED: "false"',
    )
    expect(stagingEnvironment).not.toContain('SARAH_LIVEKIT_URL:')
    expect(stagingEnvironment).not.toContain(
      'SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED:',
    )
    expect(stagingEnvironment).not.toContain(
      'SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED:',
    )
    expect(productionSecrets).toContain(
      'SARAH_LIVEKIT_SERVER_KEYS_JSON=oa-livekit-prod-server-keys:latest',
    )
    expect(productionSecrets).toContain(
      'SARAH_LIVEKIT_CONTROL_ROOT=oa-livekit-prod-sarah-control-root:latest',
    )
    expect(deployScript.match(/SARAH_LIVEKIT_SERVER_KEYS_JSON=/gu)).toHaveLength(
      1,
    )
    expect(deployScript.match(/SARAH_LIVEKIT_CONTROL_ROOT=/gu)).toHaveLength(1)
    expect(deployScript).not.toContain('SARAH_LIVEKIT_API_KEY=')
    expect(deployScript).not.toContain('SARAH_LIVEKIT_API_SECRET=')
  })

  test('carries the LiveKit admission state atomically in one revision', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )

    // EP263-LK H2 (#9282). The deploy must ship the intended admission state
    // with the revision it builds and smokes. Before this, every deploy
    // restored the committed value and an operator spent a second revision
    // re-arming admissions — the drift in H3 is what that habit produced.
    expect(deployScript).toContain('RENDERED_ENV_FILE')
    expect(deployScript).toContain('--env-vars-file "$RENDERED_ENV_FILE"')
    expect(deployScript).not.toContain('--env-vars-file "$ENV_FILE"')

    // Bounded operator intent is expressed as an explicit on/off override that
    // is applied BEFORE `gcloud run deploy`, not as a follow-up service update.
    expect(deployScript).toContain('SARAH_LIVEKIT_ADMISSIONS')
    expect(deployScript).toContain(
      'SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE',
    )
    expect(deployScript).toContain('apply_env_override')
    expect(deployScript).not.toContain('--update-env-vars')

    // The rendered copy is temporary; the committed file is never rewritten.
    expect(deployScript).toContain('cp "$ENV_FILE" "$RENDERED_ENV_FILE"')
  })

  test('detects committed-versus-serving LiveKit admission drift', () => {
    const committed = readFileSync(
      fileURLToPath(new URL('env-production.yaml', import.meta.url)),
      'utf8',
    )

    expect(LIVEKIT_ADMISSION_KEYS).toEqual([
      'SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED',
      'SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED',
    ])

    const parsed = parseCommittedAdmissionState(committed)
    expect(parsed).toEqual({
      SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED: 'true',
      SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED: 'false',
    })

    // Agreement is silence.
    expect(admissionDrift(parsed, parsed)).toEqual([])

    // The exact 2026-07-31 production divergence must be reported, not ignored.
    expect(
      admissionDrift(parsed, {
        SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED: 'false',
        SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED: 'false',
      }),
    ).toEqual([
      {
        key: 'SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED',
        committed: 'true',
        serving: 'false',
      },
    ])

    // A key missing from the serving revision is drift, not agreement: an
    // absent value is runtime-disabled and must never read as enabled.
    expect(admissionDrift(parsed, {})).toHaveLength(2)
  })

  test('reads the revision receiving all service traffic', () => {
    expect(
      resolveServingRevision({
        status: {
          latestReadyRevisionName: 'fault-on',
          traffic: [
            { revisionName: 'safe', percent: 100 },
            { revisionName: 'fault-on', percent: 0 },
          ],
        },
      }),
    ).toBe('safe')
    expect(() =>
      resolveServingRevision({
        status: {
          traffic: [
            { revisionName: 'one', percent: 50 },
            { revisionName: 'two', percent: 50 },
          ],
        },
      }),
    ).toThrow('exactly one revision serving 100 percent')
  })

  test('ships Vite Plus split chunks beside the server entry', () => {
    const dockerfile = readFileSync(
      fileURLToPath(new URL('../../Dockerfile', import.meta.url)),
      'utf8',
    )

    expect(dockerfile).toContain('COPY dist-cloudrun/*.mjs ./dist-cloudrun/')
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/node_modules ./node_modules',
    )
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/start-client ./start-client',
    )
    expect(dockerfile).toContain(
      'COPY dist-cloudrun/start-server ./start-server',
    )
    expect(dockerfile).not.toContain('astro-ui')
    expect(dockerfile).not.toContain('ASTRO_UI_DIR')
  })

  test('bundles owned workspace packages like the T3 Code pack pattern', () => {
    expect(
      shouldBundleCloudRunDependency('@openagentsinc/khala-sync-server'),
    ).toBe(true)
    expect(shouldBundleCloudRunDependency('effect')).toBe(false)
    expect(shouldBundleCloudRunDependency('nostr-effect/pure')).toBe(true)
  })

  test('rejects packages absent from the slim runtime image', () => {
    expect(
      externalRuntimeSpecifiers(`
        import fs from 'node:fs'
        import net from 'net'
        import { Effect } from 'effect'
        import '@openagentsinc/runtime-platform'
        import './local.mjs'
      `),
    ).toEqual(['@openagentsinc/runtime-platform', 'effect'])
  })
})
