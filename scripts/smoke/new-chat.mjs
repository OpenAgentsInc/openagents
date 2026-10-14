// Browser check for moving in place (#11322): "New chat" swaps only the
// main surface, with no document navigation, the same left-panel element,
// and the composer focused; back, forward, a chat's row, and a cold load
// of the new chat all work.
//
//   scripts/smoke/new-chat.sh [BASE_URL]      # default https://staging.openagents.com
//
// Signs in with a fresh staging test account when SMOKE_SIGNUP_TOKEN is set
// (new-chat.sh reads it like staging.sh, never printed); otherwise runs as
// a visitor. It sends one message to have a chat to start from.
// BUDGET_MS (default 100) is the most the paint after the click may take.
import { chromium } from 'playwright';

const base = (process.argv[2] || 'https://staging.openagents.com').replace(/\/$/, '');
const budget = Number(process.env.BUDGET_MS || 100);
let failed = 0;
function record(name, ok, detail = '') {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` (${detail})` : ''}`);
  if (!ok) failed++;
}

const browser = await chromium.launch();
// A test run (#11601): the site hides and soon deletes what it makes.
const context = await browser.newContext({
  viewport: { width: 1400, height: 900 },
  extraHTTPHeaders: { 'X-OpenAgents-Test': '1' },
});
const token = process.env.SMOKE_SIGNUP_TOKEN || '';
if (token) {
  const made = await fetch(`${base}/api/v1/accounts`, {
    method: 'POST',
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
    body: JSON.stringify({ label: 'New chat smoke' }),
  });
  const session = made.ok ? (await made.json()).session_token : '';
  record('a signed-in test account is made', !!session, `${made.status}`);
  if (session) await context.addCookies([{ name: 'oa_cloud_session', value: session, url: base }]);
}
const page = await context.newPage();
const documents = [];
page.on('request', (request) => {
  if (request.resourceType() === 'document' && request.frame() === page.mainFrame()) documents.push(request.url());
});

// Cold load of the new chat.
const cold = await page.goto(`${base}/`);
record('cold load of / shows the composer', cold.status() === 200 &&
  !!(await page.$('form[data-oa-composer] textarea')), `${cold.status()}`);

// A chat to start from.
await page.fill('form[data-oa-composer] textarea', 'Say hello in five words.');
await page.keyboard.press('Enter');
await page.waitForFunction(() => location.pathname.startsWith('/chat/'), null, { timeout: 30000 });
const chatPath = new URL(page.url()).pathname;
await page.waitForTimeout(2500);

async function mark() {
  documents.length = 0;
  await page.evaluate(() => {
    window.__oaSame = true;
    document.getElementById('oa-left-panel').__oaSame = true;
  });
}
async function same() {
  return page.evaluate(() => ({
    doc: window.__oaSame === true,
    panel: document.getElementById('oa-left-panel').__oaSame === true,
  }));
}

// New chat, five times from a chat's page.
const times = [];
for (let i = 0; i < 5; i++) {
  if (new URL(page.url()).pathname !== chatPath) {
    await page.click(`#oa-left-panel a.oa-nav-item[href="${chatPath}"]`);
    await page.waitForFunction((p) => location.pathname === p && document.getElementById('chat-content'), chatPath);
  }
  await page.waitForTimeout(1500); // warm: the new chat is loaded ahead
  await mark();
  const ms = await page.evaluate(() => new Promise((resolve) => {
    const link = document.querySelector('#oa-left-panel a.oa-nav-item[href="/"]');
    const start = performance.now();
    link.click();
    const check = () => {
      if (location.pathname === '/' && document.querySelector('.oa-home-stage') &&
          document.querySelector('form[data-oa-composer] textarea')) {
        requestAnimationFrame(() => resolve(performance.now() - start));
      } else {
        requestAnimationFrame(check);
      }
    };
    check();
  }));
  times.push(Math.round(ms));
  const s = await same();
  const focused = await page.evaluate(() => document.activeElement && document.activeElement.matches('form[data-oa-composer] textarea'));
  const current = await page.evaluate(() => !!document.querySelector('#oa-left-panel a.oa-nav-item[href="/"][aria-current="page"]'));
  record(`New chat #${i + 1}: no document navigation`, s.doc && documents.length === 0, documents.join(' '));
  record(`New chat #${i + 1}: the left panel is the same element`, s.panel);
  record(`New chat #${i + 1}: the composer is focused and New chat is current`, focused && current);
}
times.sort((a, b) => a - b);
record(`New chat paints within ${budget} ms`, times[2] <= budget, `click to paint ms: ${times.join(', ')}`);

// Back to the chat, forward to the new chat, in place.
await mark();
await page.goBack();
await page.waitForFunction((p) => location.pathname === p && document.getElementById('chat-content'), chatPath);
let s = await same();
record('back shows the chat in place', s.doc && s.panel && documents.length === 0);
await page.goForward();
await page.waitForFunction(() => location.pathname === '/' && document.querySelector('.oa-home-stage'));
s = await same();
record('forward shows the new chat in place', s.doc && s.panel && documents.length === 0);

// A chat's row from the new chat.
await mark();
await page.click(`#oa-left-panel a.oa-nav-item[href="${chatPath}"]`);
await page.waitForFunction((p) => location.pathname === p && document.getElementById('chat-content'), chatPath);
s = await same();
record("a chat's row opens it in place", s.doc && s.panel && documents.length === 0);

// A message from a new chat opened in place starts a new chat.
await page.click('#oa-left-panel a.oa-nav-item[href="/"]');
await page.waitForFunction(() => location.pathname === '/' && document.querySelector('.oa-home-stage'));
await page.fill('form[data-oa-composer] textarea', 'Say goodbye in five words.');
await page.keyboard.press('Enter');
await page.waitForFunction((p) => location.pathname.startsWith('/chat/') && location.pathname !== p, chatPath, { timeout: 30000 });
record('a message from it starts a new chat', true, new URL(page.url()).pathname);

await browser.close();
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
