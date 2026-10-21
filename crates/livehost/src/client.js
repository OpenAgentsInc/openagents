// livehost's live-reload client, injected into every page. It follows the
// host's manifest stream, swaps changed stylesheets in place, and reloads
// for anything else. It reports each update to /_livehost/ack so a
// benchmark can time it.
(() => {
  const { manifest, tenant } = document.currentScript.dataset;
  let current = manifest;
  const ack = (kind) => fetch("/_livehost/ack", {
    method: "POST", keepalive: true,
    body: JSON.stringify({ tenant, manifest: current, path: location.pathname, kind }),
  });
  // A page marks work it is still doing, such as loading a module, with
  // data-pending; the update counts once that is done. A mutation observer,
  // not a timer, so a background tab isn't throttled.
  const settled = (done) => {
    if (!document.querySelector("[data-pending]")) return done();
    const watch = new MutationObserver(() => {
      if (!document.querySelector("[data-pending]")) { watch.disconnect(); done(); }
    });
    watch.observe(document, { subtree: true, attributes: true, attributeFilter: ["data-pending"] });
  };
  addEventListener("load", () => settled(() => ack("load")));
  new EventSource(`/t/${tenant}/_live`).onmessage = (message) => {
    const update = JSON.parse(message.data);
    if (update.manifest === current) return;
    const links = [...document.querySelectorAll("link[data-asset]")];
    const patch = update.from === current && update.css;
    if (!patch || !Object.keys(patch).every((asset) => links.some((l) => l.dataset.asset === asset))) {
      return location.reload();
    }
    current = update.manifest;
    let left = Object.keys(patch).length;
    for (const [asset, href] of Object.entries(patch)) {
      const old = links.find((l) => l.dataset.asset === asset);
      const next = old.cloneNode();
      next.href = href;
      next.onload = () => { old.remove(); if (--left === 0) ack("css"); };
      old.after(next);
    }
  };
})();
