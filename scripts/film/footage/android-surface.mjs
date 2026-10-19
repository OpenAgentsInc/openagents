// The Episode 290 extra take: the same dedicated-account conversation on the released Coder
// terminal, openagents.com, and the OpenAgents Android app (debug build, emulator).
// Run under `openagents browser run`, with FILM_WORK, FILM_ADB_SERIAL and FILM_CODER set.
// The Android app signs in through its debug credential file (DebugCredential.kt), so the
// emulator's own key is never linked to the shared test account.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import * as c from './cdp.mjs';
import { Session } from './terminal.mjs';
import { fit } from './replay-terminal.mjs';

const work = process.env.FILM_WORK, serial = process.env.FILM_ADB_SERIAL;
if (!work || !serial || !process.env.OPENAGENTS_CHROME_PORT) throw Error('Set FILM_WORK and FILM_ADB_SERIAL, and run under openagents browser run.');
fs.mkdirSync(work, { recursive: true, mode: 0o700 });
const adbBin = process.env.FILM_ADB || path.join(process.env.HOME, 'Library/Android/sdk/platform-tools/adb');
const adb = (args, opts = {}) => execFileSync(adbBin, ['-s', serial, ...args], { encoding: 'utf8', ...opts });
const pkg = 'com.openagents.app.debug';
const base = 'https://openagents.com', apiBase = 'https://api.openagents.com';
const keyLine = fs.readFileSync(path.join(process.env.HOME, 'work/.secrets/openagents-test-account.env'), 'utf8').split('\n').find(l => /^(export )?OPENAGENTS_TOKEN=/.test(l));
const token = keyLine?.split('=').slice(1).join('=').trim().replace(/^['"]|['"]$/g, '');
if (!token?.startsWith('oa_sk_')) throw Error('The dedicated test-account sign-in key is required.');
const now = () => Date.now() / 1000;
const manifest = { version: 2, kind: 'android', serial, base, beats: {}, web: [], sources: {}, cleaned: false };
const beat = name => { manifest.beats[name] = now(); console.log(name); };
const save = () => fs.writeFileSync(path.join(work, 'take.json'), JSON.stringify(manifest, null, 2));
const until = async (test, seconds = 90, what = 'capture readiness') => { const end = now() + seconds; while (!await test()) { if (now() > end) throw Error(`Timed out waiting for ${what}.`); await c.sleep(200); } };
async function api(method, route) {
  const r = await fetch(apiBase + route, { method, headers: { Authorization: 'Bearer ' + token } });
  const v = await r.json().catch(() => ({}));
  if (!r.ok) throw Error(`Dedicated-account API ${method} ${route}: ${r.status}`);
  return v;
}
const account = await api('GET', '/v1/account');
if (account.account.id !== 'acct_83d16a35955ef908') throw Error('Use the dedicated test account, not the owner account.');
const initial = new Set((await api('GET', '/v1/history?limit=200')).data.map(x => x.id));
const initialComputers = new Set((await api('GET', '/v1/computers')).computers.map(x => x.name));
const home = path.join(work, 'home'), state = path.join(work, 'terminal-state');
fs.mkdirSync(home, { recursive: true }); fs.mkdirSync(state, { recursive: true });
const computer = 'Episode290-android-' + Date.now();
const cleanEnv = { PATH: process.env.PATH, HOME: home, TERM: 'xterm-256color', LANG: 'en_US.UTF-8', OPENAGENTS_ORIGIN: base, OPENAGENTS_COMPUTER_NAME: computer };
const coder = process.env.FILM_CODER || path.join(process.env.HOME, '.openagents/bin/coder');
const occurrences = (text, s) => (text || '').split(s).length - 1;
const gate = async name => { fs.writeFileSync(path.join(work, 'step'), 'waiting-' + name); console.log('step: waiting-' + name); await until(() => fs.existsSync(path.join(work, 'go-' + name)), 900, name); };
let terminal, web, recording, watchWeb = false, webWatch, screen;
try {
  execFileSync(coder, ['login', '--key', '--state', state], { input: token + '\n', env: cleanEnv, stdio: ['pipe', 'pipe', 'pipe'], timeout: 45000 });
  terminal = new Session({ ...(await fit()), command: [coder, '--state', state], cwd: home, env: cleanEnv, inheritEnv: false });
  await terminal.waitFor('❯', 45000);
  await terminal.type('/sync on', { base: 10, jitter: 2 }); terminal.input('\r');
  await c.sleep(1200);
  await terminal.type('Reply with exactly: Ready on Android too.', { base: 12, jitter: 3 }); terminal.input('\r');
  let chat;
  await until(async () => { chat = (await api('GET', '/v1/history?limit=200')).data.find(x => !initial.has(x.id) && x.source === 'terminal'); return !!chat; }, 90, 'the synced chat');
  manifest.chat = chat.id; save();
  await until(async () => (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.some(m => m.role === 'assistant'), 120, 'the first answer');

  web = await c.launch({ width: 1120, height: 760, dpr: 2 });
  await web.send('Network.setCookie', { name: 'oa_cloud_session', value: token, url: base + '/' });
  await web.send('Network.setCookie', { name: 'oa_theme', value: 'dark', url: base + '/' });
  await c.navigate(web, `${base}/chat/${chat.id}`);
  await c.waitFor(web, `!!document.querySelector('textarea[name=q]')`, 30000, 'the real web reply composer');
  manifest.sources.web = `${base}/chat/${chat.id}`;

  // A fresh debug install's data, signed in with the credential file it reads once and deletes.
  adb(['shell', 'pm', 'clear', pkg]);
  adb(['shell', `run-as ${pkg} sh -c 'mkdir -p files && cat > files/debug-credential'`], { input: token + '\n' });
  adb(['shell', 'am', 'start', '-n', `${pkg}/com.openagents.app.MainActivity`]);
  manifest.sources.android = 'OpenAgents Android app (debug build) on the emulator, ' + adb(['shell', 'getprop', 'ro.build.version.release']).trim();
  // The person running the take opens the chat on Android (adb taps), then touches go-android.
  await gate('android');
  manifest.sources.terminal = execFileSync(coder, ['--version'], { encoding: 'utf8', env: cleanEnv }).trim();
  adb(['shell', 'rm', '-f', '/sdcard/film-android.mp4']);
  screen = spawn(adbBin, ['-s', serial, 'shell', 'screenrecord', '--bit-rate', '16000000', '/sdcard/film-android.mp4'], { stdio: 'ignore' });
  await c.sleep(1000);
  manifest.androidStart = now() - 0.6;
  recording = new c.Recorder(web, path.join(work, 'web-frames')); await recording.start();
  watchWeb = true;
  webWatch = (async () => { while (watchWeb) { try { manifest.web.push({ epoch: now(), text: await c.evaluate(web, "document.querySelector('.oa-thread')?.innerText || document.body.innerText") }); } catch {} await c.sleep(400); } })();
  await c.sleep(1500);
  beat('start');
  await c.sleep(700);
  beat('terminal_typing');
  await terminal.type('Reply with exactly: Terminal, web, and Android.', { base: 35, jitter: 10 });
  terminal.input('\r'); beat('terminal_sent');
  await until(async () => (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data.at(-1)?.text?.includes('Terminal, web, and Android.') , 120, 'the terminal answer');
  beat('terminal_answered');
  await until(() => occurrences(manifest.web.at(-1)?.text, 'Terminal, web, and Android.') >= 2, 30, 'the web');
  await c.sleep(4000);
  beat('all_updated');
  await c.sleep(700);
  beat('web_typing');
  await c.click(web, 'textarea[name=q]');
  await c.type(web, 'Now reply with exactly: Same chat on Android.', { base: 32, jitter: 8 });
  await c.pressEnter(web); beat('web_sent');
  await until(async () => { const d = (await api('GET', `/v1/history/${chat.id}/messages?limit=200`)).data; return d.at(-1)?.role === 'assistant' && d.at(-1)?.text?.includes('Same chat on Android.'); }, 120, 'the web answer');
  beat('web_answered');
  await until(() => occurrences(terminal.screen(), 'Same chat on Android.') >= 2 && occurrences(manifest.web.at(-1)?.text, 'Same chat on Android.') >= 2, 30, 'every screen');
  await c.sleep(4000);
  beat('all_finished');
  await c.sleep(1500); beat('end');
  manifest.webFrames = recording.frames; manifest.webSize = { width: 1120, height: 760, dpr: 2 }; save();
} finally {
  watchWeb = false;
  if (webWatch) await webWatch;
  if (recording) { await recording.stop().catch(() => {}); manifest.webFrames = recording.frames; }
  if (screen) {
    try { adb(['shell', 'pkill', '-INT', 'screenrecord']); } catch {}
    await c.sleep(2500);
    try { adb(['pull', '/sdcard/film-android.mp4', path.join(work, 'android-raw.mp4')]); adb(['shell', 'rm', '-f', '/sdcard/film-android.mp4']); } catch {}
  }
  if (terminal) { terminal.save(path.join(work, 'terminal.cast')); manifest.terminalStart = terminal.epoch0; terminal.input('\x03'); terminal.kill(); }
  if (web) await web.close();
  // Sign the emulator out: clear the debug app's data.
  try { adb(['shell', 'pm', 'clear', pkg]); } catch {}
  let clean = true;
  try {
    for (const row of (await api('GET', '/v1/history?limit=200')).data) if (!initial.has(row.id)) { try { await api('DELETE', '/v1/threads/' + encodeURIComponent(row.id)); } catch { clean = false; } }
    const sync = JSON.parse(fs.readFileSync(path.join(state, 'sync.json')));
    for (const session of Object.keys(sync.sent || {})) { try { await api('DELETE', '/v1/threads/synced/' + encodeURIComponent(session)); } catch {} }
    for (const row of (await api('GET', '/v1/computers')).computers) if (!initialComputers.has(row.name)) { try { await api('DELETE', '/v1/computers/' + encodeURIComponent(row.name)); } catch { clean = false; } }
    manifest.leftover = { chats: (await api('GET', '/v1/history?limit=200')).data.filter(r => !initial.has(r.id)).map(r => r.id),
      computers: (await api('GET', '/v1/computers')).computers.filter(r => !initialComputers.has(r.name)).map(r => r.name) };
    if (manifest.leftover.chats.length || manifest.leftover.computers.length) clean = false;
  } catch { clean = false; }
  manifest.cleaned = clean;
  fs.rmSync(state, { recursive: true, force: true });
  fs.rmSync(home, { recursive: true, force: true });
  save();
}
if (!manifest.cleaned) throw Error('Cleanup did not pass; inspect take.json.');
console.log('Recorded the Android take:', work);
