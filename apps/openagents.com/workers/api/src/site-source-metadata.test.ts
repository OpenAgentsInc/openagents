import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { parseJsonUnknown } from './json-boundary'
import {
  OPENAGENTS_SITE_METADATA_PATH,
  openAgentsSiteMetadataFromProject,
  parseOpenAgentsSiteMetadata,
  serializeOpenAgentsSiteMetadata,
} from './site-source-metadata'
import type {
  AutopilotSiteDeployment,
  AutopilotSiteProject,
  AutopilotSiteVersion,
} from './sites'

const project = (
  input: Partial<AutopilotSiteProject> = {},
): AutopilotSiteProject => ({
  accessMode: 'public',
  activeDeploymentId: 'site_deployment_1',
  activeVersionId: 'site_version_1',
  archivedAt: null,
  createdAt: '2026-06-05T00:00:00.000Z',
  id: 'site_project_otec',
  ownerUserId: 'github:14167547',
  projectId: 'team_project_sites',
  prompt: 'Build the OTEC public Site.',
  slug: 'ben-otec',
  softwareOrderId: 'software_order_otec',
  sourceRepository: {
    name: 'openagents',
    owner: 'bensilone',
    provider: 'github',
    ref: 'main',
  },
  status: 'approved',
  teamId: 'team_openagents',
  title: 'Ben OTEC',
  updatedAt: '2026-06-05T00:01:00.000Z',
  visibility: 'public',
  ...input,
})

const version = (
  input: Partial<AutopilotSiteVersion> = {},
): AutopilotSiteVersion => ({
  artifactManifestR2Key: 'sites/ben-otec/manifest.json',
  buildCommand: 'bun run build',
  buildLogR2Key: 'sites/ben-otec/build.log',
  buildStatus: 'saved',
  createdAt: '2026-06-05T00:02:00.000Z',
  createdByRunId: 'agent_run_otec',
  createdByUserId: 'github:14167547',
  d1BindingName: 'DB',
  id: 'site_version_saved',
  metadata: {},
  r2BindingName: 'ASSETS',
  rejectedAt: null,
  savedAt: '2026-06-05T00:03:00.000Z',
  siteId: 'site_project_otec',
  sourceArchiveR2Key: 'sites/ben-otec/source.tar.gz',
  sourceCommitSha: 'abc1234',
  sourceKind: 'autopilot_generated',
  staticAssetsManifest: { assets: {} },
  workerModuleR2Key: null,
  ...input,
})

const deployment = (
  input: Partial<AutopilotSiteDeployment> = {},
): AutopilotSiteDeployment => ({
  activatedAt: '2026-06-05T00:04:00.000Z',
  createdAt: '2026-06-05T00:03:30.000Z',
  deployedByUserId: 'github:14167547',
  disabledAt: null,
  dispatchNamespace: null,
  externalDeploymentId: null,
  failedAt: null,
  id: 'site_deployment_1',
  rolledBackAt: null,
  runtimeKind: 'omega_static_r2',
  runtimeScriptName: null,
  siteId: 'site_project_otec',
  slug: 'ben-otec',
  startedAt: '2026-06-05T00:03:40.000Z',
  status: 'active',
  updatedAt: '2026-06-05T00:04:00.000Z',
  url: 'https://sites.openagents.com/ben-otec',
  versionId: 'site_version_saved',
  ...input,
})

