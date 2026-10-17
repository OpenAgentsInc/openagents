// Synthetic transport tests: no account, files, or provider calls.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('../../../crates/openagents-web/src/lab_card_view/video_transport.js', import.meta.url), 'utf8');
function fixture(answer) {
  const calls = [], removed = [], video = { pause() { removed.push('paused'); }, removeAttribute(name) { removed.push(name); }, load() {} };
  const context = vm.createContext({ URL, Map, Set, Number, JSON, Error, encodeURIComponent,
    clearTimeout() {}, poll: undefined,
    location: { origin: 'https://fixture.invalid' }, crypto: { randomUUID: () => 'synthetic' },
    window: { videoLabConfig: { document: 'film', space: 'project:private' } },
    state: { v: 1, cmp: '', data: { doc: { kind: 'cut.v1' } } },
    document: { querySelectorAll: selector => selector === 'video' ? [video] : [],
      getElementById: id => ({ replaceChildren() { removed.push(id); } }) },
    fetch: async (url, options) => {
      const call = { url: new URL(url, 'https://fixture.invalid'), options,
        body: options.body ? JSON.parse(options.body) : undefined };
      calls.push(call);
      const result = await answer(call, calls);
      return { ok: (result.status || 200) < 400, status: result.status || 200, json: async () => result.value };
    }
  });
  vm.runInContext(source, context);
  return { api: context.window.videoLabApi, calls, removed, state: context.state };
}
const plain = value => JSON.parse(JSON.stringify(value));

test('timeline edits and notes use the selected version and project', async () => {
  const f = fixture(async call => ({ value: call.url.pathname.endsWith('/notes') ? { saved: true } : { number: 2 } }));
  const command = { op: 'move', id: 'video:one', at: 2 };
  assert.deepEqual(plain(await f.api('/api/timeline', { doc: 'film', version: 1, command, summary: 'Move a clip' })), { version: 2 });
  assert.deepEqual(f.calls[0].body, { space: 'project:private', base: 1,
    change: { type: 'intent', intent: { intent: 'timeline', command } }, summary: 'Move a clip' });
  await f.api('/api/note', { doc: 'film', v: 1, t: 2.5, text: 'Shorten this shot' });
  assert.deepEqual(f.calls[1].body, { space: 'project:private', version: 1,
    locator: { at: 'time', seconds: 2.5 }, text: 'Shorten this shot' });
  for (const call of f.calls) {
    assert.equal(call.url.searchParams.get('space'), 'project:private');
    assert.equal(call.options.credentials, 'same-origin');
    assert.equal(call.options.cache, 'no-store');
    assert.equal(call.options.headers.Authorization, undefined);
  }
});

test('ambiguous preview submission reuses the same request key', async () => {
  const f = fixture(async (_, calls) => {
    if (calls.length === 1) throw Error('Connection lost');
    return { value: { id: 'job', status: 'queued' } };
  });
  await assert.rejects(f.api('/api/tool/lab_render', { document: 'film', version: 1 }));
  await f.api('/api/tool/lab_render', { document: 'film', version: 1 });
  assert.equal(f.calls[0].options.headers['Idempotency-Key'], f.calls[1].options.headers['Idempotency-Key']);
  assert.deepEqual(f.calls[1].body, { space: 'project:private', version: { document: 'film', version: 1 },
    want: { want: 'job', type: 'render', params: { quality: 'preview' } } });
});

