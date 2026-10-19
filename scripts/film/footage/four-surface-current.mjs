// The Episode 290 four-app take on the current apps: one dedicated-account chat in the
// released Coder terminal, openagents.com, the OpenAgents desktop app (its real window,
// a separate test instance), and the OpenAgents iPhone app in Simulator.
// Keep all output in FILM_WORK. No owner history, app data, or engine is opened.
//
//   FILM_WORK=DIR FILM_FORK=apps/openagents FILM_FORK_TARGET=CARGO_TARGET \
//   FILM_APP="…/OpenAgents Film.app" FILM_BRIDGE=DIR \
//   openagents browser run -- node scripts/film/footage/four-surface-current.mjs
//
// FILM_APP is a development bundle built by apps/openagents/scripts/run-macos-dev.sh with
// OPENAGENTS_DEV_APP_OUT, OPENAGENTS_DEV_BUNDLE_ID=com.openagents.desktop.dev.film and
// OPENAGENTS_TEST_RUN=1. FILM_BRIDGE is a folder served by a helper running in a terminal
// app that holds Screen Recording and Accessibility (it records and sizes that one window).
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
const appBundle = process.env.FILM_APP;
const bridge = process.env.FILM_BRIDGE;
if (!fork || !target || !appBundle || !bridge) throw Error('Set FILM_FORK, FILM_FORK_TARGET, FILM_APP and FILM_BRIDGE.');
const sim = process.env.FILM_SIM || '8010197D-517D-43DA-9559-46742EFABC65';
const webDpr = Number(process.env.FILM_WEB_DPR || 2);
const base = 'https://openagents.com', apiBase = 'https://api.openagents.com';
const keyFile = path.join(process.env.HOME, 'work/.secrets/openagents-test-account.env');
const keyLine = fs.readFileSync(keyFile, 'utf8').split('\n').find(l => /^(export )?OPENAGENTS_TOKEN=/.test(l));
const token = keyLine?.split('=').slice(1).join('=').trim().replace(/^['"]|['"]$/g, '');
if (!token?.startsWith('oa_sk_')) throw Error('The dedicated test-account sign-in key is required.');
const now = () => Date.now() / 1000;
const manifest = { version: 2, sim, base, beats: {}, web: [], phone: [], sources: {}, cleaned: false };
const beat = name => { manifest.beats[name] = now(); console.log(name); };
const save = () => fs.writeFileSync(path.join(work, 'take.json'), JSON.stringify(manifest, null, 2));
const until = async (test, seconds = 90, what = 'capture readiness') => { const end = now() + seconds; while (!await test()) { if (now() > end) throw Error(`Timed out waiting for ${what}.`); await c.sleep(200); } };
async function api(method, route, body) {
  const r = await fetch(apiBase + route, { method, headers: { Authorization: 'Bearer ' + token, 'Content-Type': 'application/json' }, ...(body ? { body: JSON.stringify(body) } : {}) });
  const v = await r.json().catch(() => ({}));
  if (!r.ok) throw Error(`Dedicated-account API ${method} ${route}: ${r.status}`);
  return v;
}
// One helper call through the bridge: "capwin", "axdrive" or "winid" with arguments.
let calls = 0;
async function helper(name, ...args) {
  const id = `${process.pid}-${++calls}`;
  fs.writeFileSync(path.join(bridge, 'req', id), [id, name, ...args].join(' '));
  const reply = path.join(bridge, 'rep', id);
  await until(() => fs.existsSync(reply), 20, `the window helper (${name})`);
  const text = fs.readFileSync(reply, 'utf8').trim(); fs.rmSync(reply);
  return text;
}
const account = await api('GET', '/v1/account');
if (account.account.id !== 'acct_83d16a35955ef908') throw Error('Use the dedicated test account, not the owner account.');
const initial = new Set((await api('GET', '/v1/history?limit=200')).data.map(x => x.id));
const initialComputers = new Set((await api('GET', '/v1/computers')).computers.map(x => x.name));
let phoneReady = false, stop = false, watchWeb = false, webWatch, native, phoneTest, web, recording, phone, terminal, desktopStop;
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
const coder = process.env.FILM_CODER || path.join(process.env.HOME, '.openagents/bin/coder');
const desktopData = path.join(work, 'desktop-data'), desktopHome = path.join(work, 'desktop-home');
const step = name => { const p = path.join(work, 'step'); fs.writeFileSync(p, name); console.log('step:', name); };
const gate = async (name, seconds = 600) => { const p = path.join(work, 'go-' + name); step('waiting-' + name); await until(() => fs.existsSync(p), seconds, name); };
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
  await until(async () => { chat = (await api('GET', '/v1/history?limit=200')).data.find(x => !initial.has(x.id) && x.source === 'terminal'); return !!chat; }, 90, 'the synced chat');
  manifest.chat = chat.id; save();
  await until(async () => (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.some(m => m.role === 'assistant'), 120, 'the first answer');
  console.log('Dedicated conversation ready:', chat.id);

  // Real web application, signed in only inside its disposable browser profile.
  web = await c.launch({ width: 1120, height: 760, dpr: webDpr });
  await web.send('Network.setCookie', { name: 'oa_cloud_session', value: token, url: base + '/' });
  await web.send('Network.setCookie', { name: 'oa_theme', value: 'dark', url: base + '/' });
  await c.navigate(web, `${base}/chat/${chat.id}`);
  await c.waitFor(web, `!!document.querySelector('textarea[name=q]')`, 30000, 'the real web reply composer');
  manifest.sources.web = `${base}/chat/${chat.id}`;

  // The OpenAgents desktop app itself: a test instance with its own data folder, HOME,
  // engine port and in-memory sign-in, so it never reads or writes the everyday app's data.
  fs.mkdirSync(desktopData, { recursive: true, mode: 0o700 }); fs.mkdirSync(desktopHome, { recursive: true, mode: 0o700 });
  native = spawn(path.join(appBundle, 'Contents/MacOS/openagents'), [], {
    env: { PATH: process.env.PATH, HOME: desktopHome, LANG: 'en_US.UTF-8', USER: process.env.USER, TMPDIR: process.env.TMPDIR,
      OPENAGENTS_DATA_DIR: desktopData, OPENAGENTS_IPC_PORT: process.env.FILM_APP_IPC_PORT || '49871', OPENAGENTS_TEST_RUN: '1',
      OPENAGENTS_AUTO_UPDATE: '0', CODER_DESKTOP_CREDENTIAL: token },
    stdio: ['ignore', fs.openSync(path.join(work, 'desktop.log'), 'w'), fs.openSync(path.join(work, 'desktop.err'), 'w')] });
  let bounds;
  await until(async () => { if (native.exitCode !== null) throw Error('Desktop app stopped; inspect desktop.err.'); bounds = await helper('axdrive', 'bounds', native.pid); return /^\d+ /.test(bounds); }, 90, 'the desktop window');
  bounds = await helper('axdrive', 'frame', native.pid, process.env.FILM_APP_X || '60', process.env.FILM_APP_Y || '60', '1120', '761');
  manifest.sources.desktopWindow = bounds;
  // The person running the take opens the chat in the app's Sessions list (see README), then
  // touches go-desktop. Clicks go only to the film app through `axdrive click`.
  await gate('desktop');
  const windowId = (await helper('axdrive', 'bounds', native.pid)).split(' ')[0];
  desktopStop = path.join(work, 'desktop-stop');
  fs.rmSync(desktopStop, { force: true });
  manifest.sources.desktop = 'OpenAgents desktop app (GPUI), test instance, window ' + windowId;

  const phoneEnv = { ...process.env, OPENAGENTS_SKIP_CORE: '1', OPENAGENTS_ACCOUNT_FIXTURE: '1', CARGO_TARGET_DIR: target,
    TEST_RUNNER_CODER_FILM_CONFIG: JSON.stringify({ chat: chat.id, key_url: control + '/key', control }) };
  delete phoneEnv.OPENAGENTS_TOKEN;
  phoneTest = spawn('xcodebuild', ['test', '-project', 'apps/ios/OpenAgents.xcodeproj', '-scheme', 'OpenAgents', '-configuration', 'Debug', '-destination', 'id=' + sim, '-derivedDataPath', path.join(target, 'ios-derived'), 'ARCHS=arm64', 'CODE_SIGNING_ALLOWED=YES', 'CODE_SIGN_IDENTITY=-', 'SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE', '-only-testing:OpenAgentsUITests/OpenAgentsHistoryTests/testRecordAccountChat'], { cwd: fork, env: phoneEnv, stdio: ['ignore', fs.openSync(path.join(work, 'phone.log'), 'w'), 'inherit'] });
  await until(() => { if (phoneTest.exitCode !== null) throw Error('Native phone stopped; inspect phone.log.'); return phoneReady; }, 900, 'the phone app');
  manifest.sources.phone = 'OpenAgents iPhone app (UIKit over the Rust core) on iOS Simulator';
  manifest.sources.terminal = execFileSync(coder, ['--version'], { encoding: 'utf8', env: cleanEnv }).trim();
  statusBar(sim);
  // The same wall clock for every recording from here on.
  await helper('capwin', windowId, path.join(work, 'desktop-raw.mp4'), path.join(work, 'desktop-frames.json'), desktopStop);
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
  await c.sleep(1500);
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
  }, 120, 'the terminal answer');
  beat('terminal_answered');
  await until(() => manifest.phone.at(-1)?.count === `${stageOne.length} messages loaded` && (manifest.web.at(-1)?.text.split('Terminal, desktop, web, and phone.').length ?? 0) > 2, 30, 'the phone and web to update');
  // The desktop polls every two seconds; give it the same allowance the phone gets.
  await c.sleep(Number(process.env.FILM_DESKTOP_SETTLE || 2500));
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
  }, 120, 'the web answer');
  beat('web_answered');
  await until(() => terminal.screen().split('One conversation, four screens.').length > 2 && manifest.phone.at(-1)?.count === `${stageTwo.length} messages loaded` && (manifest.web.at(-1)?.text.split('One conversation, four screens.').length ?? 0) > 2, 30, 'every screen to finish');
  await c.sleep(Number(process.env.FILM_DESKTOP_SETTLE || 2500));
  beat('all_four_finished');
  await c.sleep(1500); beat('end');
  manifest.messages = (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.map(m => ({ id: m.id, role: m.role, text: m.text }));
  manifest.webFrames = recording.frames; manifest.webSize = { width: 1120, height: 760, dpr: webDpr }; save();
} finally {
  stop = true;
  watchWeb = false;
  if (webWatch) await webWatch;
  if (recording) { await recording.stop().catch(() => {}); manifest.webFrames = recording.frames; }
  if (phone) await phone.stop();
  if (desktopStop) { fs.writeFileSync(desktopStop, 'stop'); await until(() => fs.existsSync(path.join(work, 'desktop-frames.json')), 30).catch(() => {}); }
  if (terminal) { terminal.save(path.join(work, 'terminal.cast')); manifest.terminalStart = terminal.epoch0; terminal.input('\x03'); terminal.kill(); }
  if (native && native.exitCode === null) { native.kill('SIGTERM'); await until(() => native.exitCode !== null, 15).catch(() => native.kill('SIGKILL')); }
  if (phoneTest) await until(() => phoneTest.exitCode !== null, 60).catch(() => phoneTest.kill());
  manifest.desktopExit = native?.exitCode ?? native?.signalCode;
  manifest.phoneTestExit = phoneTest?.exitCode;
  if (web) await web.close();
  let clean = true;
  // Every chat and computer the take made on the dedicated account, by difference.
  try {
    for (const row of (await api('GET', '/v1/history?limit=200')).data) {
      if (!initial.has(row.id)) { try { await api('DELETE', '/v1/threads/' + encodeURIComponent(row.id)); } catch { clean = false; } }
    }
    const sync = JSON.parse(fs.readFileSync(path.join(state, 'sync.json')));
    for (const session of Object.keys(sync.sent || {})) { try { await api('DELETE', '/v1/threads/synced/' + encodeURIComponent(session)); } catch {} }
    for (const row of (await api('GET', '/v1/computers')).computers) {
      if (!initialComputers.has(row.name)) { try { await api('DELETE', '/v1/computers/' + encodeURIComponent(row.name)); } catch { clean = false; } }
    }
    manifest.leftover = { chats: (await api('GET', '/v1/history?limit=200')).data.filter(r => !initial.has(r.id)).map(r => r.id),
      computers: (await api('GET', '/v1/computers')).computers.filter(r => !initialComputers.has(r.name)).map(r => r.name) };
    if (manifest.leftover.chats.length || manifest.leftover.computers.length) clean = false;
  } catch { clean = false; }
  manifest.cleaned = clean;
  // Retain recordings and timing evidence, never a sign-in.
  fs.rmSync(state, { recursive: true, force: true });
  fs.rmSync(path.join(desktopData, 'openagents-account/secrets'), { recursive: true, force: true });
  server.close(); save();
}
if (!manifest.cleaned || manifest.phoneTestExit !== 0) throw Error('Capture or cleanup did not pass; inspect the retained private logs.');
console.log('Recorded four real applications:', work);
