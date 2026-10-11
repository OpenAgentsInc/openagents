// Load generated Rust browser code. Requests and streams belong to HTMX,
// except the one fetch below that loads a main surface to move in place,
// which is parsed inert and swapped through HTMX.
import init, { start } from '/chat/assets/coder_chat_web.js';

// The new chat (`/`) and a chat's page (`/chat/{id}`) share one <head>, so
// moving between them is boosted (`hx-boost` on <body>): HTMX swaps the body
// and pushes the URL, and the left panel's list and the page's live stream
// come back with it. Every other address of this site has its own <head>
// (styles, scripts, policy), so a boosted link or form there loads in full.
const FAMILY = /^\/(chat(\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})?)?$/;
const VERBS = ['hx-get', 'hx-post', 'hx-put', 'hx-patch', 'hx-delete'];

function inFamily(path) {
  try {
    const url = new URL(path, location.href);
    return url.origin === location.origin && FAMILY.test(url.pathname);
  } catch {
    return false;
  }
}

document.addEventListener('htmx:confirm', (event) => {
  const { elt, path, triggeringEvent } = event.detail;
  if (!(elt instanceof Element) || !elt.closest('[hx-boost="true"]')) return;
  if (VERBS.some((verb) => elt.hasAttribute(verb))) return;
  if (elt.tagName !== 'A' && elt.tagName !== 'FORM') return;
  if (inFamily(path)) return;
  event.preventDefault();
  if (elt.tagName === 'A') {
    location.assign(elt.href);
    return;
  }
  // A plain submit, keeping the button that sent it.
  const submitter = triggeringEvent && triggeringEvent.submitter;
  if (submitter && submitter.name) {
    const field = document.createElement('input');
    field.type = 'hidden';
    field.name = submitter.name;
    field.value = submitter.value;
    elt.appendChild(field);
  }
  HTMLFormElement.prototype.submit.call(elt);
});

// Moving in place (#11322). "New chat" (and Control+N), the wordmark, and
// a chat's row swap only the main surface (.oa-main-surface: header,
// thread, composer). The left panel stays the same element, with its
// scroll, its folds and its live stream, and no script runs again.
// - The new chat is loaded ahead of the click (X-OA-Part: prefetch, idle
//   after load, again on hover when it is older than a minute, and after
//   every send), so "New chat" paints from memory with no request: a
//   fresh request id, the URL pushed, the composer focused.
// - A chat's page is fetched with X-OA-Part: main (the server leaves the
//   sidebar's list out), in place of a row's own hx-get, so every move
//   pushes an entry this script can take back.
// - Back and forward between addresses moved to here swap the same way.
// Anything unexpected (no HTMX, a refused or redirected answer) loads the
// address in full. Without JavaScript these are plain links.
const MOVE = /^\/(chat\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})?$/;
const FRESH_MS = 60_000;
let ahead = null; // { html, title, path, at }: the new chat, loaded ahead
let loading = null;
let moves = 0;
let shown = location.pathname + location.search;

function movable(url) {
  return url.origin === location.origin && MOVE.test(url.pathname) && !url.hash;
}

async function fetchMain(href, part) {
  const response = await fetch(href, {
    headers: { 'X-OA-Part': part },
    credentials: 'same-origin',
  });
  const final = new URL(response.url || href, location.href);
  if (!response.ok || !movable(final)) throw new Error('load in full');
  const doc = new DOMParser().parseFromString(await response.text(), 'text/html');
  const surface = doc.querySelector('.oa-main-surface');
  if (!surface) throw new Error('load in full');
  return { html: surface.outerHTML, title: doc.title, path: final.pathname + final.search, at: Date.now() };
}

function loadAhead() {
  if (!loading) {
    loading = fetchMain('/', 'prefetch')
      .then((part) => { ahead = part; return part; })
      .finally(() => { loading = null; });
    loading.catch(() => {});
  }
  return loading;
}

function soon(task) {
  if (window.requestIdleCallback) window.requestIdleCallback(task, { timeout: 2000 });
  else setTimeout(task, 300);
}

