import {
  __require,
  __toESM
} from "./chat-client-13a4mv5g.js";

// ../../node_modules/.pnpm/@electric-sql+pglite@0.3.3/node_modules/@electric-sql/pglite/dist/chunk-BTBUZ646.js
var p = Object.create;
var i = Object.defineProperty;
var c = Object.getOwnPropertyDescriptor;
var f = Object.getOwnPropertyNames;
var l = Object.getPrototypeOf;
var s = Object.prototype.hasOwnProperty;
var a = (t) => {
  throw TypeError(t);
};
var _ = (t, e, o) => (e in t) ? i(t, e, { enumerable: true, configurable: true, writable: true, value: o }) : t[e] = o;
var d = (t, e) => () => (t && (e = t(t = 0)), e);
var D = (t, e) => () => (e || t((e = { exports: {} }).exports, e), e.exports);
var F = (t, e) => {
  for (var o in e)
    i(t, o, { get: e[o], enumerable: true });
};
var g = (t, e, o, m) => {
  if (e && typeof e == "object" || typeof e == "function")
    for (let r of f(e))
      !s.call(t, r) && r !== o && i(t, r, { get: () => e[r], enumerable: !(m = c(e, r)) || m.enumerable });
  return t;
};
var L = (t, e, o) => (o = t != null ? p(l(t)) : {}, g(e || !t || !t.__esModule ? i(o, "default", { value: t, enumerable: true }) : o, t));
var P = (t, e, o) => _(t, typeof e != "symbol" ? e + "" : e, o);
var n = (t, e, o) => e.has(t) || a("Cannot " + o);
var h = (t, e, o) => (n(t, e, "read from private field"), o ? o.call(t) : e.get(t));
var R = (t, e, o) => e.has(t) ? a("Cannot add the same private member more than once") : e instanceof WeakSet ? e.add(t) : e.set(t, o);
var x = (t, e, o, m) => (n(t, e, "write to private field"), m ? m.call(t, o) : e.set(t, o), o);
var T = (t, e, o) => (n(t, e, "access private method"), o);
var U = (t, e, o, m) => ({ set _(r) {
  x(t, e, r, o);
}, get _() {
  return h(t, e, m);
} });
var u = d(() => {
});

