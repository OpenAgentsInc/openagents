// terminal-plugins.mp4: Coder's Plugins screen, recorded live.
//
// Coder (this checkout's coder-new) starts in a scratch repository, the
// person types /plugins, the screen lists what is installed on this
// computer, the selection moves down a few rows slowly so each plugin's
// details can be read, one plugin's page opens, and Esc goes back. Nothing
// is turned on or off: only arrows, Enter on a plugin's page and Esc.
//
// Coder runs with the person's real HOME (so the list is what is really
// installed here) and an isolated --state folder (a copy of their settings,
// so nothing is written to ~/.openagents/coder). Sync stays off in that
// copy: no account file is copied, so nothing reaches openagents.com.
//
// env: FILM_CODER, FILM_CODER_STATE, FILM_REPO, FILM_OUT, FILM_WORK,
//   FILM_PLUGIN_DOWNS (rows to move down, default 5),
//   FILM_PLUGIN_OPEN (rows further down to the plugin whose page opens,
//   default 0), FILM_DETAIL_DOWNS (rows to move down on that page, default
//   0; a list of agents there is only browsed), FILM_PLUGIN_STEP (seconds on each row, default 1.7),
//   FILM_PROBE=1 (print each screen and skip the render)
import path from "node:path";
import { Session } from "./terminal.mjs";
import { replay, fit } from "./replay-terminal.mjs";

const env = process.env;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const downs = Number(env.FILM_PLUGIN_DOWNS || 5);
const openAfter = Number(env.FILM_PLUGIN_OPEN || 0);
const detailDowns = Number(env.FILM_DETAIL_DOWNS || 0);
const step = Number(env.FILM_PLUGIN_STEP || 1.7) * 1000;
const probe = env.FILM_PROBE === "1";
const show = (s, label) => probe && console.log(`--- ${label} ---\n${s.screen()}`);

const { cols, rows } = await fit();
const coder = new Session({
  cols,
  rows,
  command: [env.FILM_CODER, "--state", env.FILM_CODER_STATE],
  cwd: env.FILM_REPO,
  env: { OPENAGENTS_COMPUTER_NAME: env.FILM_COMPUTER || "MacBook Pro" },
});
await coder.waitFor("❯", 30000);
await coder.settle(1200, 15000);
coder.mark("ready");
show(coder, "ready");
await sleep(900);
await coder.type("/plugins", { base: 110, jitter: 50 });
await sleep(700);
coder.input("\r");
await coder.settle(800, 15000);
show(coder, "plugins");
await sleep(step);
for (let i = 0; i < downs + openAfter; i++) {
  coder.input("\x1b[B");
  await sleep(i < downs ? step : 600);
  show(coder, `down ${i + 1}`);
}
coder.input("\r");
await sleep(step + 600);
show(coder, "detail");
for (let i = 0; i < detailDowns; i++) {
  coder.input("\x1b[B");
  await sleep(step);
  show(coder, `detail down ${i + 1}`);
}
coder.input("\x1b");
await sleep(1800);
show(coder, "back");
coder.mark("end");
coder.input("\x03");
await sleep(300);
coder.input("\x03");
await Promise.race([coder.exited, sleep(3000)]);
coder.kill();
const cast = path.join(env.FILM_WORK, "plugins.cast");
coder.save(cast);
if (probe) process.exit(0);

// From Coder's first screen to the list again after Esc. The pacing is the
// recording's own: no wait is shortened or sped up (idle and maxGap are
// longer than any pause the script makes).
const mark = (name) => coder.events.find((e) => e[1] === "m" && e[2] === name)[0];
const r = await replay(cast, path.join(env.FILM_OUT, "terminal-plugins.mp4"), {
  idle: 60,
  maxGap: 60,
  hold: 0.3,
  from: Math.max(0, mark("ready") - 0.3),
  to: mark("end"),
});
console.log(`terminal-plugins: ${r.seconds.toFixed(1)} s`);
