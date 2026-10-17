// Transport for the shared player. Edits and timeline timing stay in Rust.
(() => {
  const config = window.videoLabConfig;
  const enc = encodeURIComponent;
  const scope = 'space=' + enc(config.space);
  const root = '/v1/labs';
  const doc = id => root + '/documents/' + enc(id);
  const file = id => root + '/artifacts/' + enc(id) + '/content?' + scope;
  const renders = new Map();
  const previews = new Map();
  const histories = new Map();
  let closed = false;
  function clear() {
    closed = true;
    config.closed = true;
    clearTimeout(poll);
    state.doc = state.data = state.timeline = state.selected = null;
    state.renderVersion = null;
    state.t = 0;
    const title = document.getElementById('title');
    if (title) title.textContent = 'Video Lab';
    renders.clear();
    previews.clear();
    histories.clear();
    document.querySelectorAll('video').forEach(video => { video.pause(); video.removeAttribute('src'); video.load(); });
    for (const id of ['stage', 'tracks', 'inspector', 'versions', 'docs', 'notes']) document.getElementById(id)?.replaceChildren();
    document.querySelectorAll('button,input,textarea,select').forEach(control => {
      if ('value' in control) control.value = '';
      control.disabled = true;
    });
  }
  async function request(path, body, key) {
    if (closed) throw Error('Open the film again to continue.');
    const headers = body ? { 'Content-Type': 'application/json' } : {};
    if (key) headers['Idempotency-Key'] = key;
    const response = await fetch(path + (path.includes('?') ? '&' : '?') + scope, {
      method: body ? 'POST' : 'GET', credentials: 'same-origin', cache: 'no-store', headers,
      ...(body ? { body: JSON.stringify(body) } : {})
    });
    const value = await response.json();
    if (closed) throw Error('Open the film again to continue.');
    if (!response.ok) {
      if ([401, 403, 404].includes(response.status)) clear();
      const error = Error(value.error?.message || value.refused?.text || value.text || 'The request could not be completed.');
      error.status = response.status;
      throw error;
    }
    return value;
  }
  function brief(version) {
    return { n: version.number, summary: version.summary, author: version.author,
      created: version.created_unix_ms / 1000, renders: {} };
  }
  async function card(id, version) {
    const value = await request(doc(id) + '/versions/' + version + '/card');
    renders.set(id + ':' + version, value);
    return value;
  }
  window.videoLabApi = async (path, body) => {
    const url = new URL(path, location.origin);
    const id = body?.doc || body?.document || url.searchParams.get('doc');
    if (url.pathname === '/api/docs') {
      const after = Number(url.searchParams.get('after') || 0);
      const page = await request(root + '/documents?kind=video&limit=100&after=' + after);
      return { documents: page.data.map(value => ({ id: value.id, title: value.title, versions: value.latest })), next: page.next };
    }
    if (url.pathname === '/api/older') {
      const history = histories.get(id);
      if (!history || history.oldest <= 1) return {};
      const page = await request(doc(id) + '/versions?limit=100&after=' + Math.max(0, history.oldest - 101));
      for (const value of page.data) history.versions.set(value.number, brief(value));
      history.oldest = page.data.length ? Math.min(...page.data.map(v => v.number), history.oldest) : 1;
      return {};
    }
    if (url.pathname === '/api/doc') {
      const selectedVersion = state.v, comparison = state.cmp;
      const meta = await request(doc(id));
      const page = await request(doc(id) + '/versions?limit=100&after=' + Math.max(0, meta.latest - 100));
      let history = histories.get(id);
      if (!history) { history = { versions: new Map(), oldest: meta.latest }; histories.set(id, history); }
      for (const value of page.data) history.versions.set(value.number, brief(value));
      history.oldest = Math.min(history.oldest, ...page.data.map(v => v.number));
      const versions = [...history.versions.values()];
      const selected = selectedVersion || meta.latest;
      const wanted = [...new Set([selected, Number(comparison)].filter(n => Number.isSafeInteger(n) && n > 0))];
      for (const number of wanted) {
        if (!versions.some(v => v.n === number)) versions.push(brief(await request(doc(id) + '/versions/' + number)));
        const value = await card(id, number);
        const entry = versions.find(v => v.n === number);
        entry.renders = value.primary ? { preview: file(value.primary) } : {};
        entry.job = { state: value.state };
      }
      versions.sort((a, b) => a.n - b.n);
      const notes = await request(doc(id) + '/notes?limit=100');
      return { doc: { ...meta, kind: meta.schema }, versions, older: history.oldest > 1,
        notes: notes.data.filter(n => n.locator.at === 'time').map(n => ({ t: n.locator.seconds, version: n.subject.version, text: n.text })) };
    }
    if (url.pathname === '/api/timeline' && !body) {
      if (state.data?.doc.kind !== 'cut.v1') return null;
      const number = Number(url.searchParams.get('v'));
      const value = await request(doc(id) + '/versions/' + number + '/timeline');
      const preview = renders.get(id + ':' + number);
      for (const clip of value.clips) clip.thumbnail = clip.track === 'video' && preview?.poster ? file(preview.poster) : null;
      return value;
    }
    if (url.pathname === '/api/timeline') {
      const saved = await request(doc(id) + '/versions', { space: config.space, base: body.version,
        change: { type: 'intent', intent: { intent: 'timeline', command: body.command } }, summary: body.summary });
      return { version: saved.number };
    }
    if (url.pathname === '/api/tool/lab_render') {
      const version = { document: id, version: body.version };
      const subject = id + ':' + body.version;
      let key = previews.get(subject);
      if (!key) { key = 'web-preview-' + crypto.randomUUID(); previews.set(subject, key); }
      const job = await request(root + '/jobs', { space: config.space, version,
        want: { want: 'job', type: 'render', params: { quality: 'preview' } } }, key);
      // A received result is conclusive; a lost response keeps the same key.
      previews.delete(subject);
      return job;
    }
    if (url.pathname === '/api/note') {
      return request(doc(id) + '/notes', { space: config.space, version: body.v,
        locator: { at: 'time', seconds: body.t }, text: body.text });
    }
    throw Error('This action is not available.');
  };
})();
