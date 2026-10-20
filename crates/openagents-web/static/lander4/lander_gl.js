let wasm_bindgen = (function(exports) {
let script_src;
if (typeof document !== 'undefined' && document.currentScript !== null) {
script_src = new URL(document.currentScript.src, location.href).toString();
}
function start() {
wasm.start();
}
exports.start = start;
function __wbg_get_imports() {
const import0 = {
__proto__: null,
__wbg___wbindgen_boolean_get_7a12af2b3f899c5a: function(arg0) {
const v = getObject(arg0);
const ret = typeof(v) === 'boolean' ? v : undefined;
return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
},
__wbg___wbindgen_is_undefined_8c687d0b90d5b524: function(arg0) {
const ret = getObject(arg0) === undefined;
return ret;
},
__wbg___wbindgen_number_get_1dc732b810cb937c: function(arg0, arg1) {
const obj = getObject(arg1);
const ret = typeof(obj) === 'number' ? obj : undefined;
getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
},
__wbg___wbindgen_throw_5d9e815e6fdf150f: function(arg0, arg1) {
throw new Error(getStringFromWasm0(arg0, arg1));
},
__wbg__wbg_cb_unref_997e73d32238e655: function(arg0) {
getObject(arg0)._wbg_cb_unref();
},
__wbg_addEventListener_9534e2dc12044ee4: function() { return handleError(function (arg0, arg1, arg2, arg3) {
getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3));
}, arguments); },
__wbg_add_8ead3105ef6d0bda: function() { return handleError(function (arg0, arg1, arg2) {
getObject(arg0).add(getStringFromWasm0(arg1, arg2));
}, arguments); },
__wbg_attachShader_74ecf95efc73ee58: function(arg0, arg1, arg2) {
getObject(arg0).attachShader(getObject(arg1), getObject(arg2));
},
__wbg_bindAttribLocation_601c8f14be6021f1: function(arg0, arg1, arg2, arg3, arg4) {
getObject(arg0).bindAttribLocation(getObject(arg1), arg2 >>> 0, getStringFromWasm0(arg3, arg4));
},
__wbg_bindBuffer_f1f33e087b91687c: function(arg0, arg1, arg2) {
getObject(arg0).bindBuffer(arg1 >>> 0, getObject(arg2));
},
__wbg_bindTexture_e8a625bf5d240b0f: function(arg0, arg1, arg2) {
getObject(arg0).bindTexture(arg1 >>> 0, getObject(arg2));
},
__wbg_bindVertexArray_4687bb1e8b327fc9: function(arg0, arg1) {
getObject(arg0).bindVertexArray(getObject(arg1));
},
__wbg_blendFunc_ad7d043180d4f86a: function(arg0, arg1, arg2) {
getObject(arg0).blendFunc(arg1 >>> 0, arg2 >>> 0);
},
__wbg_bufferData_759d1c6ffcab5af1: function(arg0, arg1, arg2, arg3, arg4) {
getObject(arg0).bufferData(arg1 >>> 0, getArrayU8FromWasm0(arg2, arg3), arg4 >>> 0);
},
__wbg_classList_1501fdb96a8c296c: function(arg0) {
const ret = getObject(arg0).classList;
return addHeapObject(ret);
},
__wbg_clearColor_b7dd90543da7fe3c: function(arg0, arg1, arg2, arg3, arg4) {
getObject(arg0).clearColor(arg1, arg2, arg3, arg4);
},
__wbg_clear_55f5d5ab26fe80b4: function(arg0, arg1) {
getObject(arg0).clear(arg1 >>> 0);
},
__wbg_compileShader_cabeb4aa8cb42914: function(arg0, arg1) {
getObject(arg0).compileShader(getObject(arg1));
},
__wbg_createBuffer_77a9a8580e5123cc: function(arg0) {
const ret = getObject(arg0).createBuffer();
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_createProgram_89a2bb21dab7e222: function(arg0) {
const ret = getObject(arg0).createProgram();
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_createShader_70a84327b207bda4: function(arg0, arg1) {
const ret = getObject(arg0).createShader(arg1 >>> 0);
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_createTexture_a409a8c7972e99b3: function(arg0) {
const ret = getObject(arg0).createTexture();
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_createVertexArray_9411e94d2ee6864e: function(arg0) {
const ret = getObject(arg0).createVertexArray();
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_devicePixelRatio_da557bfb3b99fb02: function(arg0) {
const ret = getObject(arg0).devicePixelRatio;
return ret;
},
__wbg_disable_6907c683adc38e04: function(arg0, arg1) {
getObject(arg0).disable(arg1 >>> 0);
},
__wbg_documentElement_02547989338a4499: function(arg0) {
const ret = getObject(arg0).documentElement;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_document_c7f486c52d63d24e: function(arg0) {
const ret = getObject(arg0).document;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_drawArrays_983f352c9e94d800: function(arg0, arg1, arg2, arg3) {
getObject(arg0).drawArrays(arg1 >>> 0, arg2, arg3);
},
__wbg_enableVertexAttribArray_ad4d8ff487ea8fe1: function(arg0, arg1) {
getObject(arg0).enableVertexAttribArray(arg1 >>> 0);
},
__wbg_enable_9fb8e4de75a17155: function(arg0, arg1) {
getObject(arg0).enable(arg1 >>> 0);
},
__wbg_getBoundingClientRect_299f40f8641457e6: function(arg0) {
const ret = getObject(arg0).getBoundingClientRect();
return addHeapObject(ret);
},
__wbg_getContext_2c75bba1a1c9739b: function() { return handleError(function (arg0, arg1, arg2, arg3) {
const ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2), getObject(arg3));
return isLikeNone(ret) ? 0 : addHeapObject(ret);
}, arguments); },
__wbg_getElementById_ccc92d66acf76819: function(arg0, arg1, arg2) {
const ret = getObject(arg0).getElementById(getStringFromWasm0(arg1, arg2));
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_getProgramParameter_2a5860ab824690ec: function(arg0, arg1, arg2) {
const ret = getObject(arg0).getProgramParameter(getObject(arg1), arg2 >>> 0);
return addHeapObject(ret);
},
__wbg_getUniformLocation_03b9677a8f868582: function(arg0, arg1, arg2, arg3) {
const ret = getObject(arg0).getUniformLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_height_24586a23e4789d28: function(arg0) {
const ret = getObject(arg0).height;
return ret;
},
__wbg_innerHeight_f0945dfd6f2d3a87: function() { return handleError(function (arg0) {
const ret = getObject(arg0).innerHeight;
return addHeapObject(ret);
}, arguments); },
__wbg_innerWidth_1553ed0783afe5b4: function() { return handleError(function (arg0) {
const ret = getObject(arg0).innerWidth;
return addHeapObject(ret);
}, arguments); },
__wbg_instanceof_Window_a3b8566f0a9c5d1a: function(arg0) {
let result;
try {
result = getObject(arg0) instanceof Window;
} catch (_) {
result = false;
}
const ret = result;
return ret;
},
__wbg_linkProgram_e408ee5f93686f1f: function(arg0, arg1) {
getObject(arg0).linkProgram(getObject(arg1));
},
__wbg_mark_8b3755b2ab6fe425: function() { return handleError(function (arg0, arg1, arg2) {
getObject(arg0).mark(getStringFromWasm0(arg1, arg2));
}, arguments); },
__wbg_matchMedia_0e1d9f8bfb0f1f46: function() { return handleError(function (arg0, arg1, arg2) {
const ret = getObject(arg0).matchMedia(getStringFromWasm0(arg1, arg2));
return isLikeNone(ret) ? 0 : addHeapObject(ret);
}, arguments); },
__wbg_matches_0c791086704a6cb5: function(arg0) {
const ret = getObject(arg0).matches;
return ret;
},
__wbg_new_bebc3f4757acf305: function() {
const ret = new Object();
return addHeapObject(ret);
},
__wbg_performance_d96ad0b441f4510a: function(arg0) {
const ret = getObject(arg0).performance;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_pixelStorei_1bc7980b82e1ba4b: function(arg0, arg1, arg2) {
getObject(arg0).pixelStorei(arg1 >>> 0, arg2);
},
__wbg_requestAnimationFrame_4988887658fcee2e: function() { return handleError(function (arg0, arg1) {
const ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
return ret;
}, arguments); },
__wbg_set_a377297433dfea63: function() { return handleError(function (arg0, arg1, arg2) {
const ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
return ret;
}, arguments); },
__wbg_set_height_698fb3b255bc1348: function(arg0, arg1) {
getObject(arg0).height = arg1 >>> 0;
},
__wbg_set_width_4c3a2252e0dea033: function(arg0, arg1) {
getObject(arg0).width = arg1 >>> 0;
},
__wbg_shaderSource_108c27c8694ef4c0: function(arg0, arg1, arg2, arg3) {
getObject(arg0).shaderSource(getObject(arg1), getStringFromWasm0(arg2, arg3));
},
__wbg_static_accessor_GLOBAL_8eb4cd83130a11a0: function() {
const ret = typeof global === 'undefined' ? null : global;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_static_accessor_GLOBAL_THIS_1e7044f654e934db: function() {
const ret = typeof globalThis === 'undefined' ? null : globalThis;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_static_accessor_SELF_d8b50611246a6d92: function() {
const ret = typeof self === 'undefined' ? null : self;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_static_accessor_WINDOW_fd0bc376bf0f8b42: function() {
const ret = typeof window === 'undefined' ? null : window;
return isLikeNone(ret) ? 0 : addHeapObject(ret);
},
__wbg_texImage2D_ed186faa07e534c8: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9 === 0 ? undefined : getArrayU8FromWasm0(arg9, arg10));
}, arguments); },
__wbg_texParameteri_e127aa385b821d68: function(arg0, arg1, arg2, arg3) {
getObject(arg0).texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
},
__wbg_uniform1f_6067a9c69ed38c8b: function(arg0, arg1, arg2) {
getObject(arg0).uniform1f(getObject(arg1), arg2);
},
__wbg_uniform2f_2ef046aa3e732a9a: function(arg0, arg1, arg2, arg3) {
getObject(arg0).uniform2f(getObject(arg1), arg2, arg3);
},
__wbg_uniform4fv_95b1afba56301916: function(arg0, arg1, arg2, arg3) {
getObject(arg0).uniform4fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
},
__wbg_uniformMatrix4fv_e411faf4de6021c5: function(arg0, arg1, arg2, arg3, arg4) {
getObject(arg0).uniformMatrix4fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
},
__wbg_useProgram_fcab4319331e0094: function(arg0, arg1) {
getObject(arg0).useProgram(getObject(arg1));
},
__wbg_vertexAttribPointer_b6db7a62b9bda090: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
getObject(arg0).vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
},
__wbg_viewport_704a6159fb0395cb: function(arg0, arg1, arg2, arg3, arg4) {
getObject(arg0).viewport(arg1, arg2, arg3, arg4);
},
__wbg_width_71b671f940e3eb8c: function(arg0) {
const ret = getObject(arg0).width;
return ret;
},
__wbg_x_09473e91feed7557: function(arg0) {
const ret = getObject(arg0).x;
return ret;
},
__wbg_y_9897bd94adafed0f: function(arg0) {
const ret = getObject(arg0).y;
return ret;
},
__wbindgen_generic_0000000000000001: function(arg0, arg1) {
const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_107);
return addHeapObject(ret);
},
__wbindgen_generic_0000000000000002: function(arg0, arg1) {
const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_106);
return addHeapObject(ret);
},
__wbindgen_generic_0000000000000003: function(arg0, arg1) {
const ret = getStringFromWasm0(arg0, arg1);
return addHeapObject(ret);
},
__wbindgen_object_clone_ref: function(arg0) {
const ret = getObject(arg0);
return addHeapObject(ret);
},
__wbindgen_object_drop_ref: function(arg0) {
takeObject(arg0);
},
};
return {
__proto__: null,
"./lander_gl_bg.js": import0,
};
}
function __wasm_bindgen_func_elem_106(arg0, arg1) {
wasm.__wasm_bindgen_func_elem_106(arg0, arg1);
}
function __wasm_bindgen_func_elem_107(arg0, arg1, arg2) {
wasm.__wasm_bindgen_func_elem_107(arg0, arg1, arg2);
}
function addHeapObject(obj) {
if (heap_next === heap.length) heap.push(heap.length + 1);
const idx = heap_next;
heap_next = heap[idx];
heap[idx] = obj;
return idx;
}
const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
? { register: () => {}, unregister: () => {} }
: new FinalizationRegistry(state => wasm.__wbindgen_export2(state.a, state.b));
function dropObject(idx) {
if (idx < 1028) return;
heap[idx] = heap_next;
heap_next = idx;
}
function getArrayF32FromWasm0(ptr, len) {
ptr = ptr >>> 0;
return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}
function getArrayU8FromWasm0(ptr, len) {
ptr = ptr >>> 0;
return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}
let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
}
return cachedDataViewMemory0;
}
let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
}
return cachedFloat32ArrayMemory0;
}
function getStringFromWasm0(ptr, len) {
return decodeText(ptr >>> 0, len);
}
let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
}
return cachedUint8ArrayMemory0;
}
function getObject(idx) { return heap[idx]; }
function handleError(f, args) {
try {
return f.apply(this, args);
} catch (e) {
wasm.__wbindgen_export(addHeapObject(e));
}
}
let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);
let heap_next = heap.length;
function isLikeNone(x) {
return x === undefined || x === null;
}
function makeMutClosure(arg0, arg1, f) {
const state = { a: arg0, b: arg1, cnt: 1 };
const real = (...args) => {
state.cnt++;
const a = state.a;
state.a = 0;
try {
return f(a, state.b, ...args);
} finally {
state.a = a;
real._wbg_cb_unref();
}
};
real._wbg_cb_unref = () => {
if (--state.cnt === 0) {
wasm.__wbindgen_export2(state.a, state.b);
state.a = 0;
CLOSURE_DTORS.unregister(state);
}
};
CLOSURE_DTORS.register(real, state, state);
return real;
}
function takeObject(idx) {
const ret = getObject(idx);
dropObject(idx);
return ret;
}
let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
function decodeText(ptr, len) {
return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}
let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
wasmInstance = instance;
wasm = instance.exports;
wasmModule = module;
cachedDataViewMemory0 = null;
cachedFloat32ArrayMemory0 = null;
cachedUint8ArrayMemory0 = null;
wasm.__wbindgen_start();
return wasm;
}
async function __wbg_load(module, imports) {
if (typeof Response === 'function' && module instanceof Response) {
if (!module.ok) {
throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
}
if (typeof WebAssembly.instantiateStreaming === 'function') {
try {
return await WebAssembly.instantiateStreaming(module, imports);
} catch (e) {
const validResponse = expectedResponseType(module.type);
if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
} else { throw e; }
}
}
const bytes = await module.arrayBuffer();
return await WebAssembly.instantiate(bytes, imports);
} else {
const instance = await WebAssembly.instantiate(module, imports);
if (instance instanceof WebAssembly.Instance) {
return { instance, module };
} else {
return instance;
}
}
function expectedResponseType(type) {
switch (type) {
case 'basic': case 'cors': case 'default': return true;
}
return false;
}
}
function initSync(module) {
if (wasm !== undefined) return wasm;
if (module !== undefined) {
if (Object.getPrototypeOf(module) === Object.prototype) {
({module} = module)
} else {
console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
}
}
const imports = __wbg_get_imports();
if (!(module instanceof WebAssembly.Module)) {
module = new WebAssembly.Module(module);
}
const instance = new WebAssembly.Instance(module, imports);
return __wbg_finalize_init(instance, module);
}
async function __wbg_init(module_or_path) {
if (wasm !== undefined) return wasm;
if (module_or_path !== undefined) {
if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
({module_or_path} = module_or_path)
} else {
console.warn('using deprecated parameters for the initialization function; pass a single object instead')
}
}
if (module_or_path === undefined && script_src !== undefined) {
module_or_path = script_src.replace(/\.js$/, "_bg.wasm");
}
const imports = __wbg_get_imports();
if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
module_or_path = fetch(module_or_path);
}
const { instance, module } = await __wbg_load(await module_or_path, imports);
return __wbg_finalize_init(instance, module);
}
return Object.assign(__wbg_init, { initSync }, exports);
})({ __proto__: null });
