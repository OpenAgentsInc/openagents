// Firm reason: 1 (headless WebGPU observation of actual instance spans and recovery).
(() => {
  globalThis.coaxialClip = true;
  globalThis.coaxialClipRoom = true;
  const proof = globalThis.grassProof = { frames: [], total: 0, errors: [], loss: false };
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async function (...args) {
    const result = await instantiate(...args);
    if (!result.exports?.coaxial_grass_instances) return result;
    const exports = { ...result.exports }, count = exports.coaxial_grass_instances;
    exports.coaxial_grass_instances = (...a) => (proof.total = count(...a));
    if ((globalThis.grassMode === 'legacy' || globalThis.grassMode === 'negative')) {
      delete exports.coaxial_grass_range_count_v1;
      delete exports.coaxial_grass_range_v1;
    }
    if (globalThis.grassMode === 'blend') exports.coaxial_draw_blend_v1 = () => 0;
    return { exports };
  };
  const error = console.error;
  console.error = (...args) => {
    const message = args.map(String).join(' ');
    if (!(proof.loss && message.startsWith('coaxial: GPU device lost (destroyed:'))) proof.errors.push(message);
    error(...args);
  };
  const names = new WeakMap(), current = new WeakMap(), frames = new WeakMap();
  const create = GPUDevice.prototype.createRenderPipelineAsync;
  GPUDevice.prototype.createRenderPipelineAsync = async function (descriptor) {
    const pipeline = await create.call(this, descriptor);
    names.set(pipeline, descriptor.fragment?.entryPoint);
    return pipeline;
  };
  const begin = GPUCommandEncoder.prototype.beginRenderPass;
  GPUCommandEncoder.prototype.beginRenderPass = function (...args) {
    const pass = begin.apply(this, args), frame = { order: [], grass: [] };
    frames.set(pass, frame);proof.frames.push(frame);
    return pass;
  };
  const set = GPURenderPassEncoder.prototype.setPipeline;
  GPURenderPassEncoder.prototype.setPipeline = function (pipeline) {
    current.set(this, names.get(pipeline));return set.call(this, pipeline);
  };
  for (const method of ['draw', 'drawIndexed']) {
    const draw = GPURenderPassEncoder.prototype[method];
    GPURenderPassEncoder.prototype[method] = function (...args) {
      const frame = frames.get(this), entry = current.get(this);
      if (frame) {
        frame.order.push(entry);
        if (entry === 'grass_fs') frame.grass.push(args);
      }
      return draw.apply(this, args);
    };
  }
  proof.ready = async () => {
    for (let i = 0; i < 2400; i++) {
      if (globalThis.coaxial?.ready) {
        await coaxial.hostFrameAt(0);
        if (coaxial.sceneReady && coaxial.character?.().status === 'ready') return true;
      }
      await new Promise(r => setTimeout(r, 100));
    }
    return false;
  };
  proof.check = async () => {
    proof.frames.length = 0;
    for (let i = 0; i < 3; i++) if (await coaxialClipStep(1, 60) !== 1) return false;
    const frames = proof.frames.filter(f => f.order.includes('grass_fs'));
    if (frames.length !== 3 || proof.errors.length) return false;
    for (const frame of frames) {
      const scene = frame.order.indexOf('scene_fs'), grass = frame.order.indexOf('grass_fs'), hero = frame.order.indexOf('character_fs');
      if (!(scene >= 0 && hero > grass && (globalThis.grassMode === 'blend' ? grass < scene : grass > frame.order.lastIndexOf('scene_fs')))) return false;
      let end = 0, count = 0;
      for (const [vertices, instances, first = 0, base = 0] of frame.grass) {
        if (vertices !== 3 || instances <= 0 || first !== 0 || base < end || base + instances > proof.total) return false;
        end = base + instances;count += instances;
      }
      if (globalThis.grassMode === 'legacy' ? count !== proof.total || frame.grass.length !== 1 : count <= 0 || count >= proof.total) return false;
    }
    const frame = frames.at(-1);
    proof.result = { mode: globalThis.grassMode, frames: frames.length, ranges: frame.grass.length, submitted: frame.grass.reduce((n, a) => n + a[1], 0), original: proof.total, errors: proof.errors };
    return true;
  };
  proof.recover = async () => {
    proof.loss = true;coaxial.loseDevice();
    for (let i = 0; i < 600; i++) {
      await new Promise(r => setTimeout(r, 100));
      try {await coaxialClipStep(1, 60);} catch (e) {if (coaxial.gpuRecovery?.state === 'failed') throw e;}
      if (coaxial.gpuRecovery?.state === 'recovered' && coaxial.sceneReady && coaxial.character?.().status === 'ready') {
        proof.loss = false;return await proof.check();
      }
      if (coaxial.gpuRecovery?.state === 'failed') return false;
    }
    return false;
  };
})();
