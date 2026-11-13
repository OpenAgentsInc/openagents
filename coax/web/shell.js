// Firm reason: 1 (browser glue: carries a page into the app shell's frame and its words back; decides no address)
// The app shell (docs/app-shell.md) keeps the site's side up while you move between its pages. The side is
// the chat's module (chat-web), the page's own shell at the top of the window. The pages it draws itself (the
// home page, the chat, Settings) change in place. Every other page of the shell's, another module's (Projects,
// the worlds' list, a world's editor, the older settings pages: site-side.coax ss_held), is held beside the
// side in a frame (components/kit/listing.coax k_held), one frame for each page moved to. The page left behind
// goes with its frame: its canvas, GPU device, sockets, workers, listeners and timers. Which address is which
// page is the chat's module's to say; this file only carries words across the frame.
//
// At the top of the window. A page of the shell's names itself on its host's script, data-shell="page" (always)
// or data-shell="edit" (when its address has ?edit). Opened at the top of the window, shellTakes() makes it the
// shell: it empties the page, opens the chat's module there as the page's own shell (dom-host.js ...&top), and
// answers true; the page's own host then never starts, and the shell holds the same address in its frame, where
// the page runs as itself. shellHost() gives the frame the shell's three words (globalThis.coaxialShell):
//   go(url)          an address the held page goes to: the chat's module keeps it (pushes it and draws it) or the
//                    window goes there (event 15)
//   step(win, text)  how far the page has come, "PERCENT WHAT", for the loading screen (event 14)
//   drawn(win)       the page has drawn: the loading screen goes (event 14, slot 1)
// A word from any frame but the one shown now is not heard.
//
// In the frame (shellHeld). The page says how far it has come (shellWatch: its loading screen's words) and that it
// drew (shellDrawn); an address it goes to is the shell's (shellGo), and so is a plain press on a link in it.

const SHELL = "chat-web";
const parentShell = (() => { try { return window.parent !== window ? window.parent.coaxialShell || null : null; } catch { return null; } })();
// This page is held in the app shell's frame.
export const shellHeld = !!parentShell;

// At the top of the window: whether this page becomes the app shell (it then has, and the caller stops).
export function shellTakes(script) {
  const want = script?.dataset?.shell;
  if (!want || shellHeld || window.top !== window) return false;
  if (want !== "page" && !(want === "edit" && new URLSearchParams(location.search).has("edit"))) return false;
  // the page's own markup and look go; the frame shows the page as it is
  for (const el of [...document.body.children]) if (el.tagName !== "SCRIPT") el.remove();
  for (const el of document.head.querySelectorAll("style")) el.remove();
  const root = document.createElement("div"); // app shell mount
  root.id = "coaxial-shell";
  document.body.prepend(root);
  import(`/coaxial/dom-host.js?app=${SHELL}&root=${root.id}&top`).catch((error) => { console.error(error); document.documentElement.dataset.coaxialError = String(error); });
  return true;
}

// At the top of the window, in the page's own shell (dom-host.js `own`): the frame's three words, and the window's
// title as the held page's while one is held.
export function shellHost(own, root) {
  const title = document.querySelector("script[data-shell]") ? "OpenAgents" : document.title;
  const frame = () => root.querySelector("iframe.held");
  const shown = (win) => { const f = frame(); return !!f && f.contentWindow === win; };
  globalThis.coaxialShell = {
    go(url) { if (typeof url === "string" && url.length <= 320) own.send(15, 0, url); },
    step(win, text) { if (shown(win)) own.send(14, 0, String(text).slice(0, 60)); },
    drawn(win) { if (shown(win)) own.send(14, 1); },
  };
  const named = () => { const f = frame(); let t = ""; try { t = f?.contentDocument?.title || ""; } catch { /* another origin's */ } document.title = t || title; };
  own.after = named;
  root.addEventListener("load", (e) => {
    const f = e.target;
    if (!(f instanceof HTMLIFrameElement) || f !== frame()) return;
    named();
    let page = null;
    try { page = f.contentWindow; void page.location.href; } catch { page = null; }
    // a page that went somewhere by itself (a form, a script): the shell keeps that address or lets the window go
    if (!page) { own.send(14, 1); return; }
    if (page.location.protocol === "about:") return;   // the empty document a frame starts with
    const want = f.getAttribute("src"), at = page.location.pathname + page.location.search;
    if (want && at !== want) { own.send(15, 0, at + page.location.hash); return; }
    // a page with no glue of its own (a file, an error page) says nothing: it is drawn as it loads
    if (!page.coaxialHeld) own.send(14, 1);
  }, true);
}

// In the frame: an address this page goes to is the shell's. True when the shell has it.
export function shellGo(url) {
  if (!parentShell) return false;
  parentShell.go(String(url));
  return true;
}
let drawnSaid = false;
// In the frame: the page has drawn (or failed, which it then shows): the loading screen goes.
export function shellDrawn() {
  if (!parentShell || drawnSaid) return;
  drawnSaid = true;
  parentShell.drawn(window);
}
// In the frame: the page's own loading screen (loader.js, #coaxial-load) is the shell's to show: its words as they
// change, and drawn when it goes, is taken away or says it failed.
export function shellWatch(el) {
  if (!parentShell || !el) return;
  const say = () => {
    if (drawnSaid) { seen.disconnect(); return; }
    if (!el.isConnected || el.classList.contains("gone") || el.dataset.state === "fail") { seen.disconnect(); shellDrawn(); return; }
    parentShell.step(window, `${el.getAttribute("aria-valuenow") || "0"} ${el.querySelector(".lt")?.textContent || ""}`);
  };
  const seen = new MutationObserver(say);
  seen.observe(el, { attributes: true, attributeFilter: ["aria-valuenow", "class", "data-state"], subtree: true, characterData: true, childList: true });
  seen.observe(document.body, { childList: true });
  say();
}

// In the frame, once: a plain press on a link is the shell's (or, to another site, the window's); a link to a place
// on this page stays here.
if (parentShell && !globalThis.coaxialHeld) {
  globalThis.coaxialHeld = true;
  addEventListener("click", (e) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const a = e.target?.closest?.("a[href]");
    if (!a || (a.target && a.target !== "_self") || a.hasAttribute("download")) return;
    const to = new URL(a.getAttribute("href"), location.href);
    if (to.protocol !== "http:" && to.protocol !== "https:") return;
    if (to.origin === location.origin && to.pathname === location.pathname && to.search === location.search && to.hash) return;
    e.preventDefault();
    if (to.origin !== location.origin) { window.top.location.assign(to.href); return; }
    shellGo(to.pathname + to.search + to.hash);
  });
}
