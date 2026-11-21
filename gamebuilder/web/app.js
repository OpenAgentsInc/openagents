// Firm reason: 1 (browser glue: the one script the page loads; it names the hosts and holds no logic)
// gamebuilder.org's one script (issue 12). web/build.sh makes web/host/app.js from this file with
// `bun build`: the pinned tree's page host, its GPU host and all they import (some forty files of
// coax/web/, which are not ours to edit) as one file, so a first visit asks for one script, not forty.
// First, web/keep.js: how the hosts' own modules are asked of the browser's cache.
// The order is the page's: its own module first (site.coax, or site-agents.coax at /agents: web/keep.js; drawn by dom-host.js into
// #coaxial-root, which it reads from this script's address: ?app=site&root=coaxial-root, and
// &mount= in a release mounted under a path), then the glue between the page and the HUD, then the
// GPU host (hud.coax, named by this script tag's data-app). The hosts find their modules beside
// this file's folder (../build/NAME.wasm) and their workers in it (web/build.sh copies them there).
import "./keep.js";
import "../../coax/web/dom-host.js";
import "./hud-glue.js";
import "../../coax/web/host.js";
