// Firm reason: 1 (headless browser WebGPU/WebCrypto effects and factual serialization only).
// Coax supplies one operation at a time and owns scheduling and every verdict.
(() => {
  let adapter, device;
  const uncaptured = [];
  const operate = async request => {
    if (request.operation === 'open') {
      adapter = await navigator.gpu.requestAdapter();
      if (!adapter) throw Error('no WebGPU adapter');
      device = await adapter.requestDevice();
      device.addEventListener('uncapturederror', event => uncaptured.push(event.error.message));
      const info = adapter.info;
      return { device: { vendor: info.vendor, architecture: info.architecture, device: info.device, description: info.description } };
    }
    if (request.operation === 'compile') {
      const code = new TextEncoder().encode(request.code);
      const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', code)), byte => byte.toString(16).padStart(2, '0')).join('');
      device.pushErrorScope('validation');
      const module = device.createShaderModule({ label: request.label, code: request.code });
      const info = await module.getCompilationInfo(), error = await device.popErrorScope();
      return { code_sha256: hash, messages: Array.from(info.messages, message => ({
        type: message.type, message: message.message, line: message.lineNum, column: message.linePos
      })), validation_error: error?.message ?? null };
    }
    if (request.operation === 'close') {
      device.destroy();
      return { uncaptured: Array.from(uncaptured) };
    }
    throw Error('unknown WebGPU effect operation');
  };
  globalThis.aethyrWgslModuleStart = request => {
    globalThis.aethyrWgslModuleJob = { done: false, error: null, value: null };
    operate(request).then(value => {
      globalThis.aethyrWgslModuleJob.value = value;
      globalThis.aethyrWgslModuleJob.done = true;
    }, error => {
      globalThis.aethyrWgslModuleJob.error = String(error?.stack ?? error);
      globalThis.aethyrWgslModuleJob.done = true;
    });
    return 'started';
  };
  globalThis.aethyrWgslModulePoll = () => globalThis.aethyrWgslModuleJob;
  return 'ready';
})()
