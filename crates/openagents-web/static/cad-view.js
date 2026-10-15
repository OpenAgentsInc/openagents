// The CAD viewer (#11653): a version turning in 3D in a chat card and on
// /cad/{doc}. Served from /chat/cad/card.js; the page's policy allows no
// inline script.
//
// It reads the version's view model (`cad-view.v1`, crates/cad/src/view.rs:
// each solid's triangles in millimetres with Z up, the B-rep face of each
// triangle, and the faces' stable names), or an older version's glTF.
//
// - Drag to turn, right-drag or Shift-drag to move, scroll to zoom; one
//   finger turns, two fingers pinch and move.
// - A tap or click picks a face or an edge and shows its stable name
//   (`body:top`, `{between: [..]}`); in a chat it goes into the composer as
//   a chip, so "make this 5 mm thicker" says which face.
// - Measure: two picks give their distance, the angle between the faces,
//   and the gap between parallel faces.
// - Section: a plane across X, Y or Z cuts the model.
// - Explode: moves each placed part away from the middle.
// - Parts: show and hide each placed part.
(function () {
  "use strict";
  if (window.__oaCadView) return;
  window.__oaCadView = 1;

  // ---- data ----------------------------------------------------------------
  function bytes(text) {
    var bin = atob(text || ""), out = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out.buffer;
  }
  function hex(color) {
    var n = parseInt(String(color || "#b4b8c0").slice(1), 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  }
  function bounds(pos) {
    var lo = [1e30, 1e30, 1e30], hi = [-1e30, -1e30, -1e30];
    for (var i = 0; i < pos.length; i += 3) for (var k = 0; k < 3; k++) {
      if (pos[i + k] < lo[k]) lo[k] = pos[i + k];
      if (pos[i + k] > hi[k]) hi[k] = pos[i + k];
    }
    return [lo, hi];
  }
  function fromView(json) {
    var solids = (json.solids || []).map(function (s) {
      var pos = new Float32Array(bytes(s.positions));
      return {
        instance: s.instance, part: s.part, color: hex(s.color), gloss: s.gloss || 0.4,
        pos: pos, nor: new Float32Array(bytes(s.normals)), idx: new Uint32Array(bytes(s.indices)),
        tface: new Uint32Array(bytes(s.faces)), names: s.face_names || [],
        edges: (s.edges || []).map(function (e) { return {faces: e.faces || [], pts: new Float32Array(bytes(e.points))}; }),
        box: bounds(pos)
      };
    });
    return {solids: solids, instances: json.instances || [], named: true};
  }
  // An older version's glTF (metres, Y up) in millimetres with Z up.
  function fromGlb(buf) {
    var dv = new DataView(buf);
    if (dv.getUint32(0, true) !== 0x46546c67) return null;
    var jl = dv.getUint32(12, true);
    var json = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 20, jl)));
    var bin = 20 + jl + 8, solids = [];
    (json.nodes || []).forEach(function (node, n) {
      var mesh = json.meshes[node.mesh];
      if (!mesh) return;
      mesh.primitives.forEach(function (p) {
        var acc = function (i, T, k) { var a = json.accessors[i], v = json.bufferViews[a.bufferView]; return new T(buf, bin + (v.byteOffset || 0), a.count * k); };
        var mat = json.materials[p.material] || {pbrMetallicRoughness: {baseColorFactor: [0.7, 0.7, 0.7, 1]}};
        var g = acc(p.attributes.POSITION, Float32Array, 3), gn = acc(p.attributes.NORMAL, Float32Array, 3);
        var pos = new Float32Array(g.length), nor = new Float32Array(gn.length);
        for (var i = 0; i < g.length; i += 3) {
          pos[i] = g[i] * 1000; pos[i + 1] = -g[i + 2] * 1000; pos[i + 2] = g[i + 1] * 1000;
          nor[i] = gn[i]; nor[i + 1] = -gn[i + 2]; nor[i + 2] = gn[i + 1];
        }
        var rough = mat.pbrMetallicRoughness.roughnessFactor;
        var name = String(node.name || "solid " + n), instance = name.split("/")[0];
        solids.push({instance: instance, part: instance, pos: pos, nor: nor,
          idx: new Uint32Array(acc(p.indices, Uint32Array, 1)), tface: null, names: [], edges: [],
          color: mat.pbrMetallicRoughness.baseColorFactor.slice(0, 3).map(function (x) { return Math.pow(x, 1 / 2.2); }),
          gloss: 1 - (rough === undefined ? 0.6 : rough), box: bounds(pos)});
      });
    });
    return {solids: solids, instances: [], named: false};
  }

  // ---- maths -----------------------------------------------------------------
  function sub(a, b) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]; }
  function add(a, b) { return [a[0] + b[0], a[1] + b[1], a[2] + b[2]]; }
  function mul(a, s) { return [a[0] * s, a[1] * s, a[2] * s]; }
  function dot(a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
  function cross(a, b) { return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]; }
  function len(a) { return Math.sqrt(dot(a, a)); }
  function unit(a) { var l = len(a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; }
  function mm(x) { return (Math.round(x * 10) / 10 + 0).toFixed(1); }

  // ---- shaders ---------------------------------------------------------------
  var MESH_VS = "#version 300 es\nin vec3 p;in vec3 n;uniform mat4 m;uniform vec3 o;out vec3 vn;out vec3 wp;void main(){vn=n;wp=p+o;gl_Position=m*vec4(wp,1.0);}";
  var MESH_FS = "#version 300 es\nprecision highp float;in vec3 vn;in vec3 wp;uniform vec3 c;uniform vec3 l;uniform vec3 e;uniform float g;uniform vec4 k;uniform float kon;uniform float flat_;out vec4 o;" +
    "void main(){if(kon>0.5&&dot(wp,k.xyz)>k.w)discard;if(flat_>0.5){o=vec4(c,1.0);return;}" +
    "if(!gl_FrontFacing){o=vec4(c*0.55+vec3(0.12),1.0);return;}" +
    "vec3 nn=normalize(vn);float d=max(dot(nn,l),0.0);float f=max(dot(nn,normalize(vec3(-0.4,-0.3,0.6))),0.0);" +
    "float s=pow(max(dot(reflect(-l,nn),e),0.0),6.0+90.0*g*g)*0.6*g*g;o=vec4(c*(0.36+0.55*d+0.16*f)+vec3(s),1.0);}";
  var LINE_VS = "#version 300 es\nin vec3 p;uniform mat4 m;uniform vec3 o;uniform float ps;out vec3 wp;void main(){wp=p+o;gl_Position=m*vec4(wp,1.0);gl_PointSize=ps;}";
  var LINE_FS = "#version 300 es\nprecision highp float;in vec3 wp;uniform vec4 c;uniform vec4 k;uniform float kon;out vec4 o;void main(){if(kon>0.5&&dot(wp,k.xyz)>k.w)discard;o=c;}";

  function program(gl, vs, fs) {
    function shader(type, src) { var s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); return s; }
    var p = gl.createProgram();
    gl.attachShader(p, shader(gl.VERTEX_SHADER, vs));
    gl.attachShader(p, shader(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    var u = {};
    ["m", "o", "c", "l", "e", "g", "k", "kon", "flat_", "ps"].forEach(function (name) { u[name] = gl.getUniformLocation(p, name); });
    return {p: p, u: u, ap: gl.getAttribLocation(p, "p"), an: gl.getAttribLocation(p, "n")};
  }

  // ---- one viewer ------------------------------------------------------------
  function start(root) {
    if (root.__started) return;
    root.__started = 1;
    var canvas = root.querySelector("canvas");
    var gl = canvas && canvas.getContext("webgl2", {antialias: true});
    if (!gl) return;
    function get(url) {
      return fetch(url, {credentials: "same-origin"}).then(function (r) {
        if (!r.ok) return null;
        return (/name=view\.json|\.json$/.test(url) || (r.headers.get("content-type") || "").includes("application/json")) ? r.json().then(fromView) : r.arrayBuffer().then(fromGlb);
      });
    }
    // An older version has no view model: its glTF turns, without names.
    var fallback = root.getAttribute("data-cad-fallback");
    get(root.getAttribute("data-cad-model")).then(function (model) {
      return model || (fallback ? get(fallback) : null);
    }).then(function (model) {
      if (model && model.solids.length) run(root, canvas, gl, model);
    }).catch(function () { /* the picture stays */ });
  }

  function run(root, canvas, gl, model) {
    canvas.hidden = false;
    var mesh = program(gl, MESH_VS, MESH_FS), line = program(gl, LINE_VS, LINE_FS);
    var gpuBuffers = [], gpuArrays = [];
    function vao() { var v = gl.createVertexArray(); gpuArrays.push(v); return v; }
    function buffer(target, data) { var b = gl.createBuffer(); gpuBuffers.push(b); gl.bindBuffer(target, b); gl.bufferData(target, data, gl.STATIC_DRAW); return b; }
    var lo = [1e30, 1e30, 1e30], hi = [-1e30, -1e30, -1e30];
    function uploadModel() {
      lo = [1e30, 1e30, 1e30]; hi = [-1e30, -1e30, -1e30];
    model.solids.forEach(function (s) {
      for (var k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], s.box[0][k]); hi[k] = Math.max(hi[k], s.box[1][k]); }
      s.vao = vao(); gl.bindVertexArray(s.vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, s.pbuf = buffer(gl.ARRAY_BUFFER, s.pos)); gl.enableVertexAttribArray(mesh.ap); gl.vertexAttribPointer(mesh.ap, 3, gl.FLOAT, false, 0, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, s.nbuf = buffer(gl.ARRAY_BUFFER, s.nor)); gl.enableVertexAttribArray(mesh.an); gl.vertexAttribPointer(mesh.an, 3, gl.FLOAT, false, 0, 0);
      buffer(gl.ELEMENT_ARRAY_BUFFER, s.idx);
      // The edges as line segments.
      var segs = [];
      s.edges.forEach(function (e) { for (var i = 3; i < e.pts.length; i += 3) segs.push(e.pts[i - 3], e.pts[i - 2], e.pts[i - 1], e.pts[i], e.pts[i + 1], e.pts[i + 2]); });
      s.lineCount = segs.length / 3;
      if (s.lineCount) {
        s.lvao = vao(); gl.bindVertexArray(s.lvao);
        gl.bindBuffer(gl.ARRAY_BUFFER, buffer(gl.ARRAY_BUFFER, new Float32Array(segs))); gl.enableVertexAttribArray(line.ap); gl.vertexAttribPointer(line.ap, 3, gl.FLOAT, false, 0, 0);
      }
      s.visible = true;
      s.offset = [0, 0, 0];
    });
    }
    uploadModel();
    var center = [0, 1, 2].map(function (k) { return (lo[k] + hi[k]) / 2; });
    var radius = Math.max(1e-3, len(sub(hi, lo)) / 2);
    var instances = model.instances.length ? model.instances : uniqueInstances(model.solids);
    // Far enough that the whole model fits, across the narrower side.
    function fit() {
      var a = Math.max(0.2, canvas.clientWidth / Math.max(1, canvas.clientHeight));
      return {yaw: -2.3, pitch: 0.45, dist: radius / (Math.tan(0.3) * Math.min(1, a)) * 1.08, target: center.slice()};
    }
    var cam = fit();
    var state = {mode: "pick", picks: [], section: null, explode: 0, hl: null, marks: [], failures: []};
    var originalModel = model;
    root.__cadSetModel = function (json) {
      gpuBuffers.forEach(function (b) { gl.deleteBuffer(b); });
      gpuArrays.forEach(function (v) { gl.deleteVertexArray(v); });
      gpuBuffers = []; gpuArrays = [];
      model = json ? fromView(json) : originalModel;
      uploadModel();
      state.hl = null; state.edge = null; state.marks = []; state.picks = []; state.failures = [];
      draw();
    };
    var pull = null, handle = null, stem = null;
    if (root.closest("[data-cad-pull]")) {
      handle = document.createElement("button");
      handle.type = "button"; handle.className = "oa-cad-pull-handle";
      handle.textContent = "Pull"; handle.setAttribute("aria-label", "Pull the selected face along its normal");
      stem = document.createElement("span"); stem.className = "oa-cad-pull-stem"; stem.setAttribute("aria-hidden", "true");
      stem.hidden = true; handle.hidden = true; root.querySelector(".oa-cad-view").append(stem, handle);
      root.addEventListener("cad-pull-handle", function (event) {
        pull = event.detail; handle.hidden = !pull; stem.hidden = !pull; draw();
      });
      var pulling = null;
      handle.addEventListener("pointerdown", function (e) {
        e.preventDefault(); e.stopPropagation();
        if (!pull) return;
        var r = ray(e.clientX, e.clientY), a = screen(pull.point, r);
        var b = screen(add(pull.point, mul(pull.normal, radius * 0.35)), r);
        if (!a || !b) return;
        var dx = b[0] - a[0], dy = b[1] - a[1], length = Math.hypot(dx, dy);
        if (length < 8) { say("Turn the view to see this face from the side, then pull it."); return; }
        pulling = {id:e.pointerId, x:e.clientX, y:e.clientY, dx:dx/length, dy:dy/length, scale:length/(radius*0.35), delta:0};
        handle.setPointerCapture(e.pointerId);
      });
      window.addEventListener("pointermove", function (e) {
        if (!pulling || e.pointerId !== pulling.id) return;
        pulling.delta = Math.round(((e.clientX-pulling.x)*pulling.dx+(e.clientY-pulling.y)*pulling.dy)/pulling.scale*10)/10;
        root.dispatchEvent(new CustomEvent("cad-face-drag", {detail:{delta:pulling.delta,commit:false}}));
      });
      function stopPull(e) {
        if (!pulling || e.pointerId !== pulling.id) return;
        root.dispatchEvent(new CustomEvent("cad-face-drag", {detail:{delta:pulling.delta,commit:e.type==="pointerup",cancel:e.type!=="pointerup"}}));
        pulling=null;
      }
      window.addEventListener("pointerup", stopPull); window.addEventListener("pointercancel", stopPull);
    }


    var queued = false;
    function findingFaces(findings) {
      state.failures.forEach(function (h) {
        gl.deleteVertexArray(h.vao); gl.deleteBuffer(h.buffer);
        gpuArrays=gpuArrays.filter(function(v){return v!==h.vao;}); gpuBuffers=gpuBuffers.filter(function(b){return b!==h.buffer;});
      });
      state.failures=[];
      model.solids.forEach(function(s) {
        if(!s.tface)return;
        var names=findings.filter(function(f){return f.part===s.part && (!f.instance || f.instance===s.instance);}).map(function(f){return f.face;});
        if(!names.length)return;
        var indices=[];
        for(var t=0;t<s.tface.length;t++) {
          var name=s.names[s.tface[t]];
          if(name && names.some(function(f){return name===f || name.startsWith(f+'#');}))indices.push(s.idx[t*3],s.idx[t*3+1],s.idx[t*3+2]);
        }
        if(!indices.length)return;
        var view=vao();gl.bindVertexArray(view);
        gl.bindBuffer(gl.ARRAY_BUFFER,s.pbuf);gl.enableVertexAttribArray(mesh.ap);gl.vertexAttribPointer(mesh.ap,3,gl.FLOAT,false,0,0);
        gl.bindBuffer(gl.ARRAY_BUFFER,s.nbuf);gl.enableVertexAttribArray(mesh.an);gl.vertexAttribPointer(mesh.an,3,gl.FLOAT,false,0,0);
        var selected=buffer(gl.ELEMENT_ARRAY_BUFFER,new Uint32Array(indices));
        state.failures.push({solid:s,vao:view,buffer:selected,count:indices.length});s.visible=true;
      });
      root.dataset.cadHighlightedFaces=String(findings.length);draw();
    }
    root.__cadHighlightFindings=findingFaces;
    var findingsOwner=root.closest('[data-cad-findings]');
    if(findingsOwner) {
      try {findingFaces(JSON.parse(findingsOwner.dataset.cadFindings));}catch(_){}
      findingsOwner.querySelectorAll('[data-cad-faces]').forEach(function(button){button.addEventListener('click',function(){
        try {findingFaces(JSON.parse(button.dataset.cadFaces));}catch(_){}
      });});
    }

    // ---- the camera ----
    function basis(w, h) {
      var cp = Math.cos(cam.pitch);
      var eye = add(cam.target, mul([cp * Math.cos(cam.yaw), cp * Math.sin(cam.yaw), Math.sin(cam.pitch)], cam.dist));
      var f = unit(sub(cam.target, eye));
      var r = unit(cross(f, [0, 0, 1]));
      var u = cross(r, f);
      return {eye: eye, f: f, r: r, u: u, t: Math.tan(0.3), a: w / h};
    }
    function matrix(b) {
      var near = Math.max(cam.dist - radius * 3, cam.dist * 0.01), far = cam.dist + radius * 4;
      var view = [b.r[0], b.u[0], -b.f[0], 0, b.r[1], b.u[1], -b.f[1], 0, b.r[2], b.u[2], -b.f[2], 0,
        -dot(b.r, b.eye), -dot(b.u, b.eye), dot(b.f, b.eye), 1];
      var proj = [1 / (b.t * b.a), 0, 0, 0, 0, 1 / b.t, 0, 0, 0, 0, (far + near) / (near - far), -1, 0, 0, 2 * far * near / (near - far), 0];
      var m = new Float32Array(16);
      for (var i = 0; i < 4; i++) for (var j = 0; j < 4; j++) { var s = 0; for (var k = 0; k < 4; k++) s += proj[k * 4 + j] * view[i * 4 + k]; m[i * 4 + j] = s; }
      return m;
    }
    function draw() { if (!queued) { queued = true; requestAnimationFrame(frame); } }
    function clip() {
      if (!state.section) return null;
      var s = state.section, k = s.axis, n = [0, 0, 0];
      n[k] = 1;
      return [n[0], n[1], n[2], lo[k] + (hi[k] - lo[k]) * s.at];
    }
    function frame() {
      queued = false;
      var dpr = Math.min(window.devicePixelRatio || 1, 2);
      var w = Math.max(1, Math.round(canvas.clientWidth * dpr)), h = Math.max(1, Math.round(canvas.clientHeight * dpr));
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
      gl.viewport(0, 0, w, h);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      gl.enable(gl.DEPTH_TEST);
      var b = basis(w, h), m = matrix(b), k = clip();
      if (pull && handle) {
        var tip = screen(add(pull.point, mul(pull.normal, radius*0.35)), {b:b,rect:canvas.getBoundingClientRect()});
        var anchor = screen(pull.point, {b:b,rect:canvas.getBoundingClientRect()});
        if (tip && anchor) {
          handle.style.left = tip[0]+"px"; handle.style.top = tip[1]+"px";
          stem.style.left = anchor[0]+"px"; stem.style.top = anchor[1]+"px";
          stem.style.width = Math.hypot(tip[0]-anchor[0],tip[1]-anchor[1])+"px";
          stem.style.transform = "rotate("+Math.atan2(tip[1]-anchor[1],tip[0]-anchor[0])+"rad)";
        }
      }
      var l = unit(add(add(mul(b.f, -1), mul(b.u, 0.6)), mul(b.r, 0.3)));
      gl.useProgram(mesh.p);
      gl.uniformMatrix4fv(mesh.u.m, false, m);
      gl.uniform3fv(mesh.u.l, l);
      gl.uniform3fv(mesh.u.e, mul(b.f, -1));
      gl.uniform4fv(mesh.u.k, k || [0, 0, 1, 0]);
      gl.uniform1f(mesh.u.kon, k ? 1 : 0);
      gl.uniform1f(mesh.u.flat_, 0);
      gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(1, 1);
      model.solids.forEach(function (s) {
        if (!s.visible) return;
        gl.uniform3fv(mesh.u.o, s.offset);
        gl.uniform3fv(mesh.u.c, s.color);
        gl.uniform1f(mesh.u.g, s.gloss);
        gl.bindVertexArray(s.vao);
        gl.drawElements(gl.TRIANGLES, s.idx.length, gl.UNSIGNED_INT, 0);
      });
      gl.disable(gl.POLYGON_OFFSET_FILL);
      // Manufacturing findings share the model's named faces.
      if(state.failures.length) {
        gl.uniform1f(mesh.u.flat_,1);gl.uniform3fv(mesh.u.c,[0.95,0.24,0.08]);gl.depthFunc(gl.LEQUAL);
        state.failures.forEach(function(h){gl.uniform3fv(mesh.u.o,h.solid.offset);gl.bindVertexArray(h.vao);gl.drawElements(gl.TRIANGLES,h.count,gl.UNSIGNED_INT,0);});
        gl.depthFunc(gl.LESS);gl.uniform1f(mesh.u.flat_,0);
      }
      // The picked face, in the accent colour.
      if (state.hl && state.hl.solid.visible) {
        gl.uniform1f(mesh.u.flat_, 1);
        gl.uniform3fv(mesh.u.c, [0.98, 0.62, 0.16]);
        gl.uniform3fv(mesh.u.o, state.hl.solid.offset);
        gl.depthFunc(gl.LEQUAL);
        gl.bindVertexArray(state.hl.vao);
        gl.drawElements(gl.TRIANGLES, state.hl.count, gl.UNSIGNED_INT, 0);
        gl.depthFunc(gl.LESS);
      }
      // The edges.
      gl.useProgram(line.p);
      gl.uniformMatrix4fv(line.u.m, false, m);
      gl.uniform4fv(line.u.k, k || [0, 0, 1, 0]);
      gl.uniform1f(line.u.kon, k ? 1 : 0);
      gl.uniform1f(line.u.ps, 1);
      gl.uniform4fv(line.u.c, [0.08, 0.09, 0.11, 0.55]);
      model.solids.forEach(function (s) {
        if (!s.visible || !s.lineCount) return;
        gl.uniform3fv(line.u.o, s.offset);
        gl.bindVertexArray(s.lvao);
        gl.drawArrays(gl.LINES, 0, s.lineCount);
      });
      // A picked edge, and the measured points.
      gl.disable(gl.DEPTH_TEST);
      if (state.edge) {
        gl.uniform4fv(line.u.c, [0.98, 0.62, 0.16, 1]);
        gl.uniform3fv(line.u.o, state.edge.solid.offset);
        gl.bindVertexArray(state.edge.vao);
        gl.drawArrays(gl.LINES, 0, state.edge.count);
      }
      if (state.marks.length) {
        var pts = [];
        state.marks.forEach(function (p) { pts.push(p[0], p[1], p[2]); });
        if (!state.markVao) { state.markVao = gl.createVertexArray(); state.markBuf = gl.createBuffer(); }
        gl.bindVertexArray(state.markVao);
        gl.bindBuffer(gl.ARRAY_BUFFER, state.markBuf);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(pts), gl.DYNAMIC_DRAW);
        gl.enableVertexAttribArray(line.ap); gl.vertexAttribPointer(line.ap, 3, gl.FLOAT, false, 0, 0);
        gl.uniform3fv(line.u.o, [0, 0, 0]);
        gl.uniform4fv(line.u.c, [0.15, 0.45, 0.95, 1]);
        gl.uniform1f(line.u.ps, 9 * dpr);
        gl.drawArrays(gl.POINTS, 0, state.marks.length);
        if (state.marks.length === 2) gl.drawArrays(gl.LINES, 0, 2);
      }
      gl.enable(gl.DEPTH_TEST);
    }

    // ---- picking ----
    function ray(x, y) {
      var rect = canvas.getBoundingClientRect();
      var b = basis(rect.width, rect.height);
      var sx = ((x - rect.left) / rect.width) * 2 - 1, sy = 1 - ((y - rect.top) / rect.height) * 2;
      var d = unit(add(b.f, add(mul(b.r, sx * b.t * b.a), mul(b.u, sy * b.t))));
      return {o: b.eye, d: d, b: b, rect: rect};
    }
    function hitBox(o, d, box) {
      var t0 = -1e30, t1 = 1e30;
      for (var k = 0; k < 3; k++) {
        if (Math.abs(d[k]) < 1e-12) { if (o[k] < box[0][k] || o[k] > box[1][k]) return false; continue; }
        var a = (box[0][k] - o[k]) / d[k], c = (box[1][k] - o[k]) / d[k];
        t0 = Math.max(t0, Math.min(a, c)); t1 = Math.min(t1, Math.max(a, c));
      }
      return t1 >= Math.max(t0, 0);
    }
    function pickFace(r) {
      var best = null, k = clip();
      model.solids.forEach(function (s) {
        if (!s.visible) return;
        var o = sub(r.o, s.offset);
        if (!hitBox(o, r.d, s.box)) return;
        var P = s.pos, I = s.idx;
        for (var t = 0; t < I.length; t += 3) {
          var a = I[t] * 3, b2 = I[t + 1] * 3, c = I[t + 2] * 3;
          var e1 = [P[b2] - P[a], P[b2 + 1] - P[a + 1], P[b2 + 2] - P[a + 2]];
          var e2 = [P[c] - P[a], P[c + 1] - P[a + 1], P[c + 2] - P[a + 2]];
          var pv = cross(r.d, e2), det = dot(e1, pv);
          if (Math.abs(det) < 1e-12) continue;
          var inv = 1 / det, tv = [o[0] - P[a], o[1] - P[a + 1], o[2] - P[a + 2]];
          var u = dot(tv, pv) * inv; if (u < 0 || u > 1) continue;
          var q = cross(tv, e1), v = dot(r.d, q) * inv; if (v < 0 || u + v > 1) continue;
          var dist = dot(e2, q) * inv;
          if (dist <= 0 || (best && dist >= best.dist)) continue;
          var at = add(r.o, mul(r.d, dist));
          if (k && dot(at, k) > k[3]) continue;
          best = {solid: s, tri: t / 3, dist: dist, at: at, normal: unit(cross(e1, e2))};
        }
      });
      return best;
    }
    function screen(p, r) {
      var v = sub(p, r.b.eye), z = dot(v, r.b.f);
      if (z <= 0) return null;
      return [r.rect.width / 2 * (1 + dot(v, r.b.r) / (z * r.b.t * r.b.a)), r.rect.height / 2 * (1 - dot(v, r.b.u) / (z * r.b.t)), z];
    }
    function pickEdge(r, x, y, face, limit) {
      var best = null, px = x - r.rect.left, py = y - r.rect.top;
      model.solids.forEach(function (s) {
        if (!s.visible) return;
        s.edges.forEach(function (e, ei) {
          for (var i = 3; i < e.pts.length; i += 3) {
            var a = screen(add([e.pts[i - 3], e.pts[i - 2], e.pts[i - 1]], s.offset), r), b = screen(add([e.pts[i], e.pts[i + 1], e.pts[i + 2]], s.offset), r);
            if (!a || !b) continue;
            var dx = b[0] - a[0], dy = b[1] - a[1], l2 = dx * dx + dy * dy;
            var t = l2 ? Math.max(0, Math.min(1, ((px - a[0]) * dx + (py - a[1]) * dy) / l2)) : 0;
            var qx = a[0] + t * dx - px, qy = a[1] + t * dy - py, d = Math.sqrt(qx * qx + qy * qy);
            var z = a[2] + t * (b[2] - a[2]);
            // An edge behind the face under the pointer isn't the one meant.
            if (face && z > face.dist * dot(r.d, r.b.f) + radius * 0.02) continue;
            if (d < limit && (!best || d < best.d)) best = {solid: s, edge: e, index: ei, d: d};
          }
        });
      });
      return best;
    }
    function faceName(s, tri) { return s.tface ? (s.names[s.tface[tri]] || null) : null; }
    function highlight(hit) {
      state.hl = null; state.edge = null;
      if (!hit) return;
      if (hit.edge) {
        var segs = [], p = hit.edge.pts;
        for (var i = 3; i < p.length; i += 3) segs.push(p[i - 3], p[i - 2], p[i - 1], p[i], p[i + 1], p[i + 2]);
        var selection = vao(); gl.bindVertexArray(selection);
        gl.bindBuffer(gl.ARRAY_BUFFER, buffer(gl.ARRAY_BUFFER, new Float32Array(segs))); gl.enableVertexAttribArray(line.ap); gl.vertexAttribPointer(line.ap, 3, gl.FLOAT, false, 0, 0);
        state.edge = {solid: hit.solid, vao: selection, count: segs.length / 3};
        return;
      }
      var s = hit.solid;
      if (!s.tface) return;
      var f = s.tface[hit.tri], sel = [];
      for (var t = 0; t < s.tface.length; t++) if (s.tface[t] === f) sel.push(s.idx[t * 3], s.idx[t * 3 + 1], s.idx[t * 3 + 2]);
      var hv = vao(); gl.bindVertexArray(hv);
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer(gl.ARRAY_BUFFER, s.pos)); gl.enableVertexAttribArray(mesh.ap); gl.vertexAttribPointer(mesh.ap, 3, gl.FLOAT, false, 0, 0);
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer(gl.ARRAY_BUFFER, s.nor)); gl.enableVertexAttribArray(mesh.an); gl.vertexAttribPointer(mesh.an, 3, gl.FLOAT, false, 0, 0);
      buffer(gl.ELEMENT_ARRAY_BUFFER, new Uint32Array(sel));
      state.hl = {solid: s, vao: hv, count: sel.length};
    }
    var readout = root.querySelector("[data-cad-readout]");
    function say(text) { if (readout) { readout.textContent = text; readout.hidden = !text; } }

    function picked(x, y, touch) {
      var r = ray(x, y), face = pickFace(r), edge = state.mode === "pick" && model.named ? pickEdge(r, x, y, face, touch ? 6 : 5) : null;
      if (state.mode === "measure") {
        if (!face) return;
        if (state.marks.length >= 2) { state.marks = []; state.picks = []; }
        state.marks.push(face.at); state.picks.push(face);
        highlight(face);
        if (state.picks.length === 1) say("Pick a second point.");
        else {
          var a = state.picks[0], b = state.picks[1];
          var d = len(sub(b.at, a.at)), cos = Math.min(1, Math.abs(dot(a.normal, b.normal)));
          var angle = Math.acos(cos) * 180 / Math.PI, text = "Distance " + mm(d) + " mm · angle between the faces " + mm(angle) + "°";
          if (angle < 0.5) text += " · gap " + mm(Math.abs(dot(sub(b.at, a.at), a.normal))) + " mm";
          say(text);
        }
        draw();
        return;
      }
      if (edge) {
        highlight({solid: edge.solid, edge: edge.edge});
        var names = edge.edge.faces.map(function (f) { return edge.solid.names[f] || null; });
        if (names.length === 2 && names[0] && names[1]) {
          say("Edge between " + names[0] + " and " + names[1] + " · " + edge.solid.part);
          chip({part: edge.solid.part, instance: edge.solid.instance, edge: names}, "edge " + names[0] + " / " + names[1]);
        } else say("This edge has no name we can use yet.");
      } else if (face) {
        highlight(face);
        var name = faceName(face.solid, face.tri);
        if (name) {
          say("Face " + name + " · " + face.solid.part);
          chip({part: face.solid.part, instance: face.solid.instance, face: name}, name);
          root.dispatchEvent(new CustomEvent("cad-face-pick", {detail:{part:face.solid.part,face:name,point:face.at,normal:face.normal}}));
        } else say(model.named ? "This face has no name we can use yet." : "");
      } else { highlight(null); say(""); }
      draw();
    }

    // ---- the composer chip ----
    function chip(pick, label) {
      var form = document.getElementById("chat-form");
      var tray = form && form.querySelector("[data-oa-cad-picks]");
      var doc = root.getAttribute("data-cad-doc"), version = root.getAttribute("data-cad-version");
      if (!tray || !doc) return;
      pick.doc = doc; pick.version = Number(version);
      tray.querySelector("input").value = JSON.stringify(pick);
      var list = tray.querySelector("ul");
      list.textContent = "";
      var item = document.createElement("li"); item.className = "oa-file-chip"; item.dataset.state = "added";
      var name = document.createElement("span"); name.className = "oa-file-chip-name";
      name.textContent = label + " · " + pick.part; name.title = name.textContent;
      var remove = document.createElement("button");
      remove.type = "button"; remove.className = "oa-file-chip-remove"; remove.title = "Remove"; remove.textContent = "×";
      remove.setAttribute("aria-label", "Remove " + label);
      remove.addEventListener("click", function () { clearChip(tray); });
      item.append(name, remove); list.append(item);
      tray.hidden = false;
      var input = document.getElementById("chat-input");
      if (input) input.focus();
    }

    // ---- the pointer ----
    var pointers = new Map(), gesture = null;
    canvas.addEventListener("contextmenu", function (e) { e.preventDefault(); });
    canvas.addEventListener("pointerdown", function (e) {
      canvas.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, [e.clientX, e.clientY]);
      if (pointers.size === 1) gesture = {x: e.clientX, y: e.clientY, at: Date.now(), moved: 0, pan: e.button === 2 || e.shiftKey || e.ctrlKey || e.metaKey};
      else gesture = gesture && Object.assign(gesture, {moved: 99});
    });
    function pinch() { var p = Array.from(pointers.values()); return {d: len(sub([p[0][0], p[0][1], 0], [p[1][0], p[1][1], 0])), m: [(p[0][0] + p[1][0]) / 2, (p[0][1] + p[1][1]) / 2]}; }
    function pan(dx, dy) {
      var b = basis(canvas.clientWidth, canvas.clientHeight), s = 2 * cam.dist * b.t / Math.max(1, canvas.clientHeight);
      cam.target = add(cam.target, add(mul(b.r, -dx * s), mul(b.u, dy * s)));
    }
    var last = null;
    canvas.addEventListener("pointermove", function (e) {
      if (!pointers.has(e.pointerId)) return;
      var before = pointers.size === 2 ? pinch() : null, prev = pointers.get(e.pointerId);
      pointers.set(e.pointerId, [e.clientX, e.clientY]);
      var dx = e.clientX - prev[0], dy = e.clientY - prev[1];
      if (gesture) gesture.moved += Math.abs(dx) + Math.abs(dy);
      if (pointers.size === 2 && before) {
        var now = pinch();
        if (now.d > 0 && before.d > 0) cam.dist = Math.max(radius * 0.05, Math.min(radius * 40, cam.dist * before.d / now.d));
        pan(now.m[0] - before.m[0], now.m[1] - before.m[1]);
      } else if (gesture && gesture.pan) pan(dx, dy);
      else {
        cam.yaw -= dx * 0.01;
        cam.pitch = Math.max(-1.5, Math.min(1.5, cam.pitch + dy * 0.01));
      }
      last = e;
      draw();
    });
    function up(e) {
      var single = pointers.size === 1;
      pointers.delete(e.pointerId);
      if (single && gesture && gesture.moved < 6 && Date.now() - gesture.at < 600 && e.type === "pointerup") picked(e.clientX, e.clientY, e.pointerType === "touch");
      if (!pointers.size) gesture = null;
    }
    canvas.addEventListener("pointerup", up);
    canvas.addEventListener("pointercancel", up);
    canvas.addEventListener("wheel", function (e) {
      e.preventDefault();
      cam.dist = Math.max(radius * 0.05, Math.min(radius * 40, cam.dist * Math.exp(e.deltaY * 0.001)));
      draw();
    }, {passive: false});
    new ResizeObserver(draw).observe(canvas);

    // ---- the tools ----
    var tools = root.querySelector("[data-cad-tools]");
    function panel(name) { return root.querySelector('[data-cad-panel="' + name + '"]'); }
    function toggle(name, on) {
      var b = root.querySelector('[data-cad-tool="' + name + '"]');
      if (b) { b.toggleAttribute("data-selected", on); b.setAttribute("aria-pressed", on ? "true" : "false"); }
      var p = panel(name);
      if (p) p.hidden = !on;
    }
    function slider(input) {
      var text = input.closest(".oa-slider") && input.closest(".oa-slider").querySelector(".oa-slider__value-text");
      if (text) text.textContent = input.value;
      var min = Number(input.min), max = Number(input.max);
      input.dataset.fill = String(max > min ? Math.round(((Number(input.value) - min) / (max - min)) * 20) * 5 : 0);
    }
    function explode(pct) {
      state.explode = pct;
      var where = {};
      instances.forEach(function (i) { where[i.id] = i.center; });
      model.solids.forEach(function (s) {
        var c = where[s.instance] || center;
        s.offset = mul(sub(c, center), pct / 100 * 1.2);
      });
      draw();
    }
    function section() {
      var p = panel("section");
      if (!p) return;
      var axis = p.querySelector("input[type=radio]:checked"), at = p.querySelector("input[type=range]");
      state.section = {axis: axis ? Number(axis.value) : 0, at: at ? Number(at.value) / 100 : 0.5};
      draw();
    }
    if (tools) {
      tools.querySelectorAll("[data-cad-tool]").forEach(function (b) {
        var name = b.getAttribute("data-cad-tool");
        if ((name === "explode" || name === "parts") && instances.length < 2) { b.hidden = true; return; }
        b.setAttribute("aria-pressed", "false");
        b.addEventListener("click", function () {
          if (name === "fit") {
            cam = fit();
            draw();
            return;
          }
          var on = !b.hasAttribute("data-selected");
          toggle(name, on);
          if (name === "measure") {
            state.mode = on ? "measure" : "pick"; state.marks = []; state.picks = []; highlight(null);
            say(on ? "Pick two points on the model." : "");
          } else if (name === "section") {
            if (on) section(); else state.section = null;
          } else if (name === "explode") {
            var range = panel("explode") && panel("explode").querySelector("input[type=range]");
            explode(on && range ? Number(range.value) : 0);
          }
          draw();
        });
      });
      var sec = panel("section");
      if (sec) {
        sec.addEventListener("input", function (e) { if (e.target.type === "range") slider(e.target); section(); });
        sec.addEventListener("change", section);
      }
      var exp = panel("explode");
      if (exp) exp.addEventListener("input", function (e) { slider(e.target); explode(Number(e.target.value)); });
      var parts = panel("parts");
      if (parts && instances.length > 1) {
        var list = parts.querySelector("[data-cad-parts]");
        instances.forEach(function (inst, n) {
          var id = root.id + "-part-" + n;
          var label = document.createElement("label"); label.className = "oa-checkbox"; label.htmlFor = id;
          var box = document.createElement("span"); box.className = "oa-checkbox__box";
          var input = document.createElement("input"); input.className = "oa-checkbox__input"; input.type = "checkbox"; input.id = id; input.checked = true;
          var mark = document.createElement("span"); mark.className = "oa-checkbox__mark"; mark.setAttribute("aria-hidden", "true");
          box.append(input, mark);
          var text = document.createElement("span"); text.className = "oa-checkbox__label";
          var words = document.createElement("span"); words.textContent = inst.id === inst.part ? inst.id : inst.id + " (" + inst.part + ")";
          text.append(words);
          label.append(box, text);
          input.addEventListener("change", function () {
            model.solids.forEach(function (s) { if (s.instance === inst.id) s.visible = input.checked; });
            draw();
          });
          list.append(label);
        });
      }
      tools.hidden = false;
    }

    canvas.hidden = false;
    var still = root.querySelector("img"); if (still) still.hidden = true;
    draw();
  }

  function uniqueInstances(solids) {
    var seen = {}, out = [];
    solids.forEach(function (s) {
      if (seen[s.instance]) return;
      seen[s.instance] = 1;
      var lo = [1e30, 1e30, 1e30], hi = [-1e30, -1e30, -1e30];
      solids.forEach(function (t) { if (t.instance === s.instance) for (var k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], t.box[0][k]); hi[k] = Math.max(hi[k], t.box[1][k]); } });
      out.push({id: s.instance, part: s.part, center: [0, 1, 2].map(function (k) { return (lo[k] + hi[k]) / 2; })});
    });
    return out;
  }

  // ---- the composer's picked-face chip ----
  function clearChip(tray) {
    tray.querySelector("input").value = "";
    tray.querySelector("ul").textContent = "";
    tray.hidden = true;
  }
  document.addEventListener("htmx:afterRequest", function (e) {
    var form = document.getElementById("chat-form");
    if (!form || e.target !== form || !e.detail || !e.detail.successful) return;
    var tray = form.querySelector("[data-oa-cad-picks]");
    if (tray) clearChip(tray);
  });

  function scan() { document.querySelectorAll("[data-cad-model]").forEach(start); }
  document.addEventListener("DOMContentLoaded", scan);
  document.addEventListener("htmx:afterSettle", scan);
  new MutationObserver(scan).observe(document.documentElement, {childList: true, subtree: true});
  scan();
})();
