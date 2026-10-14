// `/export` in the chat composer (#11615, `crate::chat_export`). Typed and
// sent, it runs here instead of going to the chat: the chat is saved as a
// JSON file and the file's path is copied. On this computer's own local
// site the server saves it in Downloads and answers with the path; on
// openagents.com the browser downloads it, and the path copied is where
// the Downloads folder usually is. Served from /chat/export.js (the page's
// policy allows no inline script).
(() => {
  'use strict';

  const COMMAND = '/export';
  const CHAT = /^\/chat\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/;
  let lastPath = '';

  const form = () => document.getElementById('chat-form');
  const input = () => document.getElementById('chat-input');
  const part = (f, name) => (f ? f.querySelector('[data-oa-export-' + name + ']') : null);

  function isCommand(text) {
    return text.trim() === COMMAND;
  }

  // The hint shows while the box holds the start of the command.
  function hint() {
    const f = form();
    const box = input();
    const shown = part(f, 'hint');
    if (!shown || !box) return;
    const text = box.value.trim();
    shown.hidden = !(text.startsWith('/') && COMMAND.startsWith(text) && text === box.value.trimStart());
  }

  function report(f, text, path, copied) {
    const result = part(f, 'result');
    if (!result) return;
    const words = result.querySelector('[data-oa-export-text]');
    const copy = part(f, 'copy');
    if (words) words.textContent = text;
    if (copy) copy.hidden = copied || !path;
    result.dataset.oaExportPath = path || '';
    result.hidden = false;
  }

  function nameFrom(response) {
    const disposition = response.headers.get('content-disposition') || '';
    const found = /filename="([^"]+)"/.exec(disposition);
    return found ? found[1] : 'openagents-chat.atif.json';
  }

  function clear(f) {
    const box = input();
    if (box) {
      box.value = '';
      box.dispatchEvent(new Event('input', { bubbles: true }));
    }
    const shown = part(f, 'hint');
    if (shown) shown.hidden = true;
  }

  // Runs inside the click or Enter that asked for it, so the browser lets
  // the page copy once the path is known.
  function run(f) {
    const found = CHAT.exec(location.pathname);
    clear(f);
    if (!found) {
      report(f, 'Send a message first; there is nothing to save yet.', '', true);
      return;
    }
    report(f, 'Saving this chat…', '', true);
    const csrf = f.elements.namedItem('csrf');
    const request = fetch('/chat/' + found[1] + '/export', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ csrf: csrf ? csrf.value : '' }),
    }).then(async (response) => {
      if (!response.ok) throw new Error((await response.text()) || 'We couldn’t save this chat.');
      if (response.headers.get('x-oa-export') === 'saved') {
        const saved = await response.json();
        return { saved: true, path: saved.path, name: saved.name };
      }
      const name = nameFrom(response);
      return { saved: false, path: '~/Downloads/' + name, name, blob: await response.blob() };
    });
    const path = request.then((done) => done.path);
    let copied;
    try {
      if (window.ClipboardItem && navigator.clipboard && navigator.clipboard.write) {
        copied = navigator.clipboard.write([
          new ClipboardItem({ 'text/plain': path.then((p) => new Blob([p], { type: 'text/plain' })) }),
        ]);
      } else {
        copied = path.then((p) => navigator.clipboard.writeText(p));
      }
    } catch (error) {
      copied = Promise.reject(error);
    }
    copied = copied.then(() => true, () => false);
    request.then(
      async (done) => {
        if (!done.saved) {
          const link = document.createElement('a');
          link.href = URL.createObjectURL(done.blob);
          link.download = done.name;
          document.body.appendChild(link);
          link.click();
          link.remove();
          setTimeout(() => URL.revokeObjectURL(link.href), 60000);
        }
        lastPath = done.path;
        const ok = await copied;
        const shown = '~/Downloads/' + done.name;
        let text;
        if (done.saved) {
          text = ok ? 'Saved ' + shown + ', path copied.' : 'Saved ' + done.path + '.';
        } else {
          text = ok
            ? 'Downloaded ' + done.name + ', path copied (assumes your Downloads folder).'
            : 'Downloaded ' + done.name + '. If it went to your Downloads folder, its path is ' + shown + '.';
        }
        report(f, text, done.path, ok);
      },
      (error) => report(f, error.message || 'We couldn’t save this chat.', '', true),
    );
  }

  // Before the page's own send (HTMX and the composer's adapter listen on
  // the form): the command never goes to the chat.
  document.addEventListener(
    'submit',
    (event) => {
      const f = event.target;
      const box = input();
      if (!(f instanceof HTMLFormElement) || f.id !== 'chat-form' || !box) return;
      if (!isCommand(box.value)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      run(f);
    },
    true,
  );

  document.addEventListener('input', (event) => {
    if (event.target && event.target.id === 'chat-input') hint();
  });

  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    const f = form();
    if (target.closest('[data-oa-export-run]') && f) {
      event.preventDefault();
      run(f);
      return;
    }
    if (target.closest('[data-oa-export-copy]') && f && lastPath) {
      event.preventDefault();
      navigator.clipboard.writeText(lastPath).then(
        () => report(f, 'Path copied: ' + lastPath, lastPath, true),
        () => report(f, 'Your browser didn’t allow copying. The path is ' + lastPath, lastPath, false),
      );
    }
  });
})();
