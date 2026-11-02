// Firm reason: 1 (DOM, WebGPU, MediaRecorder, retained browser objects and HTTP byte effects; Coax owns order, time budgets, filenames and verdicts).
(() => {
  const state = { done: true, error: null, value: null, events: [], slots: [], streams: [], capture: null, projected: null };
  const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(b => b.toString(16).padStart(2, '0')).join('');
  addEventListener('error', e => state.events.push({ kind: 'error', message: String(e.message) }));
  addEventListener('unhandledrejection', e => state.events.push({ kind: 'unhandledrejection', message: String(e.reason?.stack || e.reason) }));
  const slot = async (name, value) => {
    const bytes = value instanceof Uint8Array ? value : new Uint8Array(value ?? []), index = state.slots.length;
    state.slots.push(bytes); return { slot: index, name, bytes: bytes.byteLength, sha256: await digest(bytes) };
  };
  const project = async capture => {
    if (!capture) return capture;
    if (state.capture === capture) return state.projected;
    const result = { ...capture }, files = [];
    for (const [field, name] of [['bytes', 'gpu.bin'], ['uniform_bytes', 'u.bin'], ['vertex_bytes', 'hud.bin'], ['draw_bytes', 'draws.bin'], ['index_bytes', 'indices.bin']]) {
      files.push(await slot(name, capture[field])); delete result[field];
    }
    result.buffers = [];
    for (const buffer of capture.buffers ?? []) { const row = { ...buffer, file: buffer.name }; files.push(await slot(buffer.name, buffer.bytes)); delete row.bytes; result.buffers.push(row); }
    result.textures = [];
    for (const [index, texture] of (capture.textures ?? []).entries()) {
      const file = texture.file ?? state.textureNames[index], row = { ...texture, file };
      if (texture.bytes !== undefined) files.push(await slot(file, texture.bytes));
      delete row.bytes; result.textures.push(row);
    }
    state.capture = capture; state.projected = result; state.files = files; return result;
  };
  const effects = {
    step: p => globalThis.coaxialClipStep?.(p.count, p.infinite ? Infinity : p.fps),
    status: () => !!globalThis.coaxial?.renderStatus,
    capture: () => globalThis.coaxial.captureFrame(),
    snapshot: async () => { const value = { capture: globalThis.coaxial?.renderCapture, diag: document.querySelector('#coaxial-diag')?.textContent, loader: document.querySelector('#coaxial-load')?.textContent, render: globalThis.coaxial?.renderStatus, ready: globalThis.coaxial?.sceneReady, engineReady: globalThis.coaxial?.ready, search: location.search, physical: globalThis.coaxialCaptureDeviceObservation, media: globalThis.coaxial?.renderMediaState?.(), body: document.body.innerText.slice(-1200) }; value.capture = await project(value.capture); return value; },
    readyMedia: () => coaxial.renderMediaObjects().some(o => o.ready),
    fixture: p => p.video ? coaxial.renderFixture('video') : coaxial.renderFixture(),
    videoStart: () => {
      const canvas = document.createElement('canvas'); canvas.width = 64; canvas.height = 32;
      const context = canvas.getContext('2d'); context.fillStyle = '#2178b4'; context.fillRect(0, 0, 64, 32);
      const stream = canvas.captureStream(0), recorder = new MediaRecorder(stream, { mimeType: 'video/webm;codecs=vp8' }), chunks = [];
      const finished = new Promise((resolve, reject) => { recorder.ondataavailable = e => chunks.push(e.data); recorder.onstop = resolve; recorder.onerror = reject; });
      state.video = { stream, recorder, chunks, finished }; recorder.start(); stream.getVideoTracks()[0].requestFrame();
    },
    videoStop: async () => {
      const { stream, recorder, chunks, finished } = state.video; recorder.stop(); await finished;
      const bytes = new Uint8Array(await new Blob(chunks, { type: recorder.mimeType }).arrayBuffer()), audio_tracks = stream.getAudioTracks().length;
      stream.getTracks().forEach(t => t.stop()); state.videoFile = await slot('fixture.webm', bytes);
      return { mime: recorder.mimeType, width: 64, height: 32, audio_tracks, bytes: bytes.byteLength, sha256: state.videoFile.sha256, transport: state.videoFile };
    },
    videoObserved: () => coaxial.renderMediaObjects().map(o => ({ kind: o.kind, ready: o.ready, width: o.width, height: o.height, muted: o.element?.muted, defaultMuted: o.element?.defaultMuted, videoWidth: o.element?.videoWidth, videoHeight: o.element?.videoHeight })),
    pendingWaiting: p => {
      globalThis.heldAsset = coaxial.renderAssets().find(a => p.kind === 'pack' ? a.pack : a.kind === p.kind);
      globalThis.heldParts = heldAsset.pack ? coaxial.renderAssets().filter(a => a.pack === heldAsset.pack) : [heldAsset];
      return { done: !!heldAsset.done, fetching: !!(heldAsset.fetching || heldAsset.pack?.fetching), parts: heldParts.length };
    },
    pendingAssets: () => (coaxial.renderAssets() ?? []).map(a => ({ kind: a.kind, pack: !!a.pack, done: !!a.done, fetching: !!(a.fetching || a.pack?.fetching) })),
    reloadEngine: () => coaxial.reloadEngine(),
    pendingSame: () => heldParts.every(a => coaxial.renderAssets().includes(a)),
    pendingRelease: () => globalThis.coaxialReleaseAsset(),
    pendingDone: () => heldParts.every(a => a.done),
    pendingReady: () => { globalThis.retainedLayer = heldParts.find(a => a.kind === 'layers')?.texture; return { done: heldParts.every(a => a.done), bytes: heldAsset.bytes?.length ?? 0, size: heldAsset.size ?? 0, texture: !!retainedLayer, failed: heldParts.some(a => a.failed) }; },
    settled: () => coaxial.renderSettled(),
    pendingFinal: () => ({ same: heldParts.every(a => coaxial.renderAssets().includes(a)), done: heldParts.every(a => a.done), bytes: heldAsset.bytes?.length ?? 0, textureSame: heldParts.find(a => a.kind === 'layers')?.texture === retainedLayer, binding: coaxial.renderBindingState(), render: coaxial.renderStatus }),
    mediaBefore: async () => {
      const { RenderHost } = await import('/coaxial/render.js'), original = RenderHost.prototype.makeGroups; globalThis.mediaReloadBindings = [];
      RenderHost.prototype.makeGroups = function(index, operation, media) { const result = original.call(this, index, operation, media); if (index === 13) mediaReloadBindings.push({ object: media.object, texture: media.object.texture }); return result; };
      globalThis.originalReadyMedia = coaxial.renderMediaObjects().find(o => o.ready); globalThis.originalReadyTexture = originalReadyMedia?.texture;
      return { ready: !!originalReadyMedia, kind: originalReadyMedia?.kind, width: originalReadyMedia?.width, height: originalReadyMedia?.height };
    },
    reloadRenderer: () => coaxial.reloadRenderer(),
    mediaShader: () => ({ same: coaxial.renderMediaObjects().includes(originalReadyMedia), textureSame: coaxial.renderMediaObjects().find(o => o.ready)?.texture === originalReadyTexture, bound: mediaReloadBindings.some(b => b.texture === originalReadyTexture), render: coaxial.renderStatus }),
    mediaClear: () => { mediaReloadBindings = []; },
    mediaEngine: () => {
      globalThis.currentReadyMedia = coaxial.renderMediaObjects().find(o => o.ready);
      return { ready: !!currentReadyMedia, kind: currentReadyMedia?.kind, width: currentReadyMedia?.width, height: currentReadyMedia?.height, bound: mediaReloadBindings.some(b => b.object === currentReadyMedia && b.texture === currentReadyMedia?.texture), oldClosed: originalReadyMedia.source === null || originalReadyMedia.element?.paused === true, coherent: coaxial.renderBindingState().coherent, render: coaxial.renderStatus };
    },
    recapture: () => { globalThis.mediaReloadPriorCapture = coaxial.renderCapture; coaxial.captureFrame(); },
    recaptured: () => coaxial.renderCapture !== mediaReloadPriorCapture,
    mediaFresh: async () => { const value = { capture: coaxial.renderCapture, render: coaxial.renderStatus, media: coaxial.renderMediaState() }; value.capture = await project(value.capture); return value; },
    mediaRetire: async () => {
      const { RenderHost } = await import('/coaxial/render.js'); globalThis.mediaReloadLateAllocations = 0; const allocate = RenderHost.prototype.mediaAllocate;
      RenderHost.prototype.mediaAllocate = function(...args) { mediaReloadLateAllocations++; return allocate.apply(this, args); };
      coaxial.retireRenderer(); currentReadyMedia.element?.dispatchEvent(new Event('loadeddata')); await Promise.resolve();
      return { binding: coaxial.renderBindingState(), cache: coaxial.renderMediaObjects().length, lateAllocations: mediaReloadLateAllocations, closed: currentReadyMedia.source === null || currentReadyMedia.element?.paused === true };
    },
    reloadInitial: async () => { globalThis.renderTestResources = coaxial.renderResources(); state.pipelineCalls = 0; state.pipelineBlocks = 0; state.pipelineDevices = []; state.pipelineOwned = new WeakMap(); const { RenderHost } = await import('/coaxial/render.js'), make = RenderHost.prototype.makePipeline; RenderHost.prototype.makePipeline = function(entry) { const device = this.device; if (!state.pipelineOwned.has(device)) { const original = device.createRenderPipelineAsync, hook = function(d) { state.pipelineCalls++; if (globalThis.renderTestFailPipeline) { state.pipelineBlocks++; return Promise.reject(Error('planted pipeline creation failure')); } return original.call(this, d); }; state.pipelineOwned.set(device, hook); device.createRenderPipelineAsync = hook; } state.pipelineDevices.push({ entry: entry.entry_index, own: Object.prototype.hasOwnProperty.call(device, 'createRenderPipelineAsync'), matches: device.createRenderPipelineAsync === state.pipelineOwned.get(device), prototype: Object.getPrototypeOf(device) === GPUDevice.prototype, name: device.createRenderPipelineAsync.name }); return make.call(this, entry); }; return coaxial.renderBank(); },
    pipelineHook: () => ({ calls: state.pipelineCalls, blocks: state.pipelineBlocks, devices: state.pipelineDevices, flag: globalThis.renderTestFailPipeline, writable: Object.getOwnPropertyDescriptor(GPUDevice.prototype, 'createRenderPipelineAsync')?.writable }),
    pipelineFailure: p => { globalThis.renderTestFailPipeline = p.enabled; },
    reloadObserved: () => ({ bank: coaxial.renderBank(), same: (() => { const a = coaxial.renderResources(); return a.length === renderTestResources.length && a.every((v, i) => v === renderTestResources[i]); })(), render: coaxial.renderStatus }),
    mediaStage: () => {
      globalThis.mediaQueueCalls = { submit: 0, writeBuffer: 0, copyExternalImageToTexture: 0 };
      for (const name of Object.keys(mediaQueueCalls)) { const original = GPUQueue.prototype[name]; GPUQueue.prototype[name] = function(...args) { mediaQueueCalls[name]++; return original.apply(this, args); }; }
      const m = coaxial.renderMediaObjects()[0]; globalThis.mediaSavedWidth = m.width; m.width++; return coaxial.renderUploadState(true);
    },
    mediaUpload: () => ({ status: coaxial.renderStatus, pending: coaxial.renderUploadState(), calls: { ...mediaQueueCalls } }),
    mediaRestore: () => { coaxial.renderMediaObjects()[0].width = mediaSavedWidth; },
    parallelReload: () => Promise.all([coaxial.reloadRenderer(), coaxial.reloadRenderer()]),
    engineState: () => ({ render: coaxial.renderStatus, bank: coaxial.renderBank() }),
    engineFailure: () => coaxial.reloadEngine().then(() => ({ failed: false }), error => ({ failed: true, message: String(error.message) })),
    binding: () => coaxial.renderBindingState(),
    files: () => state.files,
    events: () => state.events,
    dimensions: () => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio, canvas_width: document.querySelector('canvas')?.width, canvas_height: document.querySelector('canvas')?.height })
  };
  globalThis.renderHostBegin = (name, request) => {
    state.done = false; state.error = null; state.value = null;
    Promise.resolve().then(() => effects[name](request)).then(value => { state.value = value ?? null; state.done = true; }, error => { state.error = String(error.stack || error); state.done = true; }); return 'started';
  };
  globalThis.renderHostPoll = () => ({ done: state.done, error: state.error, value: state.value });
  globalThis.renderHostPost = async request => { const response = await fetch(request.url, { method: 'POST', body: state.slots[request.slot].slice(request.from, request.to) }); return { status: response.status, ack: await response.json() }; };
  globalThis.renderHostStream = async () => { const row = state.streams.find(row => !row.done); return row ? { index: row.index, ...row.file } : null; };
  globalThis.renderHostStreamAck = request => { const row = state.streams[request.index]; row.receipt.file = request.file; row.done = true; row.release(); return null; };
  globalThis.renderHostInstall = request => {
    state.textureNames = request.textureNames;
    if (request.start || request.half) globalThis.coaxialClip = true;
    if (request.half) globalThis.coaxialRenderCaptureFormat = 'rgba16float';
    if (request.pendingIndex || request.pendingLayer || request.pendingPack) {
      const original = fetch; let release, held = false; const gate = new Promise(r => release = r); globalThis.coaxialReleaseAsset = release;
      if (request.pendingLayer) { const bitmap = createImageBitmap; globalThis.createImageBitmap = async (...args) => { const image = await bitmap(...args); if (!held) { held = true; await gate; } return image; }; }
      globalThis.fetch = async (...args) => { const response = await original(...args), input = args[0], url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, location.href); if (!((request.pendingIndex && url.pathname.endsWith('Garden.indices')) || (request.pendingPack && url.pathname.endsWith('.pack')))) return response; const bytes = await response.arrayBuffer(); await gate; return new Response(bytes, { status: response.status, headers: response.headers }); };
    }
    if (request.textureStream) import('/coaxial/render.js').then(({ RenderHost }) => { Object.defineProperty(RenderHost.prototype, 'textureCaptureSink', { configurable: true, value: async (receipt, bytes) => { let release; const waiting = new Promise(r => release = r), index = state.streams.length, file = await slot('', bytes); state.streams.push({ index, receipt, file, release, done: false }); await waiting; } }); });
    if (request.physical) { const Native = ResizeObserver; globalThis.ResizeObserver = class extends Native { constructor(callback) { super((entries, observer) => callback(entries.map(entry => new Proxy(entry, { get(target, key) { if (key === 'devicePixelContentBoxSize') { globalThis.coaxialCaptureDeviceObservation = { kind: 'injected device-pixel-box', css_width: target.contentRect.width, css_height: target.contentRect.height, physical_width: request.pw, physical_height: request.ph, window_dpr: devicePixelRatio }; return [{ inlineSize: request.pw, blockSize: request.ph }]; } return Reflect.get(target, key, target); } })), observer)); } }; }
  };
  return 'ready';
})()
