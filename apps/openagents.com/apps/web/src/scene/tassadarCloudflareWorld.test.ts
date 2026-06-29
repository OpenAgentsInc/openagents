import { describe, expect, it } from 'vitest'

import {
  TASSADAR_REGION_BOUNDS,
  tassadarAvatarPositionCommandPayload,
  tassadarCloudflareWorldSubscriptionQueries,
} from './tassadarCloudflareWorld'

describe('tassadar Cloudflare world subscription plan', () => {
  it('subscribes to timeline-backed public activity world events', () => {
    const queries = tassadarCloudflareWorldSubscriptionQueries(
      'run.tassadar.executor.20260615',
    )

    expect(queries).toContain(
      'cloudflare-world:scope=run:run.tassadar.executor.20260615',
    )
    expect(queries).toContain(
      'cloudflare-world:scope=run:run.public_activity_timeline',
    )
    expect(queries).toContain(
      'cloudflare-world:region=region.run.tassadar.executor.20260615.street',
    )
  })

  it('builds avatar_position commands for the joined web avatar', () => {
    const payload = tassadarAvatarPositionCommandPayload({
      actorRef: 'actor.public.web',
      characterId: 'web',
      position: {
        movementMode: 'walking',
        pitch: 0.2,
        positionX: 12,
        positionY: 0,
        positionZ: -9,
        yaw: 1.4,
      },
    })

    expect(payload).toEqual({
      animation: 'walk',
      avatarRef: 'avatar.actor-public-web.web',
      position: { x: 12, y: 0, z: -9 },
      rotationY: 1.4,
    })
  })

  it('clamps avatar_position commands to the starter region', () => {
    const payload = tassadarAvatarPositionCommandPayload({
      actorRef: 'actor.public.web',
      characterId: 'web',
      position: {
        movementMode: 'running',
        pitch: 0,
        positionX: 999,
        positionY: -10,
        positionZ: -999,
        yaw: 0,
      },
    })

    expect(payload.animation).toBe('run')
    expect(payload.position).toEqual({
      x: TASSADAR_REGION_BOUNDS.maxX,
      y: TASSADAR_REGION_BOUNDS.minY,
      z: TASSADAR_REGION_BOUNDS.minZ,
    })
  })
})
