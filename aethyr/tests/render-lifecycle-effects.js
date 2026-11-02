// Firm reason: 1 (headless browser WebAssembly/bitmap/GPU hooks and factual reads; Coax owns every sequence and verdict).
(() => {
  globalThis.coaxialClip = true;
  globalThis.renderLifecycleStep = rate => { globalThis.coaxialClipStep?.(1, rate); return true; };
  globalThis.renderLifecycleFixture = () => { coaxial.renderFixture(); return true; };
  const events = [];
  addEventListener('error', event => events.push({ type: 'error', message: event.message }));
  addEventListener('unhandledrejection', event => events.push({ type: 'unhandledrejection', message: String(event.reason) }));
  let engine, media;
  const jobs = Object.create(null);
  globalThis.renderLifecycleArmEngine = phase => {
    engine = { phase: 'idle', nativeInstantiations: 0, activeInstantiations: 0, overlapping: false };
    let release, hold = true;
    const gate = new Promise(done => release = done), compile = WebAssembly.compile, instantiate = WebAssembly.instantiate;
    engine.release = release;
    WebAssembly.compile = async (...args) => {
      const module = await compile(...args);
      if (phase === 'compile' && hold) { hold = false; engine.phase = 'held'; await gate; }
      return module;
    };
    WebAssembly.instantiate = async (...args) => {
      engine.activeInstantiations++;
      if (engine.activeInstantiations > 1) engine.overlapping = true;
      engine.nativeInstantiations++;
      const result = await instantiate(...args);
      if (phase === 'instantiate' && hold) { hold = false; engine.phase = 'held'; await gate; }
      engine.activeInstantiations--;
      return result;
    };
    return 'armed';
  };
  globalThis.renderLifecycleReload = name => {
    jobs[name] = { done: false, result: null };
    coaxial.reloadEngine().then(value => { jobs[name] = { done: true, result: { value } }; }, error => { jobs[name] = { done: true, result: { error: error.message } }; });
    return 'started';
  };
  globalThis.renderLifecycleReloadResult = name => jobs[name];
  globalThis.renderLifecycleReleaseEngine = () => { engine.release(); return 'released'; };
  globalThis.renderLifecycleEngineBefore = () => ({ marker: coaxial.renderEngineMarker(), binding: coaxial.renderBindingState() });
  globalThis.renderLifecycleEngineHeld = () => ({ phase: engine.phase, marker: coaxial.renderEngineMarker(), binding: coaxial.renderBindingState(), nativeInstantiations: engine.nativeInstantiations });
  globalThis.renderLifecycleEngineAfter = () => ({ marker: coaxial.renderEngineMarker(), binding: coaxial.renderBindingState(), nativeInstantiations: engine.nativeInstantiations, overlapping: engine.overlapping, render: coaxial.renderStatus });
  globalThis.renderLifecycleArmMedia = () => {
    let release;
    const gate = new Promise(done => release = done), original = fetch, bitmap = createImageBitmap, close = ImageBitmap.prototype.close;
    media = { release, decoded: 0, closed: 0, allocations: 0, textures: 0, object: null };
    globalThis.createImageBitmap = async (...args) => { const result = await bitmap(...args); media.decoded++; return result; };
    ImageBitmap.prototype.close = function () { media.closed++; return close.call(this); };
    globalThis.fetch = async (...args) => {
      const response = await original(...args), input = args[0];
      const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, location.href);
      if (!url.pathname.startsWith('/api/villa/memorial/fixture0/media/')) return response;
      const bytes = await response.arrayBuffer();
      await gate;
      return new Response(bytes, { status: response.status, headers: response.headers });
    };
    return 'armed';
  };
  globalThis.renderLifecycleMediaWaiting = () => {
    media.object = coaxial.renderMediaObjects()[0];
    return { present: !!media.object, ready: !!media.object?.ready, decoded: media.decoded };
  };
  globalThis.renderLifecycleRetire = () => { coaxial.retireRenderer(); return { retired: true }; };
  globalThis.renderLifecycleReleaseMedia = async () => {
    const { RenderHost } = await import('/coaxial/render.js'), allocate = RenderHost.prototype.mediaAllocate, create = GPUDevice.prototype.createTexture;
    RenderHost.prototype.mediaAllocate = function (...args) { media.allocations++; return allocate.apply(this, args); };
    GPUDevice.prototype.createTexture = function (...args) { media.textures++; return create.apply(this, args); };
    media.release();
    return 'released';
  };
  globalThis.renderLifecycleMediaAfter = () => ({ decoded: media.decoded, closed: media.closed, allocations: media.allocations, textures: media.textures, ready: media.object.ready, hasTexture: !!media.object.texture, current: coaxial.renderMediaObjects().length, binding: coaxial.renderBindingState() });
  globalThis.renderLifecycleEvents = () => events;
  globalThis.renderLifecycleValidation = async () => {
    const value = await coaxial.renderSettled();
    return { defined: value !== undefined, value: value === undefined ? null : value };
  };
  return 'ready';
})()
