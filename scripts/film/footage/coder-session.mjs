// terminal-coder.mp4 and web-sync.mp4, from one real session.
//
// Coder (this checkout's coder-new, signed in by phone-approve.mjs) runs in
// a real terminal in the demo repository, turns on /sync, and works the
// task: reads the files, runs the tests, edits, runs them again. The whole
// terminal session is kept as an asciicast and rendered by
// replay-terminal.mjs, with idle waits shortened.
//
// At the same time a signed-in desktop browser sits on the website's home
// page and is recorded live: the chat appears in the sidebar as Coder
// syncs it, and once Coder finishes, the browser opens it and shows the same
// conversation. The long stretch while Coder works is shortened.
//
// env: FILM_BASE, FILM_SESSION_FILE, FILM_CODER, FILM_CODER_STATE,
//   FILM_HOME, FILM_REPO, FILM_OUT, FILM_WORK, FILM_TASK
import path from "node:path";
import * as c from "./cdp.mjs";
import { Session } from "./terminal.mjs";
import { replay, fit } from "./replay-terminal.mjs";
import { signedIn } from "./web.mjs";

const env = process.env;
const base = env.FILM_BASE;
const task =
  env.FILM_TASK || "The tests in this repo are failing. Find out why, fix slugify, and run the tests.";

// The web side, recording before anything happens in the terminal.
const web = await signedIn(base, env.FILM_SESSION_FILE);
await c.navigate(web, `${base}/`);
await c.sleep(1200);
const rec = new c.Recorder(web, path.join(env.FILM_WORK, "web-sync-frames"));
await rec.start();

// The sidebar row of a chat synced from a terminal.
const ROW = `[...document.querySelectorAll('a.oa-nav-item')].find(a => /^Terminal/.test(a.querySelector('.oa-nav-item-detail')?.innerText || ''))`;
let appeared = null;
const watch = (async () => {
  while (appeared === null) {
    try {
      if (await c.evaluate(web, `!!${ROW}`)) appeared = rec.now();
    } catch {}
    await c.sleep(100);
  }
})();

const { cols, rows } = await fit();
const coder = new Session({
  cols,
  rows,
  command: [env.FILM_CODER, "--state", env.FILM_CODER_STATE],
  cwd: env.FILM_REPO,
  env: {
    HOME: env.FILM_HOME,
    OPENAGENTS_ORIGIN: base,
    OPENAGENTS_COMPUTER_NAME: env.FILM_COMPUTER || "MacBook Pro",
  },
});
await coder.waitFor("❯", 30000);
coder.mark("ready");
await c.sleep(1400);
await coder.type("/sync on", { base: 70, jitter: 40 });
await c.sleep(300);
coder.input("\r");
await coder.waitFor(/save to your account|Saving your chats/, 15000);
await c.sleep(1500);
await coder.type(task, { base: 30, jitter: 25 });
await c.sleep(400);
coder.input("\r");
const started = Date.now();
await coder.waitFor("Working", 30000);
// Done: Coder is no longer working and has said something after its tools.
for (;;) {
  await c.sleep(500);
  const s = coder.screen();
  if (!/Working/.test(s) && /pass/i.test(s)) break;
  if (Date.now() - started > 10 * 60 * 1000) throw new Error(`Coder didn't finish:\n${s}`);
}
await coder.settle(2500, 30000);
const doneAt = rec.now();
console.log(coder.screen());
await c.sleep(1500);
coder.mark("end");
coder.input("\x03");
await Promise.race([coder.exited, c.sleep(3000)]);
coder.kill();
const cast = path.join(env.FILM_WORK, "coder.cast");
coder.save(cast);

// The web side: wait for the synced chat to settle, then open it.
await c.waitFor(web, `!!${ROW}`, 60000, "the terminal chat in the sidebar");
await watch;
await c.sleep(2500);
const openAt = rec.now();
await c.evaluate(web, `(${ROW}).setAttribute('data-film-target', '1')`);
await c.click(web, "[data-film-target]");
await c.waitFor(web, `document.querySelectorAll('.oa-thread .oa-message').length >= 2`, 20000, "the synced chat");
await c.sleep(3500);
await rec.stop();
await web.close();

// web-sync: from just before the chat appears; the time Coder spends
// working (the page is still) plays in about 1.5 s.
const from = Math.max(0, appeared - 2.0);
const still = { from: appeared + 1.5, to: openAt - 1.0 };
const speed = still.to - still.from > 1.5 ? [{ ...still, x: (still.to - still.from) / 1.5 }] : [];
rec.encode(path.join(env.FILM_OUT, "web-sync.mp4"), { from, speed });
console.log(`web-sync: appeared ${appeared.toFixed(1)} s, Coder done ${doneAt.toFixed(1)} s, opened ${openAt.toFixed(1)} s`);

// From Coder's first screen to its finished answer (not the exit).
const mark = (name) => coder.events.find((e) => e[1] === "m" && e[2] === name)[0];
const r = await replay(cast, path.join(env.FILM_OUT, "terminal-coder.mp4"), {
  idle: Number(env.FILM_TERM_IDLE || 0.7),
  maxGap: 0.5,
  hold: 3.0,
  from: Math.max(0, mark("ready") - 0.4),
  to: mark("end"),
});
console.log(`terminal-coder: ${r.seconds.toFixed(1)} s`);
