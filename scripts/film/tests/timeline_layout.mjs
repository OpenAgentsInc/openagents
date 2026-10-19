// Pure layout tests for the player's timeline helpers: no browser or server.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import vm from 'node:vm';

const page = readFileSync(new URL('../lab_player.html', import.meta.url), 'utf8');
const start = page.indexOf('// Timeline layout helpers.'), end = page.indexOf('// End of timeline layout helpers.');
assert.ok(start > 0 && end > start, 'the player marks its timeline layout helpers');
const context = vm.createContext({});
vm.runInContext(`const clock=t=>\`\${Math.floor(t/60)}:\${(t%60).toFixed(1).padStart(4,'0')}\`;` +
  page.slice(start, end) + ';this.h={clipForm,clipInitial,clipSpan,clipMeta,followLeft,LABEL_WIDTH,FOLLOW_MARGIN};', context);
const h = context.h;

test('short clips drop to one initial instead of a cut-off label', () => {
  assert.equal(h.clipForm(36), 'compact');
  assert.equal(h.clipForm(63.9), 'compact');
  assert.equal(h.clipForm(64), 'full');
  assert.equal(h.clipForm(300), 'full');
  assert.equal(h.clipForm(12), 'tiny');
  for (const [label, initial] of [['Desktop', 'D'], ['web', 'W'], ['↗ phone', 'P'], ['8 seconds', '8'], ['', '']])
    assert.equal(h.clipInitial(label), initial);
});

test('clip times give the in and out points', () => {
  assert.equal(h.clipSpan({ at: 5.5, dur: 1.5 }), '0:05.5–0:07.0 · 1.50s');
  // A clip too narrow for its start time shows only its length.
  assert.equal(h.clipMeta({ at: 5.5, dur: 1.5 }, 75), '1.50s');
  assert.equal(h.clipMeta({ at: 5.5, dur: 1.5 }, 120), '0:05.5 · 1.50s');
});

test('playback follow only pages forward', () => {
  const view = 1000;
  // The playhead is visible: the view stays where it is.
  assert.equal(h.followLeft(300, view, 900), 300);
  // The playhead is behind the view (the person scrolled ahead): no jump back.
  assert.equal(h.followLeft(800, view, 150), 800);
  // At the start with the playhead visible: no movement.
  assert.equal(h.followLeft(0, view, h.LABEL_WIDTH), 0);
  // The playhead reaches the right edge: page so it sits just past the track labels.
  const x = 300 + view - h.FOLLOW_MARGIN + 1;
  const left = h.followLeft(300, view, x);
  assert.ok(left > 300);
  assert.equal(x - left, h.LABEL_WIDTH + h.FOLLOW_MARGIN);
});
