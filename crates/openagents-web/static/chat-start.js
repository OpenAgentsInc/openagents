// Load generated Rust browser code. Requests and streams belong to HTMX.
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

await init();
start();
// After a boosted swap the composer is a new element: bind it.
for (const name of ['htmx:afterSettle', 'htmx:historyRestore']) {
  document.addEventListener(name, () => start());
}
