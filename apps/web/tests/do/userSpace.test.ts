import { env } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

import type { WorkerEnv } from '../../src/effuse-host/env';

declare module 'cloudflare:test' {
  interface ProvidedEnv extends WorkerEnv {}
}

const getUserSpaceStub = (userSpaceId: string) => {
  if (!env.UserSpaceDO) throw new Error('UserSpaceDO binding missing');
  const id = env.UserSpaceDO.idFromName(userSpaceId);
  return env.UserSpaceDO.get(id);
};

describe('UserSpaceDO (DO SQLite)', () => {
  it('maintains an append-only event log with monotonic seq ordering', async () => {
    const userSpaceId = `userspace-${Date.now()}`;
    const stub = getUserSpaceStub(userSpaceId);

    const createAgent = async (agentJson: unknown) => {
      const response = await stub.fetch(
        new Request('http://example.com/api/user-space/agents', {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            'x-user-id': userSpaceId,
            authorization: 'Bearer token-1',
          },
          body: JSON.stringify({ json: agentJson }),
        }),
      );
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
      expect(typeof json.event?.eventId).toBe('string');
      expect(typeof json.event?.seq).toBe('number');
      return json.event as { eventId: string; seq: number };
    };

    const e1 = await createAgent({ name: 'a' });
    const e2 = await createAgent({ name: 'b' });

    expect(e2.seq).toBeGreaterThan(e1.seq);
    expect(e2.eventId).not.toBe(e1.eventId);

    const eventsResponse = await stub.fetch(
      new Request('http://example.com/api/user-space/events?after=0', {
        headers: { 'x-user-id': userSpaceId },
      }),
    );
    expect(eventsResponse.status).toBe(200);
    const eventsJson = (await eventsResponse.json()) as any;
    expect(eventsJson.ok).toBe(true);
    expect(Array.isArray(eventsJson.events)).toBe(true);

    const events = eventsJson.events as Array<any>;
    expect(events.length).toBe(2);
    expect(events[0].eventId).toBe(e1.eventId);
    expect(events[1].eventId).toBe(e2.eventId);

    const seqs = events.map((e) => e.seq);
    expect(seqs).toEqual([...seqs].sort((a, b) => a - b));
    expect(seqs[0]).toBe(e1.seq);
    expect(seqs[1]).toBe(e2.seq);
  });

  it('applies events idempotently by eventId', async () => {
    const userSpaceId = `userspace-apply-${Date.now()}`;
    const stub = getUserSpaceStub(userSpaceId);

    const eventId = `evt-${crypto.randomUUID()}`;

    const apply = async () => {
      const response = await stub.fetch(
        new Request('http://example.com/api/user-space/events', {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            'x-user-id': userSpaceId,
            authorization: 'Bearer token-1',
          },
          body: JSON.stringify({
            eventId,
            kind: 'test.event',
            json: JSON.stringify({ ok: true }),
            createdAtMs: 123,
          }),
        }),
      );
      expect(response.status).toBe(200);
      const json = (await response.json()) as any;
      expect(json.ok).toBe(true);
      expect(json.event?.eventId).toBe(eventId);
      return json as { inserted: boolean; event: { eventId: string; seq: number } };
    };

    const first = await apply();
    expect(first.inserted).toBe(true);

    const second = await apply();
    expect(second.inserted).toBe(false);
    expect(second.event.seq).toBe(first.event.seq);

    const eventsResponse = await stub.fetch(
      new Request('http://example.com/api/user-space/events?after=0', {
        headers: { 'x-user-id': userSpaceId },
      }),
    );
    const eventsJson = (await eventsResponse.json()) as any;
    const events = eventsJson.events as Array<any>;
    expect(events.length).toBe(1);
    expect(events[0].eventId).toBe(eventId);
  });
});

