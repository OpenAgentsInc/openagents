// Firm reason: 1 (browser glue: how the page's own files are asked of the browser's cache; no logic of the game)
// A second visit asks the network for the page alone (issue 12). The files a first visit loads are
// under the release's own directory, never change, and are kept by the browser for good
// (scripts/web-stage.sh, scripts/web-deploy.sh). But three of the pinned tree's hosts ask for their
// modules with `cache: "no-store"` whatever the site (coax/web/frame-plan-host.js, layers-host.js,
// pack-host.js), so the browser fetched frame-plan.wasm and host-layers.wasm again on every visit.
// Those files are not ours to edit, so this, loaded before them (web/app.js), lets a request for a
// file of the release's own directory use the browser's copy. On this Mac's dev server nothing
// changes: there a build must always be the newest. It goes when the hosts ask as host.js does
// (no-store in development only).
const own = new URL("../", import.meta.url).href;
const dev = location.hostname === "127.0.0.1" || location.hostname === "localhost";
if (!dev) {
  const ask = globalThis.fetch;
  globalThis.fetch = (what, how) => {
    if (how && how.cache === "no-store" && (!how.method || how.method === "GET") && (typeof what === "string" || what instanceof URL) && new URL(what, location.href).href.startsWith(own) && /\.(wasm|wgsl)$/.test(new URL(what, location.href).pathname))
      return ask(what, { ...how, cache: "default" });
    return ask(what, how);
  };
}
// /agents is a module of its own (issue 30: web/site-agents.coax, the page with the chat with your agents), so the
// home page's first load carries none of it. The page's host asks for the page's module by the script's name for it
// (../build/site.wasm); at /agents (under a release's mount too) that request is for /agents' module, beside it.
// Moving from /agents to another page keeps the module that is up, which holds them all.
if (/\/agents\/?$/.test(location.pathname)) {
  const ask = globalThis.fetch;
  globalThis.fetch = (what, how) => {
    const url = (typeof what === "string" || what instanceof URL) ? new URL(what, location.href) : null;
    if (url && url.href.startsWith(own) && url.pathname.endsWith("/build/site.wasm")) return ask(new URL("site-agents.wasm", url), how);
    return ask(what, how);
  };
}
