// A genuine account chat on the released terminal, web, and fork's two native readers.
// Keep all output in FILM_WORK. No owner history or engine is opened.
// Run under `openagents browser run -- node scripts/film/footage/four-surface.mjs`.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import * as c from './cdp.mjs';
import { Session } from './terminal.mjs';
import { fit } from './replay-terminal.mjs';
import { recordSimulator, statusBar } from './setup.mjs';

const work = process.env.FILM_WORK;
if (!work || !process.env.OPENAGENTS_CHROME_PORT) throw Error('Set FILM_WORK and run under openagents browser run.');
fs.mkdirSync(work, { recursive: true, mode: 0o700 });
const fork = process.env.FILM_FORK;
const target = process.env.FILM_FORK_TARGET;
if (!fork || !target) throw Error('Set FILM_FORK and FILM_FORK_TARGET to the checked app build.');
const sim = process.env.FILM_SIM || '8010197D-517D-43DA-9559-46742EFABC65';
const base = 'https://openagents.com', apiBase = 'https://api.openagents.com';
const keyFile = path.join(process.env.HOME, 'work/.secrets/openagents-test-account.env');
const keyLine = fs.readFileSync(keyFile, 'utf8').split('\n').find(l => /^(export )?OPENAGENTS_TOKEN=/.test(l));
const token = keyLine?.split('=').slice(1).join('=').trim().replace(/^['"]|['"]$/g, '');
if (!token?.startsWith('oa_sk_')) throw Error('The dedicated test-account sign-in key is required.');
const now = () => Date.now() / 1000;
const manifest = { version: 1, sim, base, beats: {}, web: [], phone: [], sources: {}, cleaned: false };
const beat = name => { manifest.beats[name] = now(); console.log(name); };
const save = () => fs.writeFileSync(path.join(work, 'take.json'), JSON.stringify(manifest, null, 2));
const until = async (test, seconds = 90) => { const end = now() + seconds; while (!await test()) { if (now() > end) throw Error('Capture readiness timed out.'); await c.sleep(200); } };
async function api(method, route, body) {
  const r = await fetch(apiBase + route, { method, headers: { Authorization: 'Bearer ' + token, 'Content-Type': 'application/json' }, ...(body ? { body: JSON.stringify(body) } : {}) });
  const v = await r.json().catch(() => ({}));
  if (!r.ok) throw Error(`Dedicated-account API ${method} ${route}: ${r.status}`);
  return v;
}
const account = await api('GET', '/v1/account');
if (account.account.id !== 'acct_83d16a35955ef908') throw Error('Use the dedicated test account, not the owner account.');
const initial = new Set((await api('GET', '/v1/history?limit=200')).data.map(x => x.id));
let phoneReady = false, stop = false, watchWeb = false, webWatch, native, phoneTest, web, recording, phone, terminal;
const server = http.createServer(async (req, res) => {
  let payload = ''; for await (const chunk of req) { payload += chunk; if (payload.length > 1024 * 1024) { res.writeHead(413).end(); return; } }
  let data = {};
  if (req.url === '/key') data = { credential: token };
  else if (req.url === '/phone-ready') { phoneReady = true; beat('phone_ready'); }
  else if (req.url === '/phone-proof' && req.method === 'POST') { manifest.phone.push(JSON.parse(payload)); save(); }
  else if (req.url === '/status') data = { stop };
  else { res.writeHead(404).end(); return; }
  const body = JSON.stringify(data); res.writeHead(200, { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body), 'Cache-Control': 'no-store' }); res.end(body);
});
await new Promise(ok => server.listen(0, '127.0.0.1', ok));
const control = `http://127.0.0.1:${server.address().port}`;
const home = path.join(work, 'home'), state = path.join(work, 'terminal-state');
fs.mkdirSync(home, { recursive: true }); fs.mkdirSync(state, { recursive: true });
fs.writeFileSync(path.join(state, 'sync.json'), JSON.stringify({ on: false, chosen: true, sent: {} }), { mode: 0o600 });
const computer = 'Episode290-film-' + Date.now();
const cleanEnv = { PATH: process.env.PATH, HOME: home, TERM: 'xterm-256color', LANG: 'en_US.UTF-8', OPENAGENTS_ORIGIN: base, OPENAGENTS_COMPUTER_NAME: computer };
const coder = path.join(process.env.HOME, '.openagents/bin/coder');
try {
  // Login is not part of the terminal recording; the key is delivered through stdin.
  execFileSync(coder, ['login', '--key', '--state', state], { input: token + '\n', env: cleanEnv, stdio: ['pipe', 'pipe', 'pipe'], timeout: 45000 });
  const dimensions = await fit();
  terminal = new Session({ ...dimensions, command: [coder, '--state', state], cwd: home, env: cleanEnv, inheritEnv: false });
  await terminal.waitFor('❯', 45000);
  await terminal.type('/sync on', { base: 10, jitter: 2 }); terminal.input('\r');
  await c.sleep(1200);
  await terminal.type('Reply with exactly: Ready on all four screens.', { base: 12, jitter: 3 }); terminal.input('\r');
  let chat;
  await until(async () => { chat = (await api('GET', '/v1/history?limit=200')).data.find(x => !initial.has(x.id) && x.source === 'terminal'); return !!chat; });
  manifest.chat = chat.id;
  await until(async () => (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.some(m => m.role === 'assistant'), 120);
  console.log('Dedicated conversation ready:', chat.id);

  // Real web application, signed in only inside its disposable browser profile.
  web = await c.launch({ width: 1120, height: 760, dpr: 1 });
  await web.send('Network.setCookie', { name: 'oa_cloud_session', value: token, url: base + '/' });
  await web.send('Network.setCookie', { name: 'oa_theme', value: 'dark', url: base + '/' });
  await c.navigate(web, `${base}/chat/${chat.id}`);
  await c.waitFor(web, `!!document.querySelector('textarea[name=q]')`, 30000, 'the real web reply composer');
  manifest.sources.web = `${base}/chat/${chat.id}`;

  // The normal GPUI shell and saved-chat reader, rendered offscreen to avoid screen permissions.
  const app = path.join(work, 'Coder Film.app'), contents = path.join(app, 'Contents');
  fs.mkdirSync(path.join(contents, 'MacOS'), { recursive: true });
  fs.copyFileSync(path.join(target, 'mobile/examples/account-fixture'), path.join(contents, 'MacOS/account-fixture'));
  fs.chmodSync(path.join(contents, 'MacOS/account-fixture'), 0o755);
  fs.writeFileSync(path.join(contents, 'Info.plist'), `<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.openagents.coder.desktop.dev.film</string><key>CFBundleName</key><string>Coder Film</string><key>CFBundleExecutable</key><string>account-fixture</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>`);
  execFileSync('codesign', ['--force', '--sign', '-', app], { stdio: 'ignore' });
  const desktop = path.join(work, 'desktop');
  native = spawn(path.join(contents, 'MacOS/account-fixture'), [desktop, '--film'], { env: { ...process.env, OPENAGENTS_TOKEN: token, SDK_FIXTURE_API: apiBase + '/v1', SDK_FIXTURE_WEBSITE: base, SDK_FIXTURE_CHAT_IDS: JSON.stringify([chat.id]) }, stdio: ['ignore', fs.openSync(path.join(work, 'desktop.log'), 'w'), 'inherit'] });
  await until(() => { if (native.exitCode !== null) throw Error('Native desktop stopped; inspect desktop.log.'); return fs.existsSync(path.join(desktop, 'ready')); }, 60);
  manifest.sources.desktop = 'Coder Desktop native GPUI account reader';

  const phoneEnv = { ...process.env, ZERON_SKIP_CORE: '1', ZERON_ACCOUNT_FIXTURE: '1', CARGO_TARGET_DIR: target,
    TEST_RUNNER_CODER_FILM_CONFIG: JSON.stringify({ chat: chat.id, key_url: control + '/key', control }) };
  delete phoneEnv.OPENAGENTS_TOKEN;
  phoneTest = spawn('xcodebuild', ['test', '-project', 'apps/ios/Zeron.xcodeproj', '-scheme', 'Zeron', '-configuration', 'Debug', '-destination', 'id=' + sim, '-derivedDataPath', path.join(target, 'ios-derived'), 'CODE_SIGNING_ALLOWED=YES', 'CODE_SIGN_IDENTITY=-', 'SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE', '-only-testing:ZeronUITests/OpenAgentsHistoryTests/testRecordAccountChat'], { cwd: fork, env: phoneEnv, stdio: ['ignore', fs.openSync(path.join(work, 'phone.log'), 'w'), 'inherit'] });
  await until(() => { if (phoneTest.exitCode !== null) throw Error('Native phone stopped; inspect phone.log.'); return phoneReady; }, 90);
  manifest.sources.phone = 'OpenAgents Coder fork UIKit/Rust account reader on iOS Simulator';
  manifest.sources.terminal = execFileSync(coder, ['--version'], { encoding: 'utf8', env: cleanEnv }).trim();
  statusBar(sim);
  phone = await recordSimulator(sim, path.join(work, 'phone-raw.mp4'));
  manifest.phoneStart = phone.started;
  recording = new c.Recorder(web, path.join(work, 'web-frames')); await recording.start();
  watchWeb = true;
  webWatch = (async () => {
    while (watchWeb) {
      try { manifest.web.push({ epoch: now(), chat: chat.id, text: await c.evaluate(web, "document.querySelector('.oa-thread')?.innerText || document.body.innerText") }); }
      catch {}
      await c.sleep(400);
    }
  })();
  beat('start');
  await c.sleep(700);
  beat('terminal_typing');
  await terminal.type('Reply with exactly: Terminal, desktop, web, and phone.', { base: 35, jitter: 10 });
  terminal.input('\r'); beat('terminal_sent');
  let stageOne;
  await until(async () => {
    const page = await api('GET', `/v1/history/${chat.id}/messages?limit=200`);
    stageOne = page.data;
    return page.data.some(m => m.role === 'user' && m.text.includes('Reply with exactly: Terminal, desktop')) && page.data.at(-1)?.role === 'assistant';
  }, 120);
  beat('terminal_answered');
  await until(() => {
    const frames = JSON.parse(fs.readFileSync(path.join(desktop, 'frames.json')));
    return frames.at(-1).messages.length >= stageOne.length && manifest.phone.at(-1)?.count === `${stageOne.length} messages loaded` && manifest.web.at(-1)?.text.includes('Reply with exactly: Terminal, desktop');
  }, 30);
  beat('all_four_updated');
  await c.sleep(700);
  beat('web_typing');
  await c.click(web, 'textarea[name=q]');
  await c.type(web, 'Now reply with exactly: One conversation, four screens.', { base: 32, jitter: 8 });
  await c.pressEnter(web); beat('web_sent');
  let stageTwo;
  await until(async () => {
    const page = await api('GET', `/v1/history/${chat.id}/messages?limit=200`);
    stageTwo = page.data;
    return page.data.length > stageOne.length && page.data.some(m => m.role === 'user' && m.text.includes('Now reply with exactly')) && page.data.at(-1)?.role === 'assistant';
  }, 120);
  beat('web_answered');
  await until(() => {
    const frames = JSON.parse(fs.readFileSync(path.join(desktop, 'frames.json')));
    return frames.at(-1).messages.length >= stageTwo.length && manifest.phone.at(-1)?.count === `${stageTwo.length} messages loaded` && manifest.web.at(-1)?.text.includes('One conversation, four screens');
  }, 30);
  beat('all_four_finished');
  await c.sleep(700); beat('end');
  manifest.messages = (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.map(m => ({ id: m.id, role: m.role, text: m.text }));
  manifest.webFrames = recording.frames; save();
} finally {
  stop = true;
  watchWeb = false;
  if (webWatch) await webWatch;
  if (recording) { await recording.stop().catch(() => {}); manifest.webFrames = recording.frames; }
  if (phone) await phone.stop();
  if (terminal) { terminal.save(path.join(work, 'terminal.cast')); manifest.terminalStart = terminal.epoch0; terminal.input('\x03'); terminal.kill(); }
  if (native) { const p = path.join(work, 'desktop/stop'); fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, 'stop'); await until(() => native.exitCode !== null, 15).catch(() => native.kill()); }
  if (phoneTest) await until(() => phoneTest.exitCode !== null, 30).catch(() => phoneTest.kill());
  if (web) await web.close();
  const sync = JSON.parse(fs.readFileSync(path.join(state, 'sync.json')));
  let clean = true;
  for (const session of Object.keys(sync.sent || {})) { try { await api('DELETE', '/v1/threads/synced/' + encodeURIComponent(session)); } catch { clean = false; } }
  try { await api('DELETE', '/v1/computers/' + encodeURIComponent(computer)); } catch { clean = false; }
  manifest.cleaned = clean;
  // Retain recordings and timing evidence, never the sign-in file.
  fs.rmSync(state, { recursive: true, force: true });
  server.close(); save();
}
console.log('Recorded four real applications:', work);