function markCurrent(path) {
  const panel = document.getElementById('oa-left-panel');
  if (!panel) return;
  for (const link of panel.querySelectorAll('a.oa-nav-item[aria-current]')) {
    link.removeAttribute('aria-current');
  }
  for (const link of panel.querySelectorAll('a.oa-nav-item[href]')) {
    const url = new URL(link.href, location.href);
    if (url.pathname + url.search === path) {
      link.setAttribute('aria-current', 'page');
      break;
    }
  }
}

function closeDrawer() {
  const panel = document.getElementById('oa-left-panel');
  try {
    if (panel && panel.matches(':popover-open')) panel.hidePopover();
  } catch { /* no popover support: nothing is open */ }
}

function show(part, push) {
  const surface = document.querySelector('.oa-main-surface');
  if (!surface || !window.htmx) {
    location.assign(part.path);
    return;
  }
  if (push) history.pushState({ oaMove: true }, '', part.path);
  shown = part.path;
  if (part.title) document.title = part.title;
  window.htmx.swap(surface, part.html, { swapStyle: 'outerHTML', swapDelay: 0, settleDelay: 0 });
  // A new chat never reuses the id of one loaded before.
  if (part.path === '/' && window.crypto && crypto.randomUUID) {
    for (const field of document.querySelectorAll('input[name="request_id"]')) {
      field.value = crypto.randomUUID();
    }
  }
  const input = document.querySelector('.oa-main-surface form[data-oa-composer] textarea');
  if (input && !input.disabled) input.focus({ preventScroll: true });
}

async function move(url, push) {
  const path = url.pathname + url.search;
  const turn = ++moves;
  closeDrawer();
  markCurrent(path);
  let part;
  if (path === '/' && ahead) {
    part = ahead;
    ahead = null;
  } else {
    try {
      part = path === '/' ? await (loading || fetchMain(url.href, 'main')) : await fetchMain(url.href, 'main');
      if (path === '/') ahead = null;
    } catch {
      if (turn === moves) location.assign(url.href);
      return;
    }
  }
  if (turn !== moves) return;
  show(part, push);
  if (path === '/' || !ahead) soon(loadAhead);
}

document.addEventListener('click', (event) => {
  if (event.defaultPrevented || event.button !== 0) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const link = event.target instanceof Element ? event.target.closest('a[href]') : null;
  if (!link || (link.target && link.target !== '_self') || link.hasAttribute('download')) return;
  if (link.closest('[hx-boost="false"]') || !document.querySelector('.oa-main-surface')) return;
  const url = new URL(link.href, location.href);
  if (!movable(url)) return;
  if (['hx-post', 'hx-put', 'hx-patch', 'hx-delete'].some((verb) => link.hasAttribute(verb))) return;
  event.preventDefault();
  event.stopPropagation();
  move(url, url.pathname + url.search !== shown);
}, true);

window.addEventListener('popstate', (event) => {
  // HTMX restores the entries its own boosted swaps pushed.
  if (event.state && event.state.htmx) return;
  const url = new URL(location.href);
  if (url.pathname + url.search === shown) return;
  if (!movable(url)) return;
  move(url, false);
});

// HTMX's own swaps (a sent message, a boosted page) change what is shown.
document.addEventListener('htmx:pushedIntoHistory', () => { shown = location.pathname + location.search; });
document.addEventListener('htmx:historyRestore', () => { shown = location.pathname + location.search; });
// After a send, the starters and picks may have changed: load again.
document.addEventListener('htmx:afterRequest', (event) => {
  const elt = event.detail && event.detail.elt;
  if (elt && elt.hasAttribute && elt.hasAttribute('data-oa-composer') && event.detail.successful) {
    ahead = null;
    setTimeout(() => soon(loadAhead), 1000);
  }
});
document.addEventListener('pointerover', (event) => {
  const link = event.target instanceof Element ? event.target.closest('a[href="/"]') : null;
  if (link && (!ahead || Date.now() - ahead.at > FRESH_MS)) loadAhead();
});
if (document.querySelector('.oa-main-surface') && document.querySelector('a.oa-nav-item[href="/"]')) {
  soon(loadAhead);
}

await init();
start();
// After a boosted swap, or a move in place, the composer is a new
// element: bind it.
for (const name of ['htmx:afterSettle', 'htmx:historyRestore']) {
  document.addEventListener(name, () => start());
}