// ../../node_modules/.pnpm/@electric-sql+pglite@0.3.3/node_modules/@electric-sql/pglite/dist/chunk-WGR4JCLS.js
var w = D(($r, l2) => {
  u();
  var j = 9007199254740991, B = function(r) {
    return r;
  }();
  function mr(r) {
    return r === B;
  }
  function q(r) {
    return typeof r == "string" || Object.prototype.toString.call(r) == "[object String]";
  }
  function lr(r) {
    return Object.prototype.toString.call(r) == "[object Date]";
  }
  function N(r) {
    return r !== null && typeof r == "object";
  }
  function U2(r) {
    return typeof r == "function";
  }
  function fr(r) {
    return typeof r == "number" && r > -1 && r % 1 == 0 && r <= j;
  }
  function yr(r) {
    return Object.prototype.toString.call(r) == "[object Array]";
  }
  function Y(r) {
    return N(r) && !U2(r) && fr(r.length);
  }
  function D2(r) {
    return Object.prototype.toString.call(r) == "[object ArrayBuffer]";
  }
  function gr(r, e) {
    return Array.prototype.map.call(r, e);
  }
  function hr(r, e) {
    var t = B;
    return U2(e) && Array.prototype.every.call(r, function(s2, a2, n2) {
      var o = e(s2, a2, n2);
      return o && (t = s2), !o;
    }), t;
  }
  function Sr(r) {
    return Object.assign.apply(null, arguments);
  }
  function W(r) {
    var e, t, s2;
    if (q(r)) {
      for (t = r.length, s2 = new Uint8Array(t), e = 0;e < t; e++)
        s2[e] = r.charCodeAt(e) & 255;
      return s2;
    }
    return D2(r) ? new Uint8Array(r) : N(r) && D2(r.buffer) ? new Uint8Array(r.buffer) : Y(r) ? new Uint8Array(r) : N(r) && U2(r.toString) ? W(r.toString()) : new Uint8Array;
  }
  l2.exports.MAX_SAFE_INTEGER = j;
  l2.exports.isUndefined = mr;
  l2.exports.isString = q;
  l2.exports.isObject = N;
  l2.exports.isDateTime = lr;
  l2.exports.isFunction = U2;
  l2.exports.isArray = yr;
  l2.exports.isArrayLike = Y;
  l2.exports.isArrayBuffer = D2;
  l2.exports.map = gr;
  l2.exports.find = hr;
  l2.exports.extend = Sr;
  l2.exports.toUint8Array = W;
});
var x2 = D((Qr, X) => {
  u();
  var M = "\x00";
  X.exports = { NULL_CHAR: M, TMAGIC: "ustar" + M + "00", OLDGNU_MAGIC: "ustar  " + M, REGTYPE: 0, LNKTYPE: 1, SYMTYPE: 2, CHRTYPE: 3, BLKTYPE: 4, DIRTYPE: 5, FIFOTYPE: 6, CONTTYPE: 7, TSUID: parseInt("4000", 8), TSGID: parseInt("2000", 8), TSVTX: parseInt("1000", 8), TUREAD: parseInt("0400", 8), TUWRITE: parseInt("0200", 8), TUEXEC: parseInt("0100", 8), TGREAD: parseInt("0040", 8), TGWRITE: parseInt("0020", 8), TGEXEC: parseInt("0010", 8), TOREAD: parseInt("0004", 8), TOWRITE: parseInt("0002", 8), TOEXEC: parseInt("0001", 8), TPERMALL: parseInt("0777", 8), TPERMMASK: parseInt("0777", 8) };
});
var L2 = D((ee, f2) => {
  u();
  var K = w(), p2 = x2(), Fr = 512, I = p2.TPERMALL, V = 0, Z = 0, _2 = [["name", 100, 0, function(r, e) {
    return v(r[e[0]], e[1]);
  }, function(r, e, t) {
    return A(r.slice(e, e + t[1]));
  }], ["mode", 8, 100, function(r, e) {
    var t = r[e[0]] || I;
    return t = t & p2.TPERMMASK, P2(t, e[1], I);
  }, function(r, e, t) {
    var s2 = S(r.slice(e, e + t[1]));
    return s2 &= p2.TPERMMASK, s2;
  }], ["uid", 8, 108, function(r, e) {
    return P2(r[e[0]], e[1], V);
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["gid", 8, 116, function(r, e) {
    return P2(r[e[0]], e[1], Z);
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["size", 12, 124, function(r, e) {
    return P2(r.data.length, e[1]);
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["modifyTime", 12, 136, function(r, e) {
    return k(r[e[0]], e[1]);
  }, function(r, e, t) {
    return z(r.slice(e, e + t[1]));
  }], ["checksum", 8, 148, function(r, e) {
    return "        ";
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["type", 1, 156, function(r, e) {
    return "" + (parseInt(r[e[0]], 10) || 0) % 8;
  }, function(r, e, t) {
    return (parseInt(String.fromCharCode(r[e]), 10) || 0) % 8;
  }], ["linkName", 100, 157, function(r, e) {
    return "";
  }, function(r, e, t) {
    return A(r.slice(e, e + t[1]));
  }], ["ustar", 8, 257, function(r, e) {
    return p2.TMAGIC;
  }, function(r, e, t) {
    return br(A(r.slice(e, e + t[1]), true));
  }, function(r, e) {
    return r[e[0]] == p2.TMAGIC || r[e[0]] == p2.OLDGNU_MAGIC;
  }], ["owner", 32, 265, function(r, e) {
    return v(r[e[0]], e[1]);
  }, function(r, e, t) {
    return A(r.slice(e, e + t[1]));
  }], ["group", 32, 297, function(r, e) {
    return v(r[e[0]], e[1]);
  }, function(r, e, t) {
    return A(r.slice(e, e + t[1]));
  }], ["majorNumber", 8, 329, function(r, e) {
    return "";
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["minorNumber", 8, 337, function(r, e) {
    return "";
  }, function(r, e, t) {
    return S(r.slice(e, e + t[1]));
  }], ["prefix", 131, 345, function(r, e) {
    return v(r[e[0]], e[1]);
  }, function(r, e, t) {
    return A(r.slice(e, e + t[1]));
  }], ["accessTime", 12, 476, function(r, e) {
    return k(r[e[0]], e[1]);
  }, function(r, e, t) {
    return z(r.slice(e, e + t[1]));
  }], ["createTime", 12, 488, function(r, e) {
    return k(r[e[0]], e[1]);
  }, function(r, e, t) {
    return z(r.slice(e, e + t[1]));
  }]], $ = function(r) {
    var e = r[r.length - 1];
    return e[2] + e[1];
  }(_2);
  function br(r) {
    if (r.length == 8) {
      var e = r.split("");
      if (e[5] == p2.NULL_CHAR)
        return (e[6] == " " || e[6] == p2.NULL_CHAR) && (e[6] = "0"), (e[7] == " " || e[7] == p2.NULL_CHAR) && (e[7] = "0"), e = e.join(""), e == p2.TMAGIC ? e : r;
      if (e[7] == p2.NULL_CHAR)
        return e[5] == p2.NULL_CHAR && (e[5] = " "), e[6] == p2.NULL_CHAR && (e[6] = " "), e == p2.OLDGNU_MAGIC ? e : r;
    }
    return r;
  }
  function v(r, e) {
    return e -= 1, K.isUndefined(r) && (r = ""), r = ("" + r).substr(0, e), r + p2.NULL_CHAR;
  }
  function P2(r, e, t) {
    for (t = parseInt(t) || 0, e -= 1, r = (parseInt(r) || t).toString(8).substr(-e, e);r.length < e; )
      r = "0" + r;
    return r + p2.NULL_CHAR;
  }
  function k(r, e) {
    if (K.isDateTime(r))
      r = Math.floor(1 * r / 1000);
    else if (r = parseInt(r, 10), isFinite(r)) {
      if (r <= 0)
        return "";
    } else
      r = Math.floor(1 * new Date / 1000);
    return P2(r, e, 0);
  }
  function A(r, e) {
    var t = String.fromCharCode.apply(null, r);
    if (e)
      return t;
    var s2 = t.indexOf(p2.NULL_CHAR);
    return s2 >= 0 ? t.substr(0, s2) : t;
  }
  function S(r) {
    var e = String.fromCharCode.apply(null, r);
    return parseInt(e.replace(/^0+$/g, ""), 8) || 0;
  }
  function z(r) {
    return r.length == 0 || r[0] == 0 ? null : new Date(1000 * S(r));
  }
  function Tr(r, e, t) {
    var s2 = parseInt(e, 10) || 0, a2 = Math.min(s2 + $, r.length), n2 = 0, o = 0, i2 = 0;
    t && _2.every(function(y) {
      return y[0] == "checksum" ? (o = s2 + y[2], i2 = o + y[1], false) : true;
    });
    for (var u2 = 32, c2 = s2;c2 < a2; c2++) {
      var m = c2 >= o && c2 < i2 ? u2 : r[c2];
      n2 = (n2 + m) % 262144;
    }
    return n2;
  }
  f2.exports.recordSize = Fr;
  f2.exports.defaultFileMode = I;
  f2.exports.defaultUid = V;
  f2.exports.defaultGid = Z;
  f2.exports.posixHeader = _2;
  f2.exports.effectiveHeaderSize = $;
  f2.exports.calculateChecksum = Tr;
  f2.exports.formatTarString = v;
  f2.exports.formatTarNumber = P2;
  f2.exports.formatTarDateTime = k;
  f2.exports.parseTarString = A;
  f2.exports.parseTarNumber = S;
  f2.exports.parseTarDateTime = z;
});
var er = D((ne, rr) => {
  u();
  var Ar = x2(), O = w(), F2 = L2();
  function J(r) {
    return F2.recordSize;
  }
  function Q(r) {
    return Math.ceil(r.data.length / F2.recordSize) * F2.recordSize;
  }
  function Er(r) {
    var e = 0;
    return r.forEach(function(t) {
      e += J(t) + Q(t);
    }), e += F2.recordSize * 2, new Uint8Array(e);
  }
  function Pr(r, e, t) {
    t = parseInt(t) || 0;
    var s2 = t;
    F2.posixHeader.forEach(function(u2) {
      for (var c2 = u2[3](e, u2), m = c2.length, y = 0;y < m; y += 1)
        r[s2 + y] = c2.charCodeAt(y) & 255;
      s2 += u2[1];
    });
    var a2 = O.find(F2.posixHeader, function(u2) {
      return u2[0] == "checksum";
    });
    if (a2) {
      var n2 = F2.calculateChecksum(r, t, true), o = F2.formatTarNumber(n2, a2[1] - 2) + Ar.NULL_CHAR + " ";
      s2 = t + a2[2];
      for (var i2 = 0;i2 < o.length; i2 += 1)
        r[s2] = o.charCodeAt(i2) & 255, s2++;
    }
    return t + J(e);
  }
  function wr(r, e, t) {
    return t = parseInt(t, 10) || 0, r.set(e.data, t), t + Q(e);
  }
  function xr(r) {
    r = O.map(r, function(s2) {
      return O.extend({}, s2, { data: O.toUint8Array(s2.data) });
    });
    var e = Er(r), t = 0;
    return r.forEach(function(s2) {
      t = Pr(e, s2, t), t = wr(e, s2, t);
    }), e;
  }
  rr.exports.tar = xr;
});
var nr = D((oe, tr) => {
  u();
  var vr = x2(), G = w(), h2 = L2(), Nr = { extractData: true, checkHeader: true, checkChecksum: true, checkFileSize: true }, Ur = { size: true, checksum: true, ustar: true }, R2 = { unexpectedEndOfFile: "Unexpected end of file.", fileCorrupted: "File is corrupted.", checksumCheckFailed: "Checksum check failed." };
  function kr(r) {
    return h2.recordSize;
  }
  function zr(r) {
    return Math.ceil(r / h2.recordSize) * h2.recordSize;
  }
  function Or(r, e) {
    for (var t = e, s2 = Math.min(r.length, e + h2.recordSize * 2), a2 = t;a2 < s2; a2++)
      if (r[a2] != 0)
        return false;
    return true;
  }
  function Cr(r, e, t) {
    if (r.length - e < h2.recordSize) {
      if (t.checkFileSize)
        throw new Error(R2.unexpectedEndOfFile);
      return null;
    }
    e = parseInt(e) || 0;
    var s2 = {}, a2 = e;
    if (h2.posixHeader.forEach(function(i2) {
      s2[i2[0]] = i2[4](r, a2, i2), a2 += i2[1];
    }), s2.type != 0 && (s2.size = 0), t.checkHeader && h2.posixHeader.forEach(function(i2) {
      if (G.isFunction(i2[5]) && !i2[5](s2, i2)) {
        var u2 = new Error(R2.fileCorrupted);
        throw u2.data = { offset: e + i2[2], field: i2[0] }, u2;
      }
    }), t.checkChecksum) {
      var n2 = h2.calculateChecksum(r, e, true);
      if (n2 != s2.checksum) {
        var o = new Error(R2.checksumCheckFailed);
        throw o.data = { offset: e, header: s2, checksum: n2 }, o;
      }
    }
    return s2;
  }
  function Dr(r, e, t, s2) {
    return s2.extractData ? t.size <= 0 ? new Uint8Array : r.slice(e, e + t.size) : null;
  }
  function Mr(r, e) {
    var t = {};
    return h2.posixHeader.forEach(function(s2) {
      var a2 = s2[0];
      Ur[a2] || (t[a2] = r[a2]);
    }), t.isOldGNUFormat = r.ustar == vr.OLDGNU_MAGIC, e && (t.data = e), t;
  }
  function Ir(r, e) {
    e = G.extend({}, Nr, e);
    for (var t = [], s2 = 0, a2 = r.length;a2 - s2 >= h2.recordSize; ) {
      r = G.toUint8Array(r);
      var n2 = Cr(r, s2, e);
      if (!n2)
        break;
      s2 += kr(n2);
      var o = Dr(r, s2, n2, e);
      if (t.push(Mr(n2, o)), s2 += zr(n2.size), Or(r, s2))
        break;
    }
    return t;
  }
  tr.exports.untar = Ir;
});
var or = D((se, ir) => {
  u();
  var _r = w(), Lr = x2(), Rr = er(), Gr = nr();
  _r.extend(ir.exports, Rr, Gr, Lr);
});
u();
u();
var g2 = L(or(), 1);
async function H(r, e, t = "pgdata", s2 = "auto") {
  let a2 = Br(r, e), [n2, o] = await qr(a2, s2), i2 = t + (o ? ".tar.gz" : ".tar"), u2 = o ? "application/x-gzip" : "application/x-tar";
  return typeof File < "u" ? new File([n2], i2, { type: u2 }) : new Blob([n2], { type: u2 });
}
var Hr = ["application/x-gtar", "application/x-tar+gzip", "application/x-gzip", "application/gzip"];
async function ce(r, e, t) {
  let s2 = new Uint8Array(await e.arrayBuffer()), a2 = typeof File < "u" && e instanceof File ? e.name : undefined;
  (Hr.includes(e.type) || a2?.endsWith(".tgz") || a2?.endsWith(".tar.gz")) && (s2 = await ar(s2));
  let o;
  try {
    o = (0, g2.untar)(s2);
  } catch (i2) {
    if (i2 instanceof Error && i2.message.includes("File is corrupted"))
      s2 = await ar(s2), o = (0, g2.untar)(s2);
    else
      throw i2;
  }
  for (let i2 of o) {
    let u2 = t + i2.name, c2 = u2.split("/").slice(0, -1);
    for (let m = 1;m <= c2.length; m++) {
      let y = c2.slice(0, m).join("/");
      r.analyzePath(y).exists || r.mkdir(y);
    }
    i2.type === g2.REGTYPE ? (r.writeFile(u2, i2.data), r.utime(u2, sr(i2.modifyTime), sr(i2.modifyTime))) : i2.type === g2.DIRTYPE && r.mkdir(u2);
  }
}
function jr(r, e) {
  let t = [], s2 = (a2) => {
    r.readdir(a2).forEach((o) => {
      if (o === "." || o === "..")
        return;
      let i2 = a2 + "/" + o, u2 = r.stat(i2), c2 = r.isFile(u2.mode) ? r.readFile(i2, { encoding: "binary" }) : new Uint8Array(0);
      t.push({ name: i2.substring(e.length), mode: u2.mode, size: u2.size, type: r.isFile(u2.mode) ? g2.REGTYPE : g2.DIRTYPE, modifyTime: u2.mtime, data: c2 }), r.isDir(u2.mode) && s2(i2);
    });
  };
  return s2(e), t;
}
function Br(r, e) {
  let t = jr(r, e);
  return (0, g2.tar)(t);
}
async function qr(r, e = "auto") {
  if (e === "none")
    return [r, false];
  if (typeof CompressionStream < "u")
    return [await Yr(r), true];
  if (typeof process < "u" && process.versions && process.versions.node)
    return [await Wr(r), true];
  if (e === "auto")
    return [r, false];
  throw new Error("Compression not supported in this environment");
}
async function Yr(r) {
  let e = new CompressionStream("gzip"), t = e.writable.getWriter(), s2 = e.readable.getReader();
  t.write(r), t.close();
  let a2 = [];
  for (;; ) {
    let { value: i2, done: u2 } = await s2.read();
    if (u2)
      break;
    i2 && a2.push(i2);
  }
  let n2 = new Uint8Array(a2.reduce((i2, u2) => i2 + u2.length, 0)), o = 0;
  return a2.forEach((i2) => {
    n2.set(i2, o), o += i2.length;
  }), n2;
}
async function Wr(r) {
  let { promisify: e } = await import("./index-ecd5wekt.js"), { gzip: t } = await import("./index-z7xxh162.js");
  return await e(t)(r);
}
async function ar(r) {
  if (typeof CompressionStream < "u")
    return await Xr(r);
  if (typeof process < "u" && process.versions && process.versions.node)
    return await Kr(r);
  throw new Error("Unsupported environment for decompression");
}
async function Xr(r) {
  let e = new DecompressionStream("gzip"), t = e.writable.getWriter(), s2 = e.readable.getReader();
  t.write(r), t.close();
  let a2 = [];
  for (;; ) {
    let { value: i2, done: u2 } = await s2.read();
    if (u2)
      break;
    i2 && a2.push(i2);
  }
  let n2 = new Uint8Array(a2.reduce((i2, u2) => i2 + u2.length, 0)), o = 0;
  return a2.forEach((i2) => {
    n2.set(i2, o), o += i2.length;
  }), n2;
}
async function Kr(r) {
  let { promisify: e } = await import("./index-ecd5wekt.js"), { gunzip: t } = await import("./index-z7xxh162.js");
  return await e(t)(r);
}
function sr(r) {
  return r ? typeof r == "number" ? r : Math.floor(r.getTime() / 1000) : Math.floor(Date.now() / 1000);
}
var Vr = "/tmp/pglite";
var C = Vr + "/base";
var ur = class {
  constructor(e) {
    this.dataDir = e;
  }
  async init(e, t) {
    return this.pg = e, { emscriptenOpts: t };
  }
  async syncToFs(e) {
  }
  async initialSyncFs() {
  }
  async closeFs() {
  }
  async dumpTar(e, t) {
    return H(this.pg.Module.FS, C, e, t);
  }
};
var cr = class {
  constructor(e, { debug: t = false } = {}) {
    this.dataDir = e, this.debug = t;
  }
  async syncToFs(e) {
  }
  async initialSyncFs() {
  }
  async closeFs() {
  }
  async dumpTar(e, t) {
    return H(this.pg.Module.FS, C, e, t);
  }
  async init(e, t) {
    return this.pg = e, { emscriptenOpts: { ...t, preRun: [...t.preRun || [], (a2) => {
      let n2 = Zr(a2, this);
      a2.FS.mkdir(C), a2.FS.mount(n2, {}, C);
    }] } };
  }
};
var pr = { EBADF: 8, EBADFD: 127, EEXIST: 20, EINVAL: 28, EISDIR: 31, ENODEV: 43, ENOENT: 44, ENOTDIR: 54, ENOTEMPTY: 55 };
var Zr = (r, e) => {
  let t = r.FS, s2 = e.debug ? console.log : null, a2 = { tryFSOperation(n2) {
    try {
      return n2();
    } catch (o) {
      throw o.code ? o.code === "UNKNOWN" ? new t.ErrnoError(pr.EINVAL) : new t.ErrnoError(o.code) : o;
    }
  }, mount(n2) {
    return a2.createNode(null, "/", 16895, 0);
  }, syncfs(n2, o, i2) {
  }, createNode(n2, o, i2, u2) {
    if (!t.isDir(i2) && !t.isFile(i2))
      throw new t.ErrnoError(28);
    let c2 = t.createNode(n2, o, i2);
    return c2.node_ops = a2.node_ops, c2.stream_ops = a2.stream_ops, c2;
  }, getMode: function(n2) {
    return s2?.("getMode", n2), a2.tryFSOperation(() => e.lstat(n2).mode);
  }, realPath: function(n2) {
    let o = [];
    for (;n2.parent !== n2; )
      o.push(n2.name), n2 = n2.parent;
    return o.push(n2.mount.opts.root), o.reverse(), o.join("/");
  }, node_ops: { getattr(n2) {
    s2?.("getattr", a2.realPath(n2));
    let o = a2.realPath(n2);
    return a2.tryFSOperation(() => {
      let i2 = e.lstat(o);
      return { ...i2, dev: 0, ino: n2.id, nlink: 1, rdev: n2.rdev, atime: new Date(i2.atime), mtime: new Date(i2.mtime), ctime: new Date(i2.ctime) };
    });
  }, setattr(n2, o) {
    s2?.("setattr", a2.realPath(n2), o);
    let i2 = a2.realPath(n2);
    a2.tryFSOperation(() => {
      o.mode !== undefined && e.chmod(i2, o.mode), o.size !== undefined && e.truncate(i2, o.size), o.timestamp !== undefined && e.utimes(i2, o.timestamp, o.timestamp), o.size !== undefined && e.truncate(i2, o.size);
    });
  }, lookup(n2, o) {
    s2?.("lookup", a2.realPath(n2), o);
    let i2 = [a2.realPath(n2), o].join("/"), u2 = a2.getMode(i2);
    return a2.createNode(n2, o, u2);
  }, mknod(n2, o, i2, u2) {
    s2?.("mknod", a2.realPath(n2), o, i2, u2);
    let c2 = a2.createNode(n2, o, i2, u2), m = a2.realPath(c2);
    return a2.tryFSOperation(() => (t.isDir(c2.mode) ? e.mkdir(m, { mode: i2 }) : e.writeFile(m, "", { mode: i2 }), c2));
  }, rename(n2, o, i2) {
    s2?.("rename", a2.realPath(n2), a2.realPath(o), i2);
    let u2 = a2.realPath(n2), c2 = [a2.realPath(o), i2].join("/");
    a2.tryFSOperation(() => {
      e.rename(u2, c2);
    }), n2.name = i2;
  }, unlink(n2, o) {
    s2?.("unlink", a2.realPath(n2), o);
    let i2 = [a2.realPath(n2), o].join("/");
    try {
      e.unlink(i2);
    } catch {
    }
  }, rmdir(n2, o) {
    s2?.("rmdir", a2.realPath(n2), o);
    let i2 = [a2.realPath(n2), o].join("/");
    return a2.tryFSOperation(() => {
      e.rmdir(i2);
    });
  }, readdir(n2) {
    s2?.("readdir", a2.realPath(n2));
    let o = a2.realPath(n2);
    return a2.tryFSOperation(() => e.readdir(o));
  }, symlink(n2, o, i2) {
    throw s2?.("symlink", a2.realPath(n2), o, i2), new t.ErrnoError(63);
  }, readlink(n2) {
    throw s2?.("readlink", a2.realPath(n2)), new t.ErrnoError(63);
  } }, stream_ops: { open(n2) {
    s2?.("open stream", a2.realPath(n2.node));
    let o = a2.realPath(n2.node);
    return a2.tryFSOperation(() => {
      t.isFile(n2.node.mode) && (n2.shared.refcount = 1, n2.nfd = e.open(o));
    });
  }, close(n2) {
    return s2?.("close stream", a2.realPath(n2.node)), a2.tryFSOperation(() => {
      t.isFile(n2.node.mode) && n2.nfd && --n2.shared.refcount === 0 && e.close(n2.nfd);
    });
  }, dup(n2) {
    s2?.("dup stream", a2.realPath(n2.node)), n2.shared.refcount++;
  }, read(n2, o, i2, u2, c2) {
    return s2?.("read stream", a2.realPath(n2.node), i2, u2, c2), u2 === 0 ? 0 : a2.tryFSOperation(() => e.read(n2.nfd, o, i2, u2, c2));
  }, write(n2, o, i2, u2, c2) {
    return s2?.("write stream", a2.realPath(n2.node), i2, u2, c2), a2.tryFSOperation(() => e.write(n2.nfd, o.buffer, i2, u2, c2));
  }, llseek(n2, o, i2) {
    s2?.("llseek stream", a2.realPath(n2.node), o, i2);
    let u2 = o;
    if (i2 === 1 ? u2 += n2.position : i2 === 2 && t.isFile(n2.node.mode) && a2.tryFSOperation(() => {
      let c2 = e.fstat(n2.nfd);
      u2 += c2.size;
    }), u2 < 0)
      throw new t.ErrnoError(28);
    return u2;
  }, mmap(n2, o, i2, u2, c2) {
    if (s2?.("mmap stream", a2.realPath(n2.node), o, i2, u2, c2), !t.isFile(n2.node.mode))
      throw new t.ErrnoError(pr.ENODEV);
    let m = r.mmapAlloc(o);
    return a2.stream_ops.read(n2, r.HEAP8, m, o, i2), { ptr: m, allocated: true };
  }, msync(n2, o, i2, u2, c2) {
    return s2?.("msync stream", a2.realPath(n2.node), i2, u2, c2), a2.stream_ops.write(n2, o, 0, u2, i2), 0;
  } } };
  return a2;
};

export { F, L, P, h, R, x, T, U, u, or, ce, Vr, C, ur, cr, pr };
