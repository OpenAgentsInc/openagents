// Files in the chat composer (#11174, `crate::chat_files`): pick them with
// the paperclip, paste them into the text box, or drop them on it. Each is
// uploaded to the chat's private files as soon as it is added; the send
// carries their ids in the form's `files` field. Served from /chat/files.js
// (the page's policy allows no inline script).
(() => {
  'use strict';

  const MAX_PER_MESSAGE = 4;
  const MAX_BYTES = 10 * 1024 * 1024;
  const CHAT = /^\/chat\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/;

  const form = () => document.getElementById('chat-form');
  const tray = () => {
    const f = form();
    return f ? f.querySelector('[data-oa-files]') : null;
  };

  // The chat the files belong to: the chat this page shows, or on the new
  // chat page the id the first message will give it.
  function chatId(f) {
    let path = '';
    try {
      path = new URL(f.getAttribute('action') || '', location.href).pathname;
    } catch {
      path = '';
    }
    const found = CHAT.exec(path);
    if (found) return found[1];
    const field = f.elements.namedItem('request_id');
    return field && 'value' in field ? field.value : '';
  }

  function token(f) {
    const field = f.elements.namedItem('csrf');
    return field && 'value' in field ? field.value : '';
  }

  // A plain message in the composer's status line, with a link when there
  // is something to do.
  function say(text, href, label) {
    const status = document.getElementById('chat-form-status');
    if (!status) return;
    status.textContent = text;
    if (href && label) {
      status.append(' ');
      const link = document.createElement('a');
      link.href = href;
      link.textContent = label;
      status.append(link);
    }
  }

  function chips(t) {
    return Array.from(t.querySelectorAll('[data-oa-file-id]'));
  }

  // Bring the hidden field, the empty state, and the privacy note up to date.
  function sync(t) {
    const added = chips(t);
    const field = t.querySelector('[data-oa-files-field]');
    if (field) {
      field.value = added
        .filter((chip) => chip.dataset.state === 'added')
        .map((chip) => chip.dataset.oaFileId)
        .join(',');
    }
    const any = added.length > 0;
    t.toggleAttribute('data-empty', !any);
    const note = t.querySelector('[data-oa-file-note]');
    if (note) note.hidden = !any;
  }

  function busy(t) {
    return chips(t).some((chip) => chip.dataset.state === 'adding');
  }

  function chip(name) {
    const item = document.createElement('li');
    item.className = 'oa-file-chip';
    item.dataset.state = 'adding';
    item.dataset.oaFileId = '';
    const label = document.createElement('span');
    label.className = 'oa-file-chip-name';
    label.textContent = name;
    label.title = name;
    const size = document.createElement('span');
    size.className = 'oa-file-chip-size';
    size.textContent = 'Adding…';
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.className = 'oa-file-chip-remove';
    remove.setAttribute('aria-label', `Remove ${name}`);
    remove.title = 'Remove';
    remove.textContent = '×';
    remove.dataset.oaFileRemove = '';
    item.append(label, size, remove);
    return item;
  }

  async function upload(file) {
    const f = form();
    const t = tray();
    if (!f || !t) return;
    if (chips(t).length >= MAX_PER_MESSAGE) {
      say('Send up to four files with one message.');
      return;
    }
    if (file.size > MAX_BYTES) {
      say('Files must be 10 MB or smaller.');
      return;
    }
    if (file.size === 0) {
      say('This file is empty.');
      return;
    }
    const chat = chatId(f);
    if (!chat) {
      say('Something went wrong. Reload this page.');
      return;
    }
    const name = file.name || (file.type.startsWith('image/') ? 'Pasted image' : 'File');
    const item = chip(name);
    t.querySelector('[data-oa-file-list]').append(item);
    item.dataset.oaFileId = `adding-${Math.random().toString(16).slice(2)}`;
    sync(t);
    say('');
    let body = null;
    let ok = false;
    try {
      const response = await fetch(
        `/chat/${chat}/files?name=${encodeURIComponent(name)}`,
        {
          method: 'POST',
          credentials: 'same-origin',
          headers: {
            'content-type': 'application/octet-stream',
            'x-openagents-csrf': token(f),
          },
          body: file,
        },
      );
      ok = response.ok;
      body = await response.json().catch(() => null);
    } catch {
      body = null;
    }
    if (!item.isConnected) {
      // Removed while it was adding: drop the copy that was kept.
      if (ok && body && body.id) forget(f, chat, body.id);
      return;
    }
    if (!ok || !body || !body.id) {
      item.remove();
      sync(t);
      const error = body && body.error ? body.error : "We couldn't add that file. Try again.";
      say(error, body && body.href, body && body.label);
      return;
    }
    item.dataset.state = 'added';
    item.dataset.oaFileId = body.id;
    item.querySelector('.oa-file-chip-size').textContent = body.size || '';
    if (['png', 'jpeg', 'gif', 'webp'].includes(body.kind) && body.url) {
      const picture = document.createElement('img');
      picture.src = body.url;
      picture.alt = '';
      item.prepend(picture);
    }
    sync(t);
  }

  function forget(f, chat, id) {
    fetch(`/chat/${chat}/files/${id}/delete`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'x-openagents-csrf': token(f) },
    }).catch(() => {});
  }

  function add(files) {
    for (const file of Array.from(files || [])) upload(file);
  }

  // Picked with the paperclip.
  document.addEventListener('change', (event) => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) || !input.hasAttribute('data-oa-file-input')) return;
    add(input.files);
    input.value = '';
  });

  // Removed from the row before sending.
  document.addEventListener('click', (event) => {
    const button = event.target instanceof Element
      ? event.target.closest('[data-oa-file-remove]')
      : null;
    if (!button) return;
    event.preventDefault();
    const item = button.closest('.oa-file-chip');
    const f = form();
    const t = tray();
    if (!item || !f || !t) return;
    if (item.dataset.state === 'added') forget(f, chatId(f), item.dataset.oaFileId);
    item.remove();
    sync(t);
  });

  // Pasted into the text box: files only; pasted words stay words.
  document.addEventListener('paste', (event) => {
    const f = form();
    if (!f || !(event.target instanceof Element) || !f.contains(event.target)) return;
    const data = event.clipboardData;
    if (!data || !data.files || data.files.length === 0) return;
    event.preventDefault();
    add(data.files);
  });

  // Dropped on the composer.
  const hasFiles = (event) =>
    event.dataTransfer && Array.from(event.dataTransfer.types || []).includes('Files');
  const body = (event) =>
    event.target instanceof Element ? event.target.closest('#chat-form [data-composer-body]') : null;
  document.addEventListener('dragover', (event) => {
    const target = body(event);
    if (!target || !hasFiles(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    target.setAttribute('data-oa-file-over', '');
  });
  document.addEventListener('dragleave', (event) => {
    const target = body(event);
    if (target && !target.contains(event.relatedTarget)) target.removeAttribute('data-oa-file-over');
  });
  document.addEventListener('drop', (event) => {
    const target = body(event);
    if (!target || !hasFiles(event)) return;
    event.preventDefault();
    target.removeAttribute('data-oa-file-over');
    add(event.dataTransfer.files);
  });

  // A send waits for files still being added.
  document.addEventListener(
    'submit',
    (event) => {
      const f = form();
      const t = tray();
      if (!f || event.target !== f || !t || !busy(t)) return;
      event.preventDefault();
      event.stopPropagation();
      say('Wait for your files to finish adding, then send.');
    },
    true,
  );

  // A sent message took the files: the row empties for the next one.
  document.addEventListener('htmx:afterRequest', (event) => {
    const detail = event.detail || {};
    const f = form();
    const t = tray();
    if (!f || !t || detail.elt !== f || !detail.successful) return;
    for (const item of chips(t)) if (item.dataset.state === 'added') item.remove();
    sync(t);
  });
})();
