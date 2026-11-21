// Firm reason: 1 (runs in the browser page: presses the game types' names on the ring, holds keys, clicks the world and reads the page's clock; the budget and the verdict are tools/entrance-check.coax's)
// What tools/entrance-check.coax runs in the page, in headless Chrome with WebGPU, to time the
// entrance (issue 14; docs/web-world.md, "The first second"): for each game type, from the press
// on its card, how long until the world takes a key (the character, the car or the ship moves) and
// until a thing is placed with the hotbar's first tool. It decides nothing: it answers a line a
// type, "KIND FIRST_MS PLACED_MS" (-1: it never happened within the wait), and the tool judges.
// The page is opened with ?observe=1, so the world reports its own frame (web/observe.coax).
// The dev server has no login service, so while the builder still asks for a login the two login
// calls are answered here, as scripts/web-shots.sh does; a builder open to guests needs none.
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  const real = window.fetch;
  window.fetch = (u, o) => {
    const s = String(u);
    if (s.includes("/api/auth/email/start")) return Promise.resolve(new Response('{"state":"sent"}', { status: 200 }));
    if (s.includes("/api/auth/email/verify")) return Promise.resolve(new Response('{"state":"logged-in","account":"acct_1","email":"dev@example.test"}', { status: 200 }));
    return real(u, o);
  };
  const until = async (f, ms = 20000) => { const end = performance.now() + ms; for (;;) { const v = f(); if (v) return v; if (performance.now() > end) throw new Error("the page did not get there: " + f); await sleep(50); } };
  const key = (code, k, down) => dispatchEvent(new KeyboardEvent(down ? "keydown" : "keyup", { code, key: k, bubbles: true }));
  const picker = () => document.querySelector(".gs.pick:not(.asking) .kind");
  // to the picker: as a guest when the builder is open, else through the login
  await until(() => globalThis.coaxial?.ready && document.querySelector(".gs"));
  await sleep(300);
  if (!picker() && !document.querySelector(".gs.v2")) {
    const way = document.getElementById("nav.login");
    if (way) way.click();
    for (const text of ["dev@example.test", "123456"]) {
      const field = await until(() => picker() || [...document.querySelectorAll("#coaxial-root input")].find((x) => !x.hidden && !/gs-/.test(x.className) && x.value === ""));
      if (picker()) break;
      field.focus(); field.value = text; field.dispatchEvent(new Event("input", { bubbles: true }));
      await sleep(200);
      field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, bubbles: true }));
      await sleep(900);
    }
  }
  const only = globalThis.gbEntranceOnly;   // (a list of kinds, for looking at one)
  const wait = globalThis.gbEntranceWait || 12000;
  const lines = [];
  const kinds = (await until(() => picker() && document.querySelectorAll(".kind"))).length;
  for (let kind = 0; kind < kinds; kind++) {
    if (only && !only.includes(kind)) continue;
    if (!picker()) { document.getElementById("me").click(); (await until(() => document.getElementById("type"))).click(); }
    const card = await until(() => picker() && document.getElementById("kind." + kind));
    await sleep(700);   // (the camera is back up at the picker's pose)
    const count = () => Number((document.querySelector(".gs-blocks")?.textContent || "0").split("\n")[0]) || 0;
    const t0 = performance.now();
    card.click();
    let first = -1, placed = -1, at = null, had = -1, tool = false, tried = 0, next = 0;
    // where the world is clicked, in turn, until a thing is placed: shares of the window
    const spots = [[0.5, 0.62], [0.42, 0.55], [0.6, 0.5], [0.36, 0.42], [0.5, 0.36]];
    while (performance.now() - t0 < wait && (first < 0 || placed < 0)) {
      await frame();
      const now = performance.now() - t0;
      const o = globalThis.gamebuilderObservation;
      const up = document.querySelector(".gs.v2.g" + kind) && o && o.game_type === kind;
      if (!up) continue;
      document.activeElement?.blur?.();
      if (first < 0) {
        // W and D held (side on W is a jump and D runs; a car is driven and steered; a ship flies ahead and right)
        key("KeyW", "w", true); key("KeyD", "d", true);
        // (a car settling on its wheels as it drops in has not been driven: racing is taken by the car's own speed, a metre a second)
        if (!at) at = [o.x_cm, o.z_cm];
        else if (kind === 7 ? o.race_speed_cm >= 100 : Math.abs(o.x_cm - at[0]) + Math.abs(o.z_cm - at[1]) >= 5) { first = Math.round(now); key("KeyW", "w", false); key("KeyD", "d", false); }
      }
      if (placed < 0) {
        if (had < 0) had = count();
        if (count() > had) { placed = Math.round(now); continue; }
        // the hotbar's first tool, then a press on the world
        if (!tool) { key("Digit1", "1", true); key("Digit1", "1", false); tool = true; next = now + 30; continue; }
        if (now < next) continue;
        const [sx, sy] = spots[tried % spots.length]; tried++; next = now + 120;
        const x = Math.round(innerWidth * sx), y = Math.round(innerHeight * sy);
        const el = document.elementFromPoint(x, y) || document.body;
        if (el.closest(".chat,.nav,.hotdock,a,button,input,textarea,select")) continue;
        const press = { clientX: x, clientY: y, pointerId: 1, pointerType: "mouse", button: 0, bubbles: true, cancelable: true };
        el.dispatchEvent(new PointerEvent("pointermove", { ...press, buttons: 0 }));
        el.dispatchEvent(new PointerEvent("pointerdown", { ...press, buttons: 1 }));
        // (first person holds the pointer on its first press: the dev page's stand-in for the browser's lock)
        if (kind === 1) globalThis.coaxial?.pointerLock?.(1);
        el.dispatchEvent(new PointerEvent("pointerup", { ...press, buttons: 0 }));
      }
    }
    key("KeyW", "w", false); key("KeyD", "d", false); key("Escape", "Escape", true); key("Escape", "Escape", false);
    if (kind === 1) globalThis.coaxial?.pointerLock?.(0);
    lines.push(`${kind} ${first} ${placed}`);
  }
  return lines.join("\n");
})()
