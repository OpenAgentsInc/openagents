// Firm reason: 1 (browser glue: controlled clock, offline browser effects and read-only page observations for the Coax journey)
// Decisions, schedule and acceptance live in grid-journey.coax. This file only
// adapts the browser's effects and carries measurements back to that tool.
(() => {
  globalThis.coaxialClip = true;
  const fixed = 1793577600000, NativeDate = Date, fetchActual = fetch, SocketActual = WebSocket;
  globalThis.Date = class extends NativeDate {
    constructor(...args) { super(...(args.length ? args : [fixed])); }
    static now() { return fixed; }
  };
  const record = globalThis.gridJourneyEffects = { requests: [], sockets: [], exceptions: [], faults: [], consoleErrors: [] };
  const consoleError = console.error;
  console.error = (...args) => { record.consoleErrors.push(args.map(String)); return consoleError.apply(console, args); };
  addEventListener('error', event => record.exceptions.push(String(event.message)));
  addEventListener('unhandledrejection', event => record.exceptions.push(String(event.reason)));
  globalThis.fetch = (...args) => {
    const url = new URL(typeof args[0] === 'string' ? args[0] : args[0] instanceof URL ? args[0].href : args[0].url, location.href);
    record.requests.push(url.href);
    // The offline profile leaves external/API effects pending. It never
    // supplies invented server answers, authorizes a player or alters inputs.
    if (url.origin !== location.origin || url.pathname.startsWith('/api/')) return new Promise(() => {});
    return fetchActual(...args);
  };
  globalThis.WebSocket = class extends SocketActual {
    constructor(url, ...args) {
      record.sockets.push(String(url));
      if (new URL(url, location.href).host !== location.host) throw Error('offline Grid journey');
      super(url, ...args);
    }
  };
  globalThis.gridJourneyRead = () => ({
    url: location.pathname + location.search,
    golden: new URLSearchParams(location.search).has('golden') || globalThis.coaxialGolden === true,
    probe: Array.from({ length: 10 }, (_, field) => coaxial.gridProbe(field)),
    pose: Array.from({ length: 10 }, (_, field) => coaxial.gridPose(field)),
    status: coaxial.renderStatus, ready: coaxial.ready, sceneReady: coaxial.sceneReady,
    viewport: { innerWidth, innerHeight, dpr: devicePixelRatio,
      width: document.querySelector('canvas').width, height: document.querySelector('canvas').height,
      cssWidth: document.querySelector('canvas').clientWidth, cssHeight: document.querySelector('canvas').clientHeight },
  });
  globalThis.gridJourneyCaptureInfo = () => {
    const c = coaxial.renderCapture;
    const { bytes, uniform_bytes, vertex_bytes, draw_bytes, index_bytes, buffers, textures, ...meta } = c;
    return { ...meta,
      buffers: buffers.map(({ bytes, ...row }, index) => ({ ...row, file: `buffer-${index}.bin` })),
      textures: textures.map(({ bytes, ...row }, index) => ({ ...row, file: `texture-${index}.bin` })),
      resize_before: globalThis.gridJourneyBeforeResize, resize_after: gridJourneyRead() };
  };
  globalThis.gridJourneyCaptureBytes = (kind, index) => {
    const c = coaxial.renderCapture;
    const bytes = new Uint8Array(kind === 0 ? c.bytes : kind === 1 ? c.uniform_bytes : kind === 2 ? c.vertex_bytes :
      kind === 3 ? c.draw_bytes : kind === 4 ? c.index_bytes : kind === 5 ? c.buffers[index].bytes : c.textures[index].bytes);
    let text = '';
    for (let at = 0; at < bytes.length; at += 32768) text += String.fromCharCode(...bytes.subarray(at, at + 32768));
    return btoa(text);
  };
  import('/coaxial/render.js').then(({ RenderHost }) => {
    const frame = RenderHost.prototype.frame;
    RenderHost.prototype.frame = function (...args) {
      if (!globalThis.gridJourneyMissingSans) {
        if (globalThis.gridJourneyLastSans) record.recovery_same_texture = this.sansTexture === globalThis.gridJourneyLastSans;
        return frame.apply(this, args);
      }
      const saved = this.sansTexture, queue = this.device.queue;
      globalThis.gridJourneyLastSans = saved;
      const writeBuffer = queue.writeBuffer, submit = queue.submit;
      const calls = { writeBuffer: 0, submit: 0 };
      queue.writeBuffer = function (...values) { calls.writeBuffer++; return writeBuffer.apply(this, values); };
      queue.submit = function (...values) { calls.submit++; return submit.apply(this, values); };
      this.sansTexture = null;
      try { return frame.apply(this, args); }
      finally {
        this.sansTexture = saved; queue.writeBuffer = writeBuffer; queue.submit = submit;
        record.faults.push({ ...calls, refusal: this.lastRefusal });
      }
    };
    globalThis.gridJourneyGlueReady = true;
  });
})();