test('saved cards supply scoped media URLs and old versions remain selectable', async () => {
  const f = fixture(async call => {
    const path = call.url.pathname;
    if (path.endsWith('/notes')) return { value: { data: [{ subject: { version: 1 }, locator: { at: 'time', seconds: 2 }, text: 'Note' }] } };
    if (path.endsWith('/timeline')) return { value: { clips: [{ track: 'video', source: '/private/never-fetch.mp4' }] } };
    if (path.endsWith('/card')) return { value: { primary: 'render/a', poster: 'poster/a', state: 'done' } };
    if (path.endsWith('/versions')) {
      assert.equal(call.url.searchParams.get('after'), '100');
      return { value: { data: [{ number: 200, summary: 'Latest', author: 'fixture', created_unix_ms: 1000 }] } };
    }
    if (path.endsWith('/versions/1')) return { value: { number: 1, summary: 'First', author: 'fixture', created_unix_ms: 1 } };
    return { value: { id: 'film', schema: 'cut.v1', latest: 200 } };
  });
  const document = await f.api('/api/doc?doc=film');
  assert.deepEqual(plain(document.versions.map(v => v.n)), [1, 200]);
  assert.equal(document.versions[0].renders.preview, '/v1/labs/artifacts/render%2Fa/content?space=project%3Aprivate');
  assert.equal(document.notes[0].version, 1);
  const timeline = await f.api('/api/timeline?doc=film&v=1');
  assert.equal(timeline.clips[0].thumbnail, '/v1/labs/artifacts/poster%2Fa/content?space=project%3Aprivate');
  assert.ok(f.calls.every(c => c.url.pathname.startsWith('/v1/labs/')));
});

test('refusals stay readable and loss of account access removes media', async () => {
  let denied = false;
  const f = fixture(async () => ({ status: denied ? 403 : 409,
    value: { error: { message: denied ? 'Sign in again.' : 'Reload this film.' } } }));
  await assert.rejects(f.api('/api/timeline', { doc: 'film', version: 1, command: {}, summary: 'Edit' }), /Reload this film/);
  assert.deepEqual(f.removed, []);
  denied = true;
  await assert.rejects(f.api('/api/doc?doc=film'), /Sign in again/);
  assert.ok(f.removed.includes('src'));
  assert.ok(f.removed.includes('stage'));
  assert.equal(f.state.data, null);
  assert.equal(f.state.timeline, null);
  assert.equal(f.state.doc, null);
  const calls = f.calls.length;
  await assert.rejects(f.api('/api/doc?doc=film'), /Open the film again/);
  assert.equal(f.calls.length, calls);
});


test('a late read cannot restore a player after another read loses access', async () => {
  let finish;
  const held = new Promise(resolve => { finish = resolve; });
  const f = fixture(async (_, calls) => calls.length === 1 ? held :
    { status: 403, value: { error: { message: 'Sign in again.' } } });
  const old = f.api('/api/doc?doc=film');
  await assert.rejects(f.api('/api/docs'), /Sign in again/);
  finish({ value: { id: 'film', schema: 'cut.v1', latest: 1 } });
  await assert.rejects(old, /Open the film again/);
  assert.equal(f.state.data, null);
  assert.equal(f.calls.length, 2);
});

test('earlier versions load in bounded pages without losing the latest', async () => {
  const f = fixture(async call => {
    if (call.url.pathname.endsWith('/versions')) {
      assert.equal(call.url.searchParams.get('limit'), '100');
      const after = Number(call.url.searchParams.get('after'));
      return { value: { data: Array.from({ length: Math.min(100, 205 - after) }, (_, i) =>
        ({ number: after + i + 1, summary: 'Synthetic', author: 'fixture', created_unix_ms: 1 })) } };
    }
    if (call.url.pathname.endsWith('/card')) return { value: { primary: null } };
    if (call.url.pathname.endsWith('/notes')) return { value: { data: [] } };
    return { value: { latest: 205, schema: 'cut.v1' } };
  });
  f.state.v = null;
  let document = await f.api('/api/doc?doc=film');
  assert.equal(document.versions.length, 100);
  assert.equal(document.older, true);
  await f.api('/api/older?doc=film');
  document = await f.api('/api/doc?doc=film');
  assert.equal(document.versions[0].n, 6);
  await f.api('/api/older?doc=film');
  document = await f.api('/api/doc?doc=film');
  assert.equal(document.versions.length, 205);
  assert.equal(document.versions.at(-1).n, 205);
  assert.equal(document.older, false);
});

test('film pages preserve the public cursor', async () => {
  const f = fixture(async call => {
    assert.equal(call.url.searchParams.get('after'), '17');
    assert.equal(call.url.searchParams.get('limit'), '100');
    return { value: { data: [{ id: 'film', title: 'Synthetic', latest: 2 }], next: 29 } };
  });
  assert.deepEqual(plain(await f.api('/api/docs?after=17')), { documents: [{ id: 'film', title: 'Synthetic', versions: 2 }], next: 29 });
});