describe('.openagents/site.json metadata', () => {
  test('derives safe metadata from Site project, version, and deployment state', async () => {
    const metadata = openAgentsSiteMetadataFromProject({
      activeDeployment: deployment(),
      agentSurface: {
        agentReferralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
        capabilityManifestUrl: '/.well-known/openagents.json',
        openAgentsJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        preset: 'proof_and_challenge',
        proofUrl: '/api/public/proof/otec',
        publicSourceRef: 'site_ref_otec_ben',
        referralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
      },
      hostedProjectId: 'hosted_site_otec',
      project: project(),
      updatedAt: '2026-06-05T00:05:00.000Z',
      version: version(),
    })
    const text = await Effect.runPromise(serializeOpenAgentsSiteMetadata(metadata))

    expect(OPENAGENTS_SITE_METADATA_PATH).toBe('.openagents/site.json')
    expect(parseJsonUnknown(text)).toEqual({
      accessMode: 'public',
      activeDeploymentId: 'site_deployment_1',
      agentSurface: {
        agentReferralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
        capabilityManifestUrl: '/.well-known/openagents.json',
        openAgentsJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        preset: 'proof_and_challenge',
        proofUrl: '/api/public/proof/otec',
        publicSourceRef: 'site_ref_otec_ben',
        referralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
      },
      bindings: {
        d1: 'DB',
        r2: 'ASSETS',
      },
      hostedProjectId: 'hosted_site_otec',
      lastSavedVersionId: 'site_version_saved',
      schemaVersion: 'openagents.site.v1',
      siteId: 'site_project_otec',
      softwareOrderId: 'software_order_otec',
      source: {
        name: 'openagents',
        owner: 'bensilone',
        provider: 'github',
        ref: 'main',
      },
      target: {
        runtimeKind: 'omega_static_r2',
        slug: 'ben-otec',
        url: 'https://sites.openagents.com/ben-otec',
      },
      updatedAt: '2026-06-05T00:05:00.000Z',
      visibility: 'public',
    })
  })

  test('parses and round-trips metadata without secret-bearing fields', async () => {
    const text = `{
      "schemaVersion": "openagents.site.v1",
      "siteId": null,
      "accessMode": "owner_admins",
      "visibility": "private",
      "source": null,
      "target": {
        "runtimeKind": "omega_static_r2",
        "slug": "draft-site",
        "url": null
      },
      "bindings": {
        "d1": null,
        "r2": null
      },
      "lastSavedVersionId": null,
      "activeDeploymentId": null
    }`
    const metadata = await Effect.runPromise(parseOpenAgentsSiteMetadata(text))
    const serialized = await Effect.runPromise(
      serializeOpenAgentsSiteMetadata(metadata),
    )

    await expect(
      Effect.runPromise(parseOpenAgentsSiteMetadata(serialized)),
    ).resolves.toEqual(metadata)
  })

  test('accepts public agent surface presets used by generated Sites', async () => {
    const text = `{
      "schemaVersion": "openagents.site.v1",
      "siteId": "site_project_agent",
      "accessMode": "public",
      "visibility": "public",
      "target": {
        "runtimeKind": "omega_static_r2",
        "slug": "agent-site",
        "url": "https://sites.openagents.com/agent-site"
      },
      "bindings": {
        "d1": null,
        "r2": null
      },
      "lastSavedVersionId": null,
      "activeDeploymentId": null,
      "agentSurface": {
        "preset": "openagents_network",
        "capabilityManifestUrl": "/.well-known/openagents.json",
        "proofUrl": "/api/public/proof/otec"
      }
    }`

    await expect(
      Effect.runPromise(parseOpenAgentsSiteMetadata(text)),
    ).resolves.toMatchObject({
      agentSurface: {
        preset: 'openagents_network',
      },
    })
  })

  test('rejects unsupported access modes and runtime kinds', async () => {
    await expect(
      Effect.runPromise(
        parseOpenAgentsSiteMetadata(`{
          "schemaVersion": "openagents.site.v1",
          "siteId": "site_project_1",
          "accessMode": "workspace_all",
          "visibility": "private",
          "target": {
            "runtimeKind": "lambda",
            "slug": "bad-site"
          },
          "bindings": {
            "d1": null,
            "r2": null
          },
          "lastSavedVersionId": null,
          "activeDeploymentId": null
        }`),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsSiteMetadataValidationError',
    })
  })

  test('rejects secret-shaped metadata values and keys', async () => {
    await expect(
      Effect.runPromise(
        parseOpenAgentsSiteMetadata(`{
          "schemaVersion": "openagents.site.v1",
          "siteId": "site_project_1",
          "accessMode": "owner_admins",
          "visibility": "private",
          "target": {
            "runtimeKind": "omega_static_r2",
            "slug": "bad-site"
          },
          "bindings": {
            "d1": "DB",
            "r2": "ASSETS"
          },
          "lastSavedVersionId": null,
          "activeDeploymentId": null,
          "auth_token": "gho_abcdefghijklmnopqrstuvwxyz"
        }`),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsSiteMetadataUnsafe',
    })

    await expect(
      Effect.runPromise(
        serializeOpenAgentsSiteMetadata({
          accessMode: 'owner_admins',
          activeDeploymentId: null,
          bindings: { d1: null, r2: null },
          lastSavedVersionId: null,
          schemaVersion: 'openagents.site.v1',
          siteId: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
          target: {
            runtimeKind: 'omega_static_r2',
            slug: 'bad-site',
          },
          visibility: 'private',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsSiteMetadataUnsafe',
    })
  })
})
