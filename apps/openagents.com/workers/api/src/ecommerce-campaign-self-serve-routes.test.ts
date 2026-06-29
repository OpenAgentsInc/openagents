import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { makeEcommerceCampaignSelfServeRoutes } from './ecommerce-campaign-self-serve-routes'
import {
  type CreatePrefilledWorkspaceInput,
  type PrefilledWorkspaceRecord,
  type PrefilledWorkspaceServiceShape,
  makePrefilledWorkspaceRecord,
} from './prefilled-workspace'

const fixtureNowIso = '2026-06-20T12:00:00.000Z'
const recordRuntime = {
  makeId: (prefix: string) => `${prefix}_1`,
  nowIso: () => fixtureNowIso,
}

class MemoryWorkspaceStore {
  readonly workspaces = new Map<string, PrefilledWorkspaceRecord>()

  async createWorkspace(
    input: CreatePrefilledWorkspaceInput,
  ): Promise<PrefilledWorkspaceRecord> {
    const record = makePrefilledWorkspaceRecord(input, recordRuntime)
    this.workspaces.set(record.id, record)
    return record
  }

  async readWorkspace(id: string): Promise<PrefilledWorkspaceRecord | undefined> {
    return this.workspaces.get(id)
  }

  async readOrClaimWorkspaceForHolder(
    id: string,
    holderUserId: string,
  ): Promise<PrefilledWorkspaceRecord | undefined> {
    throw new Error('Method not implemented.')
  }

  async readPrivateWorkspaceForTeamMember(
    id: string,
    holderUserId: string,
  ): Promise<PrefilledWorkspaceRecord | undefined> {
    throw new Error('Method not implemented.')
  }

  async recordFirstRunForOperator(
    id: string,
  ): Promise<PrefilledWorkspaceRecord | undefined> {
    throw new Error('Method not implemented.')
  }

  async recordFirstRunForHolder(
    id: string,
    holderUserId: string,
  ): Promise<PrefilledWorkspaceRecord | undefined> {
    throw new Error('Method not implemented.')
  }

  async recordFirstRunForPrivateTeamMember(
    id: string,
    holderUserId: string,
  ): Promise<PrefilledWorkspaceRecord | undefined> {
    throw new Error('Method not implemented.')
  }
}

describe('ecommerce campaign self-serve routes', () => {
  it('returns 503 when not enabled', async () => {
    const store = new MemoryWorkspaceStore() as unknown as PrefilledWorkspaceServiceShape
    const routes = makeEcommerceCampaignSelfServeRoutes({
      makeStore: () => store,
      enabled: false,
    })

    const request = new Request('https://openagents.com/api/public/ecommerce-campaign/workspaces', {
      method: 'POST',
    })

    const responseEffect = routes.routeEcommerceCampaignSelfServeRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(503)
  })

  it('creates workspace and returns 201 when enabled', async () => {
    const store = new MemoryWorkspaceStore() as unknown as PrefilledWorkspaceServiceShape
    const routes = makeEcommerceCampaignSelfServeRoutes({
      makeStore: () => store,
      enabled: true,
    })

    const request = new Request('https://openagents.com/api/public/ecommerce-campaign/workspaces', {
      method: 'POST',
    })

    const responseEffect = routes.routeEcommerceCampaignSelfServeRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(201)

    const json = (await response.json()) as any
    expect(json).toMatchObject({
      schema: 'openagents.ecommerce_campaign.self_serve_workspace.v1',
      promiseIds: ['business.ecommerce_workspace_pack.v1'],
      promiseState: 'yellow',
      inert: true,
      unclearedBlockerRefs: [
        'blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing',
      ],
    })
    expect(json.workspace).toBeDefined()
    // This is the public-safe self-serve projection (toPublicProjection), which
    // intentionally omits the holder-scoped `holderRef` so prospect/holder
    // identity material never leaks on the public endpoint. The e-commerce
    // vertical template identity is proven by the public-safe projectName.
    expect(json.workspace.holderRef).toBeUndefined()
    expect(json.workspace.projectName).toBe('Inventory-Aware Campaign Workspace')
  })
})
