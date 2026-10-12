// web-chat.mp4: the signed-in web chat, dark. A question is typed into the
// composer and the real answer from the site's chat worker comes in. The
// wait between sending and the first words of the answer is shortened to
// about a second; everything else plays at its own speed.
//
// Answers vary. A take whose answer has no code block is deleted (through
// the chat's own Delete page) and taken again, up to FILM_TAKES times.
//
// env: FILM_BASE, FILM_SESSION_FILE, FILM_OUT, FILM_WORK, FILM_QUESTION
import path from "node:path";
import * as c from "./cdp.mjs";
import { signedIn, ask, answered, deleteOpenChat, ANSWER_TEXT } from "./web.mjs";

const env = process.env;
const base = env.FILM_BASE;
const question = env.FILM_QUESTION || "How do I reverse a linked list in Rust?";
const out = path.join(env.FILM_OUT, "web-chat.mp4");
const takes = Number(env.FILM_TAKES || 4);

for (let take = 1; take <= takes; take++) {
  const cdp = await signedIn(base, env.FILM_SESSION_FILE);
  await c.navigate(cdp, `${base}/`);
  await c.sleep(1200);
  const rec = new c.Recorder(cdp, path.join(env.FILM_WORK, "web-chat-frames"));
  await rec.start();
  await c.sleep(700);
  await ask(cdp, question, { base: 38, jitter: 30, beforeSend: 450 });
  const sent = rec.now();
  // The first words of the answer.
  await c.waitFor(cdp, `${ANSWER_TEXT} > 0`, 120000, "the first words");
  const first = rec.now();
  await answered(cdp);
  const done = rec.now();
  await c.sleep(2200);
  await rec.stop();
  const code = await c.evaluate(cdp, `document.querySelectorAll('.oa-thread pre').length`);
  if (!code && take < takes) {
    console.log(`web-chat: take ${take} has no code block; deleting it and asking again`);
    await deleteOpenChat(cdp, base);
    await cdp.close();
    continue;
  }
  const wait = first - 0.3 - (sent + 0.9);
  const speed = wait > 0.6 ? [{ from: sent + 0.9, to: first - 0.3, x: wait / 0.6 }] : [];
  rec.encode(out, { speed });
  await cdp.close();
  console.log(
    `web-chat: ${out} (take ${take}; sent ${sent.toFixed(1)} s, first words ${first.toFixed(1)} s, done ${done.toFixed(1)} s)`,
  );
  break;
}
