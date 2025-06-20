import"./chat-client-13a4mv5g.js";

// node:stream
var al = Object.create;
var tt = Object.defineProperty;
var cl = Object.getOwnPropertyDescriptor;
var dl = Object.getOwnPropertyNames;
var hl = Object.getPrototypeOf;
var pl = Object.prototype.hasOwnProperty;
var yl = (e, t) => () => (e && (t = e(e = 0)), t);
var E = (e, t) => () => (t || e((t = { exports: {} }).exports, t), t.exports);
var Qr = (e, t) => {
  for (var r in t)
    tt(e, r, { get: t[r], enumerable: true });
};
var et = (e, t, r, n) => {
  if (t && typeof t == "object" || typeof t == "function")
    for (let i of dl(t))
      !pl.call(e, i) && i !== r && tt(e, i, { get: () => t[i], enumerable: !(n = cl(t, i)) || n.enumerable });
  return e;
};
var ue = (e, t, r) => (et(e, t, "default"), r && et(r, t, "default"));
var rt = (e, t, r) => (r = e != null ? al(hl(e)) : {}, et(t || !e || !e.__esModule ? tt(r, "default", { value: e, enumerable: true }) : r, e));
var pe = (e) => et(tt({}, "__esModule", { value: true }), e);
var tn = E((nt) => {
  nt.byteLength = bl;
  nt.toByteArray = _l;
  nt.fromByteArray = ml;
  var G = [], P = [], wl = typeof Uint8Array < "u" ? Uint8Array : Array, Wt = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  for (ye = 0, Zr = Wt.length;ye < Zr; ++ye)
    G[ye] = Wt[ye], P[Wt.charCodeAt(ye)] = ye;
  var ye, Zr;
  P[45] = 62;
  P[95] = 63;
  function en(e) {
    var t = e.length;
    if (t % 4 > 0)
      throw new Error("Invalid string. Length must be a multiple of 4");
    var r = e.indexOf("=");
    r === -1 && (r = t);
    var n = r === t ? 0 : 4 - r % 4;
    return [r, n];
  }
  function bl(e) {
    var t = en(e), r = t[0], n = t[1];
    return (r + n) * 3 / 4 - n;
  }
  function gl(e, t, r) {
    return (t + r) * 3 / 4 - r;
  }
  function _l(e) {
    var t, r = en(e), n = r[0], i = r[1], o = new wl(gl(e, n, i)), l = 0, u = i > 0 ? n - 4 : n, f;
    for (f = 0;f < u; f += 4)
      t = P[e.charCodeAt(f)] << 18 | P[e.charCodeAt(f + 1)] << 12 | P[e.charCodeAt(f + 2)] << 6 | P[e.charCodeAt(f + 3)], o[l++] = t >> 16 & 255, o[l++] = t >> 8 & 255, o[l++] = t & 255;
    return i === 2 && (t = P[e.charCodeAt(f)] << 2 | P[e.charCodeAt(f + 1)] >> 4, o[l++] = t & 255), i === 1 && (t = P[e.charCodeAt(f)] << 10 | P[e.charCodeAt(f + 1)] << 4 | P[e.charCodeAt(f + 2)] >> 2, o[l++] = t >> 8 & 255, o[l++] = t & 255), o;
  }
  function El(e) {
    return G[e >> 18 & 63] + G[e >> 12 & 63] + G[e >> 6 & 63] + G[e & 63];
  }
  function Sl(e, t, r) {
    for (var n, i = [], o = t;o < r; o += 3)
      n = (e[o] << 16 & 16711680) + (e[o + 1] << 8 & 65280) + (e[o + 2] & 255), i.push(El(n));
    return i.join("");
  }
  function ml(e) {
    for (var t, r = e.length, n = r % 3, i = [], o = 16383, l = 0, u = r - n;l < u; l += o)
      i.push(Sl(e, l, l + o > u ? u : l + o));
    return n === 1 ? (t = e[r - 1], i.push(G[t >> 2] + G[t << 4 & 63] + "==")) : n === 2 && (t = (e[r - 2] << 8) + e[r - 1], i.push(G[t >> 10] + G[t >> 4 & 63] + G[t << 2 & 63] + "=")), i.join("");
  }
});
var rn = E(($t) => {
  $t.read = function(e, t, r, n, i) {
    var o, l, u = i * 8 - n - 1, f = (1 << u) - 1, s = f >> 1, d = -7, c = r ? i - 1 : 0, y = r ? -1 : 1, h = e[t + c];
    for (c += y, o = h & (1 << -d) - 1, h >>= -d, d += u;d > 0; o = o * 256 + e[t + c], c += y, d -= 8)
      ;
    for (l = o & (1 << -d) - 1, o >>= -d, d += n;d > 0; l = l * 256 + e[t + c], c += y, d -= 8)
      ;
    if (o === 0)
      o = 1 - s;
    else {
      if (o === f)
        return l ? NaN : (h ? -1 : 1) * (1 / 0);
      l = l + Math.pow(2, n), o = o - s;
    }
    return (h ? -1 : 1) * l * Math.pow(2, o - n);
  };
  $t.write = function(e, t, r, n, i, o) {
    var l, u, f, s = o * 8 - i - 1, d = (1 << s) - 1, c = d >> 1, y = i === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0, h = n ? 0 : o - 1, p = n ? 1 : -1, B = t < 0 || t === 0 && 1 / t < 0 ? 1 : 0;
    for (t = Math.abs(t), isNaN(t) || t === 1 / 0 ? (u = isNaN(t) ? 1 : 0, l = d) : (l = Math.floor(Math.log(t) / Math.LN2), t * (f = Math.pow(2, -l)) < 1 && (l--, f *= 2), l + c >= 1 ? t += y / f : t += y * Math.pow(2, 1 - c), t * f >= 2 && (l++, f /= 2), l + c >= d ? (u = 0, l = d) : l + c >= 1 ? (u = (t * f - 1) * Math.pow(2, i), l = l + c) : (u = t * Math.pow(2, c - 1) * Math.pow(2, i), l = 0));i >= 8; e[r + h] = u & 255, h += p, u /= 256, i -= 8)
      ;
    for (l = l << i | u, s += i;s > 0; e[r + h] = l & 255, h += p, l /= 256, s -= 8)
      ;
    e[r + h - p] |= B * 128;
  };
});
var te = E((Fe) => {
  var jt = tn(), Le = rn(), nn = typeof Symbol == "function" && typeof Symbol.for == "function" ? Symbol.for("nodejs.util.inspect.custom") : null;
  Fe.Buffer = a;
  Fe.SlowBuffer = Bl;
  Fe.INSPECT_MAX_BYTES = 50;
  var it = 2147483647;
  Fe.kMaxLength = it;
  a.TYPED_ARRAY_SUPPORT = xl();
  !a.TYPED_ARRAY_SUPPORT && typeof console < "u" && typeof console.error == "function" && console.error("This browser lacks typed array (Uint8Array) support which is required by `buffer` v5.x. Use `buffer` v4.x if you require old browser support.");
  function xl() {
    try {
      let e = new Uint8Array(1), t = { foo: function() {
        return 42;
      } };
      return Object.setPrototypeOf(t, Uint8Array.prototype), Object.setPrototypeOf(e, t), e.foo() === 42;
    } catch {
      return false;
    }
  }
  Object.defineProperty(a.prototype, "parent", { enumerable: true, get: function() {
    if (!!a.isBuffer(this))
      return this.buffer;
  } });
  Object.defineProperty(a.prototype, "offset", { enumerable: true, get: function() {
    if (!!a.isBuffer(this))
      return this.byteOffset;
  } });
  function ee(e) {
    if (e > it)
      throw new RangeError('The value "' + e + '" is invalid for option "size"');
    let t = new Uint8Array(e);
    return Object.setPrototypeOf(t, a.prototype), t;
  }
  function a(e, t, r) {
    if (typeof e == "number") {
      if (typeof t == "string")
        throw new TypeError('The "string" argument must be of type string. Received type number');
      return Yt(e);
    }
    return fn(e, t, r);
  }
  a.poolSize = 8192;
  function fn(e, t, r) {
    if (typeof e == "string")
      return Al(e, t);
    if (ArrayBuffer.isView(e))
      return Il(e);
    if (e == null)
      throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof e);
    if (H(e, ArrayBuffer) || e && H(e.buffer, ArrayBuffer) || typeof SharedArrayBuffer < "u" && (H(e, SharedArrayBuffer) || e && H(e.buffer, SharedArrayBuffer)))
      return Ht(e, t, r);
    if (typeof e == "number")
      throw new TypeError('The "value" argument must not be of type number. Received type number');
    let n = e.valueOf && e.valueOf();
    if (n != null && n !== e)
      return a.from(n, t, r);
    let i = Tl(e);
    if (i)
      return i;
    if (typeof Symbol < "u" && Symbol.toPrimitive != null && typeof e[Symbol.toPrimitive] == "function")
      return a.from(e[Symbol.toPrimitive]("string"), t, r);
    throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof e);
  }
  a.from = function(e, t, r) {
    return fn(e, t, r);
  };
  Object.setPrototypeOf(a.prototype, Uint8Array.prototype);
  Object.setPrototypeOf(a, Uint8Array);
  function sn(e) {
    if (typeof e != "number")
      throw new TypeError('"size" argument must be of type number');
    if (e < 0)
      throw new RangeError('The value "' + e + '" is invalid for option "size"');
  }
  function Rl(e, t, r) {
    return sn(e), e <= 0 ? ee(e) : t !== undefined ? typeof r == "string" ? ee(e).fill(t, r) : ee(e).fill(t) : ee(e);
  }
  a.alloc = function(e, t, r) {
    return Rl(e, t, r);
  };
  function Yt(e) {
    return sn(e), ee(e < 0 ? 0 : Kt(e) | 0);
  }
  a.allocUnsafe = function(e) {
    return Yt(e);
  };
  a.allocUnsafeSlow = function(e) {
    return Yt(e);
  };
  function Al(e, t) {
    if ((typeof t != "string" || t === "") && (t = "utf8"), !a.isEncoding(t))
      throw new TypeError("Unknown encoding: " + t);
    let r = an(e, t) | 0, n = ee(r), i = n.write(e, t);
    return i !== r && (n = n.slice(0, i)), n;
  }
  function Gt(e) {
    let t = e.length < 0 ? 0 : Kt(e.length) | 0, r = ee(t);
    for (let n = 0;n < t; n += 1)
      r[n] = e[n] & 255;
    return r;
  }
  function Il(e) {
    if (H(e, Uint8Array)) {
      let t = new Uint8Array(e);
      return Ht(t.buffer, t.byteOffset, t.byteLength);
    }
    return Gt(e);
  }
  function Ht(e, t, r) {
    if (t < 0 || e.byteLength < t)
      throw new RangeError('"offset" is outside of buffer bounds');
    if (e.byteLength < t + (r || 0))
      throw new RangeError('"length" is outside of buffer bounds');
    let n;
    return t === undefined && r === undefined ? n = new Uint8Array(e) : r === undefined ? n = new Uint8Array(e, t) : n = new Uint8Array(e, t, r), Object.setPrototypeOf(n, a.prototype), n;
  }
  function Tl(e) {
    if (a.isBuffer(e)) {
      let t = Kt(e.length) | 0, r = ee(t);
      return r.length === 0 || e.copy(r, 0, 0, t), r;
    }
    if (e.length !== undefined)
      return typeof e.length != "number" || Xt(e.length) ? ee(0) : Gt(e);
    if (e.type === "Buffer" && Array.isArray(e.data))
      return Gt(e.data);
  }
  function Kt(e) {
    if (e >= it)
      throw new RangeError("Attempt to allocate Buffer larger than maximum size: 0x" + it.toString(16) + " bytes");
    return e | 0;
  }
  function Bl(e) {
    return +e != e && (e = 0), a.alloc(+e);
  }
  a.isBuffer = function(t) {
    return t != null && t._isBuffer === true && t !== a.prototype;
  };
  a.compare = function(t, r) {
    if (H(t, Uint8Array) && (t = a.from(t, t.offset, t.byteLength)), H(r, Uint8Array) && (r = a.from(r, r.offset, r.byteLength)), !a.isBuffer(t) || !a.isBuffer(r))
      throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
    if (t === r)
      return 0;
    let n = t.length, i = r.length;
    for (let o = 0, l = Math.min(n, i);o < l; ++o)
      if (t[o] !== r[o]) {
        n = t[o], i = r[o];
        break;
      }
    return n < i ? -1 : i < n ? 1 : 0;
  };
  a.isEncoding = function(t) {
    switch (String(t).toLowerCase()) {
      case "hex":
      case "utf8":
      case "utf-8":
      case "ascii":
      case "latin1":
      case "binary":
      case "base64":
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
        return true;
      default:
        return false;
    }
  };
  a.concat = function(t, r) {
    if (!Array.isArray(t))
      throw new TypeError('"list" argument must be an Array of Buffers');
    if (t.length === 0)
      return a.alloc(0);
    let n;
    if (r === undefined)
      for (r = 0, n = 0;n < t.length; ++n)
        r += t[n].length;
    let i = a.allocUnsafe(r), o = 0;
    for (n = 0;n < t.length; ++n) {
      let l = t[n];
      if (H(l, Uint8Array))
        o + l.length > i.length ? (a.isBuffer(l) || (l = a.from(l)), l.copy(i, o)) : Uint8Array.prototype.set.call(i, l, o);
      else if (a.isBuffer(l))
        l.copy(i, o);
      else
        throw new TypeError('"list" argument must be an Array of Buffers');
      o += l.length;
    }
    return i;
  };
  function an(e, t) {
    if (a.isBuffer(e))
      return e.length;
    if (ArrayBuffer.isView(e) || H(e, ArrayBuffer))
      return e.byteLength;
    if (typeof e != "string")
      throw new TypeError('The "string" argument must be one of type string, Buffer, or ArrayBuffer. Received type ' + typeof e);
    let r = e.length, n = arguments.length > 2 && arguments[2] === true;
    if (!n && r === 0)
      return 0;
    let i = false;
    for (;; )
      switch (t) {
        case "ascii":
        case "latin1":
        case "binary":
          return r;
        case "utf8":
        case "utf-8":
          return Vt(e).length;
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return r * 2;
        case "hex":
          return r >>> 1;
        case "base64":
          return _n(e).length;
        default:
          if (i)
            return n ? -1 : Vt(e).length;
          t = ("" + t).toLowerCase(), i = true;
      }
  }
  a.byteLength = an;
  function Ll(e, t, r) {
    let n = false;
    if ((t === undefined || t < 0) && (t = 0), t > this.length || ((r === undefined || r > this.length) && (r = this.length), r <= 0) || (r >>>= 0, t >>>= 0, r <= t))
      return "";
    for (e || (e = "utf8");; )
      switch (e) {
        case "hex":
          return vl(this, t, r);
        case "utf8":
        case "utf-8":
          return dn(this, t, r);
        case "ascii":
          return kl(this, t, r);
        case "latin1":
        case "binary":
          return Ul(this, t, r);
        case "base64":
          return Dl(this, t, r);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return ql(this, t, r);
        default:
          if (n)
            throw new TypeError("Unknown encoding: " + e);
          e = (e + "").toLowerCase(), n = true;
      }
  }
  a.prototype._isBuffer = true;
  function we(e, t, r) {
    let n = e[t];
    e[t] = e[r], e[r] = n;
  }
  a.prototype.swap16 = function() {
    let t = this.length;
    if (t % 2 !== 0)
      throw new RangeError("Buffer size must be a multiple of 16-bits");
    for (let r = 0;r < t; r += 2)
      we(this, r, r + 1);
    return this;
  };
  a.prototype.swap32 = function() {
    let t = this.length;
    if (t % 4 !== 0)
      throw new RangeError("Buffer size must be a multiple of 32-bits");
    for (let r = 0;r < t; r += 4)
      we(this, r, r + 3), we(this, r + 1, r + 2);
    return this;
  };
  a.prototype.swap64 = function() {
    let t = this.length;
    if (t % 8 !== 0)
      throw new RangeError("Buffer size must be a multiple of 64-bits");
    for (let r = 0;r < t; r += 8)
      we(this, r, r + 7), we(this, r + 1, r + 6), we(this, r + 2, r + 5), we(this, r + 3, r + 4);
    return this;
  };
  a.prototype.toString = function() {
    let t = this.length;
    return t === 0 ? "" : arguments.length === 0 ? dn(this, 0, t) : Ll.apply(this, arguments);
  };
  a.prototype.toLocaleString = a.prototype.toString;
  a.prototype.equals = function(t) {
    if (!a.isBuffer(t))
      throw new TypeError("Argument must be a Buffer");
    return this === t ? true : a.compare(this, t) === 0;
  };
  a.prototype.inspect = function() {
    let t = "", r = Fe.INSPECT_MAX_BYTES;
    return t = this.toString("hex", 0, r).replace(/(.{2})/g, "$1 ").trim(), this.length > r && (t += " ... "), "<Buffer " + t + ">";
  };
  nn && (a.prototype[nn] = a.prototype.inspect);
  a.prototype.compare = function(t, r, n, i, o) {
    if (H(t, Uint8Array) && (t = a.from(t, t.offset, t.byteLength)), !a.isBuffer(t))
      throw new TypeError('The "target" argument must be one of type Buffer or Uint8Array. Received type ' + typeof t);
    if (r === undefined && (r = 0), n === undefined && (n = t ? t.length : 0), i === undefined && (i = 0), o === undefined && (o = this.length), r < 0 || n > t.length || i < 0 || o > this.length)
      throw new RangeError("out of range index");
    if (i >= o && r >= n)
      return 0;
    if (i >= o)
      return -1;
    if (r >= n)
      return 1;
    if (r >>>= 0, n >>>= 0, i >>>= 0, o >>>= 0, this === t)
      return 0;
    let l = o - i, u = n - r, f = Math.min(l, u), s = this.slice(i, o), d = t.slice(r, n);
    for (let c = 0;c < f; ++c)
      if (s[c] !== d[c]) {
        l = s[c], u = d[c];
        break;
      }
    return l < u ? -1 : u < l ? 1 : 0;
  };
  function cn(e, t, r, n, i) {
    if (e.length === 0)
      return -1;
    if (typeof r == "string" ? (n = r, r = 0) : r > 2147483647 ? r = 2147483647 : r < -2147483648 && (r = -2147483648), r = +r, Xt(r) && (r = i ? 0 : e.length - 1), r < 0 && (r = e.length + r), r >= e.length) {
      if (i)
        return -1;
      r = e.length - 1;
    } else if (r < 0)
      if (i)
        r = 0;
      else
        return -1;
    if (typeof t == "string" && (t = a.from(t, n)), a.isBuffer(t))
      return t.length === 0 ? -1 : on(e, t, r, n, i);
    if (typeof t == "number")
      return t = t & 255, typeof Uint8Array.prototype.indexOf == "function" ? i ? Uint8Array.prototype.indexOf.call(e, t, r) : Uint8Array.prototype.lastIndexOf.call(e, t, r) : on(e, [t], r, n, i);
    throw new TypeError("val must be string, number or Buffer");
  }
  function on(e, t, r, n, i) {
    let o = 1, l = e.length, u = t.length;
    if (n !== undefined && (n = String(n).toLowerCase(), n === "ucs2" || n === "ucs-2" || n === "utf16le" || n === "utf-16le")) {
      if (e.length < 2 || t.length < 2)
        return -1;
      o = 2, l /= 2, u /= 2, r /= 2;
    }
    function f(d, c) {
      return o === 1 ? d[c] : d.readUInt16BE(c * o);
    }
    let s;
    if (i) {
      let d = -1;
      for (s = r;s < l; s++)
        if (f(e, s) === f(t, d === -1 ? 0 : s - d)) {
          if (d === -1 && (d = s), s - d + 1 === u)
            return d * o;
        } else
          d !== -1 && (s -= s - d), d = -1;
    } else
      for (r + u > l && (r = l - u), s = r;s >= 0; s--) {
        let d = true;
        for (let c = 0;c < u; c++)
          if (f(e, s + c) !== f(t, c)) {
            d = false;
            break;
          }
        if (d)
          return s;
      }
    return -1;
  }
  a.prototype.includes = function(t, r, n) {
    return this.indexOf(t, r, n) !== -1;
  };
  a.prototype.indexOf = function(t, r, n) {
    return cn(this, t, r, n, true);
  };
  a.prototype.lastIndexOf = function(t, r, n) {
    return cn(this, t, r, n, false);
  };
  function Nl(e, t, r, n) {
    r = Number(r) || 0;
    let i = e.length - r;
    n ? (n = Number(n), n > i && (n = i)) : n = i;
    let o = t.length;
    n > o / 2 && (n = o / 2);
    let l;
    for (l = 0;l < n; ++l) {
      let u = parseInt(t.substr(l * 2, 2), 16);
      if (Xt(u))
        return l;
      e[r + l] = u;
    }
    return l;
  }
  function Fl(e, t, r, n) {
    return ot(Vt(t, e.length - r), e, r, n);
  }
  function Ml(e, t, r, n) {
    return ot(Gl(t), e, r, n);
  }
  function Cl(e, t, r, n) {
    return ot(_n(t), e, r, n);
  }
  function Ol(e, t, r, n) {
    return ot(Hl(t, e.length - r), e, r, n);
  }
  a.prototype.write = function(t, r, n, i) {
    if (r === undefined)
      i = "utf8", n = this.length, r = 0;
    else if (n === undefined && typeof r == "string")
      i = r, n = this.length, r = 0;
    else if (isFinite(r))
      r = r >>> 0, isFinite(n) ? (n = n >>> 0, i === undefined && (i = "utf8")) : (i = n, n = undefined);
    else
      throw new Error("Buffer.write(string, encoding, offset[, length]) is no longer supported");
    let o = this.length - r;
    if ((n === undefined || n > o) && (n = o), t.length > 0 && (n < 0 || r < 0) || r > this.length)
      throw new RangeError("Attempt to write outside buffer bounds");
    i || (i = "utf8");
    let l = false;
    for (;; )
      switch (i) {
        case "hex":
          return Nl(this, t, r, n);
        case "utf8":
        case "utf-8":
          return Fl(this, t, r, n);
        case "ascii":
        case "latin1":
        case "binary":
          return Ml(this, t, r, n);
        case "base64":
          return Cl(this, t, r, n);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return Ol(this, t, r, n);
        default:
          if (l)
            throw new TypeError("Unknown encoding: " + i);
          i = ("" + i).toLowerCase(), l = true;
      }
  };
  a.prototype.toJSON = function() {
    return { type: "Buffer", data: Array.prototype.slice.call(this._arr || this, 0) };
  };
  function Dl(e, t, r) {
    return t === 0 && r === e.length ? jt.fromByteArray(e) : jt.fromByteArray(e.slice(t, r));
  }
  function dn(e, t, r) {
    r = Math.min(e.length, r);
    let n = [], i = t;
    for (;i < r; ) {
      let o = e[i], l = null, u = o > 239 ? 4 : o > 223 ? 3 : o > 191 ? 2 : 1;
      if (i + u <= r) {
        let f, s, d, c;
        switch (u) {
          case 1:
            o < 128 && (l = o);
            break;
          case 2:
            f = e[i + 1], (f & 192) === 128 && (c = (o & 31) << 6 | f & 63, c > 127 && (l = c));
            break;
          case 3:
            f = e[i + 1], s = e[i + 2], (f & 192) === 128 && (s & 192) === 128 && (c = (o & 15) << 12 | (f & 63) << 6 | s & 63, c > 2047 && (c < 55296 || c > 57343) && (l = c));
            break;
          case 4:
            f = e[i + 1], s = e[i + 2], d = e[i + 3], (f & 192) === 128 && (s & 192) === 128 && (d & 192) === 128 && (c = (o & 15) << 18 | (f & 63) << 12 | (s & 63) << 6 | d & 63, c > 65535 && c < 1114112 && (l = c));
        }
      }
      l === null ? (l = 65533, u = 1) : l > 65535 && (l -= 65536, n.push(l >>> 10 & 1023 | 55296), l = 56320 | l & 1023), n.push(l), i += u;
    }
    return Pl(n);
  }
  var ln = 4096;
  function Pl(e) {
    let t = e.length;
    if (t <= ln)
      return String.fromCharCode.apply(String, e);
    let r = "", n = 0;
    for (;n < t; )
      r += String.fromCharCode.apply(String, e.slice(n, n += ln));
    return r;
  }
  function kl(e, t, r) {
    let n = "";
    r = Math.min(e.length, r);
    for (let i = t;i < r; ++i)
      n += String.fromCharCode(e[i] & 127);
    return n;
  }
  function Ul(e, t, r) {
    let n = "";
    r = Math.min(e.length, r);
    for (let i = t;i < r; ++i)
      n += String.fromCharCode(e[i]);
    return n;
  }
  function vl(e, t, r) {
    let n = e.length;
    (!t || t < 0) && (t = 0), (!r || r < 0 || r > n) && (r = n);
    let i = "";
    for (let o = t;o < r; ++o)
      i += Vl[e[o]];
    return i;
  }
  function ql(e, t, r) {
    let n = e.slice(t, r), i = "";
    for (let o = 0;o < n.length - 1; o += 2)
      i += String.fromCharCode(n[o] + n[o + 1] * 256);
    return i;
  }
  a.prototype.slice = function(t, r) {
    let n = this.length;
    t = ~~t, r = r === undefined ? n : ~~r, t < 0 ? (t += n, t < 0 && (t = 0)) : t > n && (t = n), r < 0 ? (r += n, r < 0 && (r = 0)) : r > n && (r = n), r < t && (r = t);
    let i = this.subarray(t, r);
    return Object.setPrototypeOf(i, a.prototype), i;
  };
  function F(e, t, r) {
    if (e % 1 !== 0 || e < 0)
      throw new RangeError("offset is not uint");
    if (e + t > r)
      throw new RangeError("Trying to access beyond buffer length");
  }
  a.prototype.readUintLE = a.prototype.readUIntLE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || F(t, r, this.length);
    let i = this[t], o = 1, l = 0;
    for (;++l < r && (o *= 256); )
      i += this[t + l] * o;
    return i;
  };
  a.prototype.readUintBE = a.prototype.readUIntBE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || F(t, r, this.length);
    let i = this[t + --r], o = 1;
    for (;r > 0 && (o *= 256); )
      i += this[t + --r] * o;
    return i;
  };
  a.prototype.readUint8 = a.prototype.readUInt8 = function(t, r) {
    return t = t >>> 0, r || F(t, 1, this.length), this[t];
  };
  a.prototype.readUint16LE = a.prototype.readUInt16LE = function(t, r) {
    return t = t >>> 0, r || F(t, 2, this.length), this[t] | this[t + 1] << 8;
  };
  a.prototype.readUint16BE = a.prototype.readUInt16BE = function(t, r) {
    return t = t >>> 0, r || F(t, 2, this.length), this[t] << 8 | this[t + 1];
  };
  a.prototype.readUint32LE = a.prototype.readUInt32LE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), (this[t] | this[t + 1] << 8 | this[t + 2] << 16) + this[t + 3] * 16777216;
  };
  a.prototype.readUint32BE = a.prototype.readUInt32BE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), this[t] * 16777216 + (this[t + 1] << 16 | this[t + 2] << 8 | this[t + 3]);
  };
  a.prototype.readBigUInt64LE = fe(function(t) {
    t = t >>> 0, Ne(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && Ge(t, this.length - 8);
    let i = r + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + this[++t] * 2 ** 24, o = this[++t] + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + n * 2 ** 24;
    return BigInt(i) + (BigInt(o) << BigInt(32));
  });
  a.prototype.readBigUInt64BE = fe(function(t) {
    t = t >>> 0, Ne(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && Ge(t, this.length - 8);
    let i = r * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + this[++t], o = this[++t] * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + n;
    return (BigInt(i) << BigInt(32)) + BigInt(o);
  });
  a.prototype.readIntLE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || F(t, r, this.length);
    let i = this[t], o = 1, l = 0;
    for (;++l < r && (o *= 256); )
      i += this[t + l] * o;
    return o *= 128, i >= o && (i -= Math.pow(2, 8 * r)), i;
  };
  a.prototype.readIntBE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || F(t, r, this.length);
    let i = r, o = 1, l = this[t + --i];
    for (;i > 0 && (o *= 256); )
      l += this[t + --i] * o;
    return o *= 128, l >= o && (l -= Math.pow(2, 8 * r)), l;
  };
  a.prototype.readInt8 = function(t, r) {
    return t = t >>> 0, r || F(t, 1, this.length), this[t] & 128 ? (255 - this[t] + 1) * -1 : this[t];
  };
  a.prototype.readInt16LE = function(t, r) {
    t = t >>> 0, r || F(t, 2, this.length);
    let n = this[t] | this[t + 1] << 8;
    return n & 32768 ? n | 4294901760 : n;
  };
  a.prototype.readInt16BE = function(t, r) {
    t = t >>> 0, r || F(t, 2, this.length);
    let n = this[t + 1] | this[t] << 8;
    return n & 32768 ? n | 4294901760 : n;
  };
  a.prototype.readInt32LE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), this[t] | this[t + 1] << 8 | this[t + 2] << 16 | this[t + 3] << 24;
  };
  a.prototype.readInt32BE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), this[t] << 24 | this[t + 1] << 16 | this[t + 2] << 8 | this[t + 3];
  };
  a.prototype.readBigInt64LE = fe(function(t) {
    t = t >>> 0, Ne(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && Ge(t, this.length - 8);
    let i = this[t + 4] + this[t + 5] * 2 ** 8 + this[t + 6] * 2 ** 16 + (n << 24);
    return (BigInt(i) << BigInt(32)) + BigInt(r + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + this[++t] * 2 ** 24);
  });
  a.prototype.readBigInt64BE = fe(function(t) {
    t = t >>> 0, Ne(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && Ge(t, this.length - 8);
    let i = (r << 24) + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + this[++t];
    return (BigInt(i) << BigInt(32)) + BigInt(this[++t] * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + n);
  });
  a.prototype.readFloatLE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), Le.read(this, t, true, 23, 4);
  };
  a.prototype.readFloatBE = function(t, r) {
    return t = t >>> 0, r || F(t, 4, this.length), Le.read(this, t, false, 23, 4);
  };
  a.prototype.readDoubleLE = function(t, r) {
    return t = t >>> 0, r || F(t, 8, this.length), Le.read(this, t, true, 52, 8);
  };
  a.prototype.readDoubleBE = function(t, r) {
    return t = t >>> 0, r || F(t, 8, this.length), Le.read(this, t, false, 52, 8);
  };
  function O(e, t, r, n, i, o) {
    if (!a.isBuffer(e))
      throw new TypeError('"buffer" argument must be a Buffer instance');
    if (t > i || t < o)
      throw new RangeError('"value" argument is out of bounds');
    if (r + n > e.length)
      throw new RangeError("Index out of range");
  }
  a.prototype.writeUintLE = a.prototype.writeUIntLE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, n = n >>> 0, !i) {
      let u = Math.pow(2, 8 * n) - 1;
      O(this, t, r, n, u, 0);
    }
    let o = 1, l = 0;
    for (this[r] = t & 255;++l < n && (o *= 256); )
      this[r + l] = t / o & 255;
    return r + n;
  };
  a.prototype.writeUintBE = a.prototype.writeUIntBE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, n = n >>> 0, !i) {
      let u = Math.pow(2, 8 * n) - 1;
      O(this, t, r, n, u, 0);
    }
    let o = n - 1, l = 1;
    for (this[r + o] = t & 255;--o >= 0 && (l *= 256); )
      this[r + o] = t / l & 255;
    return r + n;
  };
  a.prototype.writeUint8 = a.prototype.writeUInt8 = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 1, 255, 0), this[r] = t & 255, r + 1;
  };
  a.prototype.writeUint16LE = a.prototype.writeUInt16LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 2, 65535, 0), this[r] = t & 255, this[r + 1] = t >>> 8, r + 2;
  };
  a.prototype.writeUint16BE = a.prototype.writeUInt16BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 2, 65535, 0), this[r] = t >>> 8, this[r + 1] = t & 255, r + 2;
  };
  a.prototype.writeUint32LE = a.prototype.writeUInt32LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 4, 4294967295, 0), this[r + 3] = t >>> 24, this[r + 2] = t >>> 16, this[r + 1] = t >>> 8, this[r] = t & 255, r + 4;
  };
  a.prototype.writeUint32BE = a.prototype.writeUInt32BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 4, 4294967295, 0), this[r] = t >>> 24, this[r + 1] = t >>> 16, this[r + 2] = t >>> 8, this[r + 3] = t & 255, r + 4;
  };
  function hn(e, t, r, n, i) {
    gn(t, n, i, e, r, 7);
    let o = Number(t & BigInt(4294967295));
    e[r++] = o, o = o >> 8, e[r++] = o, o = o >> 8, e[r++] = o, o = o >> 8, e[r++] = o;
    let l = Number(t >> BigInt(32) & BigInt(4294967295));
    return e[r++] = l, l = l >> 8, e[r++] = l, l = l >> 8, e[r++] = l, l = l >> 8, e[r++] = l, r;
  }
  function pn(e, t, r, n, i) {
    gn(t, n, i, e, r, 7);
    let o = Number(t & BigInt(4294967295));
    e[r + 7] = o, o = o >> 8, e[r + 6] = o, o = o >> 8, e[r + 5] = o, o = o >> 8, e[r + 4] = o;
    let l = Number(t >> BigInt(32) & BigInt(4294967295));
    return e[r + 3] = l, l = l >> 8, e[r + 2] = l, l = l >> 8, e[r + 1] = l, l = l >> 8, e[r] = l, r + 8;
  }
  a.prototype.writeBigUInt64LE = fe(function(t, r = 0) {
    return hn(this, t, r, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  a.prototype.writeBigUInt64BE = fe(function(t, r = 0) {
    return pn(this, t, r, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  a.prototype.writeIntLE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, !i) {
      let f = Math.pow(2, 8 * n - 1);
      O(this, t, r, n, f - 1, -f);
    }
    let o = 0, l = 1, u = 0;
    for (this[r] = t & 255;++o < n && (l *= 256); )
      t < 0 && u === 0 && this[r + o - 1] !== 0 && (u = 1), this[r + o] = (t / l >> 0) - u & 255;
    return r + n;
  };
  a.prototype.writeIntBE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, !i) {
      let f = Math.pow(2, 8 * n - 1);
      O(this, t, r, n, f - 1, -f);
    }
    let o = n - 1, l = 1, u = 0;
    for (this[r + o] = t & 255;--o >= 0 && (l *= 256); )
      t < 0 && u === 0 && this[r + o + 1] !== 0 && (u = 1), this[r + o] = (t / l >> 0) - u & 255;
    return r + n;
  };
  a.prototype.writeInt8 = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 1, 127, -128), t < 0 && (t = 255 + t + 1), this[r] = t & 255, r + 1;
  };
  a.prototype.writeInt16LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 2, 32767, -32768), this[r] = t & 255, this[r + 1] = t >>> 8, r + 2;
  };
  a.prototype.writeInt16BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 2, 32767, -32768), this[r] = t >>> 8, this[r + 1] = t & 255, r + 2;
  };
  a.prototype.writeInt32LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 4, 2147483647, -2147483648), this[r] = t & 255, this[r + 1] = t >>> 8, this[r + 2] = t >>> 16, this[r + 3] = t >>> 24, r + 4;
  };
  a.prototype.writeInt32BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || O(this, t, r, 4, 2147483647, -2147483648), t < 0 && (t = 4294967295 + t + 1), this[r] = t >>> 24, this[r + 1] = t >>> 16, this[r + 2] = t >>> 8, this[r + 3] = t & 255, r + 4;
  };
  a.prototype.writeBigInt64LE = fe(function(t, r = 0) {
    return hn(this, t, r, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  a.prototype.writeBigInt64BE = fe(function(t, r = 0) {
    return pn(this, t, r, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  function yn(e, t, r, n, i, o) {
    if (r + n > e.length)
      throw new RangeError("Index out of range");
    if (r < 0)
      throw new RangeError("Index out of range");
  }
  function wn(e, t, r, n, i) {
    return t = +t, r = r >>> 0, i || yn(e, t, r, 4, 340282346638528860000000000000000000000, -340282346638528860000000000000000000000), Le.write(e, t, r, n, 23, 4), r + 4;
  }
  a.prototype.writeFloatLE = function(t, r, n) {
    return wn(this, t, r, true, n);
  };
  a.prototype.writeFloatBE = function(t, r, n) {
    return wn(this, t, r, false, n);
  };
  function bn(e, t, r, n, i) {
    return t = +t, r = r >>> 0, i || yn(e, t, r, 8, 179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000, -179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000), Le.write(e, t, r, n, 52, 8), r + 8;
  }
  a.prototype.writeDoubleLE = function(t, r, n) {
    return bn(this, t, r, true, n);
  };
  a.prototype.writeDoubleBE = function(t, r, n) {
    return bn(this, t, r, false, n);
  };
  a.prototype.copy = function(t, r, n, i) {
    if (!a.isBuffer(t))
      throw new TypeError("argument should be a Buffer");
    if (n || (n = 0), !i && i !== 0 && (i = this.length), r >= t.length && (r = t.length), r || (r = 0), i > 0 && i < n && (i = n), i === n || t.length === 0 || this.length === 0)
      return 0;
    if (r < 0)
      throw new RangeError("targetStart out of bounds");
    if (n < 0 || n >= this.length)
      throw new RangeError("Index out of range");
    if (i < 0)
      throw new RangeError("sourceEnd out of bounds");
    i > this.length && (i = this.length), t.length - r < i - n && (i = t.length - r + n);
    let o = i - n;
    return this === t && typeof Uint8Array.prototype.copyWithin == "function" ? this.copyWithin(r, n, i) : Uint8Array.prototype.set.call(t, this.subarray(n, i), r), o;
  };
  a.prototype.fill = function(t, r, n, i) {
    if (typeof t == "string") {
      if (typeof r == "string" ? (i = r, r = 0, n = this.length) : typeof n == "string" && (i = n, n = this.length), i !== undefined && typeof i != "string")
        throw new TypeError("encoding must be a string");
      if (typeof i == "string" && !a.isEncoding(i))
        throw new TypeError("Unknown encoding: " + i);
      if (t.length === 1) {
        let l = t.charCodeAt(0);
        (i === "utf8" && l < 128 || i === "latin1") && (t = l);
      }
    } else
      typeof t == "number" ? t = t & 255 : typeof t == "boolean" && (t = Number(t));
    if (r < 0 || this.length < r || this.length < n)
      throw new RangeError("Out of range index");
    if (n <= r)
      return this;
    r = r >>> 0, n = n === undefined ? this.length : n >>> 0, t || (t = 0);
    let o;
    if (typeof t == "number")
      for (o = r;o < n; ++o)
        this[o] = t;
    else {
      let l = a.isBuffer(t) ? t : a.from(t, i), u = l.length;
      if (u === 0)
        throw new TypeError('The value "' + t + '" is invalid for argument "value"');
      for (o = 0;o < n - r; ++o)
        this[o + r] = l[o % u];
    }
    return this;
  };
  var Be = {};
  function zt(e, t, r) {
    Be[e] = class extends r {
      constructor() {
        super(), Object.defineProperty(this, "message", { value: t.apply(this, arguments), writable: true, configurable: true }), this.name = `${this.name} [${e}]`, this.stack, delete this.name;
      }
      get code() {
        return e;
      }
      set code(i) {
        Object.defineProperty(this, "code", { configurable: true, enumerable: true, value: i, writable: true });
      }
      toString() {
        return `${this.name} [${e}]: ${this.message}`;
      }
    };
  }
  zt("ERR_BUFFER_OUT_OF_BOUNDS", function(e) {
    return e ? `${e} is outside of buffer bounds` : "Attempt to access memory outside buffer bounds";
  }, RangeError);
  zt("ERR_INVALID_ARG_TYPE", function(e, t) {
    return `The "${e}" argument must be of type number. Received type ${typeof t}`;
  }, TypeError);
  zt("ERR_OUT_OF_RANGE", function(e, t, r) {
    let n = `The value of "${e}" is out of range.`, i = r;
    return Number.isInteger(r) && Math.abs(r) > 2 ** 32 ? i = un(String(r)) : typeof r == "bigint" && (i = String(r), (r > BigInt(2) ** BigInt(32) || r < -(BigInt(2) ** BigInt(32))) && (i = un(i)), i += "n"), n += ` It must be ${t}. Received ${i}`, n;
  }, RangeError);
  function un(e) {
    let t = "", r = e.length, n = e[0] === "-" ? 1 : 0;
    for (;r >= n + 4; r -= 3)
      t = `_${e.slice(r - 3, r)}${t}`;
    return `${e.slice(0, r)}${t}`;
  }
  function Wl(e, t, r) {
    Ne(t, "offset"), (e[t] === undefined || e[t + r] === undefined) && Ge(t, e.length - (r + 1));
  }
  function gn(e, t, r, n, i, o) {
    if (e > r || e < t) {
      let l = typeof t == "bigint" ? "n" : "", u;
      throw o > 3 ? t === 0 || t === BigInt(0) ? u = `>= 0${l} and < 2${l} ** ${(o + 1) * 8}${l}` : u = `>= -(2${l} ** ${(o + 1) * 8 - 1}${l}) and < 2 ** ${(o + 1) * 8 - 1}${l}` : u = `>= ${t}${l} and <= ${r}${l}`, new Be.ERR_OUT_OF_RANGE("value", u, e);
    }
    Wl(n, i, o);
  }
  function Ne(e, t) {
    if (typeof e != "number")
      throw new Be.ERR_INVALID_ARG_TYPE(t, "number", e);
  }
  function Ge(e, t, r) {
    throw Math.floor(e) !== e ? (Ne(e, r), new Be.ERR_OUT_OF_RANGE(r || "offset", "an integer", e)) : t < 0 ? new Be.ERR_BUFFER_OUT_OF_BOUNDS : new Be.ERR_OUT_OF_RANGE(r || "offset", `>= ${r ? 1 : 0} and <= ${t}`, e);
  }
  var $l = /[^+/0-9A-Za-z-_]/g;
  function jl(e) {
    if (e = e.split("=")[0], e = e.trim().replace($l, ""), e.length < 2)
      return "";
    for (;e.length % 4 !== 0; )
      e = e + "=";
    return e;
  }
  function Vt(e, t) {
    t = t || 1 / 0;
    let r, n = e.length, i = null, o = [];
    for (let l = 0;l < n; ++l) {
      if (r = e.charCodeAt(l), r > 55295 && r < 57344) {
        if (!i) {
          if (r > 56319) {
            (t -= 3) > -1 && o.push(239, 191, 189);
            continue;
          } else if (l + 1 === n) {
            (t -= 3) > -1 && o.push(239, 191, 189);
            continue;
          }
          i = r;
          continue;
        }
        if (r < 56320) {
          (t -= 3) > -1 && o.push(239, 191, 189), i = r;
          continue;
        }
        r = (i - 55296 << 10 | r - 56320) + 65536;
      } else
        i && (t -= 3) > -1 && o.push(239, 191, 189);
      if (i = null, r < 128) {
        if ((t -= 1) < 0)
          break;
        o.push(r);
      } else if (r < 2048) {
        if ((t -= 2) < 0)
          break;
        o.push(r >> 6 | 192, r & 63 | 128);
      } else if (r < 65536) {
        if ((t -= 3) < 0)
          break;
        o.push(r >> 12 | 224, r >> 6 & 63 | 128, r & 63 | 128);
      } else if (r < 1114112) {
        if ((t -= 4) < 0)
          break;
        o.push(r >> 18 | 240, r >> 12 & 63 | 128, r >> 6 & 63 | 128, r & 63 | 128);
      } else
        throw new Error("Invalid code point");
    }
    return o;
  }
  function Gl(e) {
    let t = [];
    for (let r = 0;r < e.length; ++r)
      t.push(e.charCodeAt(r) & 255);
    return t;
  }
  function Hl(e, t) {
    let r, n, i, o = [];
    for (let l = 0;l < e.length && !((t -= 2) < 0); ++l)
      r = e.charCodeAt(l), n = r >> 8, i = r % 256, o.push(i), o.push(n);
    return o;
  }
  function _n(e) {
    return jt.toByteArray(jl(e));
  }
  function ot(e, t, r, n) {
    let i;
    for (i = 0;i < n && !(i + r >= t.length || i >= e.length); ++i)
      t[i + r] = e[i];
    return i;
  }
  function H(e, t) {
    return e instanceof t || e != null && e.constructor != null && e.constructor.name != null && e.constructor.name === t.name;
  }
  function Xt(e) {
    return e !== e;
  }
  var Vl = function() {
    let e = "0123456789abcdef", t = new Array(256);
    for (let r = 0;r < 16; ++r) {
      let n = r * 16;
      for (let i = 0;i < 16; ++i)
        t[n + i] = e[r] + e[i];
    }
    return t;
  }();
  function fe(e) {
    return typeof BigInt > "u" ? Yl : e;
  }
  function Yl() {
    throw new Error("BigInt not supported");
  }
});
var I = E((Gc, En) => {
  En.exports = { ArrayIsArray(e) {
    return Array.isArray(e);
  }, ArrayPrototypeIncludes(e, t) {
    return e.includes(t);
  }, ArrayPrototypeIndexOf(e, t) {
    return e.indexOf(t);
  }, ArrayPrototypeJoin(e, t) {
    return e.join(t);
  }, ArrayPrototypeMap(e, t) {
    return e.map(t);
  }, ArrayPrototypePop(e, t) {
    return e.pop(t);
  }, ArrayPrototypePush(e, t) {
    return e.push(t);
  }, ArrayPrototypeSlice(e, t, r) {
    return e.slice(t, r);
  }, Error, FunctionPrototypeCall(e, t, ...r) {
    return e.call(t, ...r);
  }, FunctionPrototypeSymbolHasInstance(e, t) {
    return Function.prototype[Symbol.hasInstance].call(e, t);
  }, MathFloor: Math.floor, Number, NumberIsInteger: Number.isInteger, NumberIsNaN: Number.isNaN, NumberMAX_SAFE_INTEGER: Number.MAX_SAFE_INTEGER, NumberMIN_SAFE_INTEGER: Number.MIN_SAFE_INTEGER, NumberParseInt: Number.parseInt, ObjectDefineProperties(e, t) {
    return Object.defineProperties(e, t);
  }, ObjectDefineProperty(e, t, r) {
    return Object.defineProperty(e, t, r);
  }, ObjectGetOwnPropertyDescriptor(e, t) {
    return Object.getOwnPropertyDescriptor(e, t);
  }, ObjectKeys(e) {
    return Object.keys(e);
  }, ObjectSetPrototypeOf(e, t) {
    return Object.setPrototypeOf(e, t);
  }, Promise, PromisePrototypeCatch(e, t) {
    return e.catch(t);
  }, PromisePrototypeThen(e, t, r) {
    return e.then(t, r);
  }, PromiseReject(e) {
    return Promise.reject(e);
  }, ReflectApply: Reflect.apply, RegExpPrototypeTest(e, t) {
    return e.test(t);
  }, SafeSet: Set, String, StringPrototypeSlice(e, t, r) {
    return e.slice(t, r);
  }, StringPrototypeToLowerCase(e) {
    return e.toLowerCase();
  }, StringPrototypeToUpperCase(e) {
    return e.toUpperCase();
  }, StringPrototypeTrim(e) {
    return e.trim();
  }, Symbol, SymbolAsyncIterator: Symbol.asyncIterator, SymbolHasInstance: Symbol.hasInstance, SymbolIterator: Symbol.iterator, TypedArrayPrototypeSet(e, t, r) {
    return e.set(t, r);
  }, Uint8Array };
});
var V = E((Hc, Qt) => {
  var Kl = te(), zl = Object.getPrototypeOf(async function() {
  }).constructor, Sn = globalThis.Blob || Kl.Blob, Xl = typeof Sn < "u" ? function(t) {
    return t instanceof Sn;
  } : function(t) {
    return false;
  }, Jt = class extends Error {
    constructor(t) {
      if (!Array.isArray(t))
        throw new TypeError(`Expected input to be an Array, got ${typeof t}`);
      let r = "";
      for (let n = 0;n < t.length; n++)
        r += `    ${t[n].stack}
`;
      super(r), this.name = "AggregateError", this.errors = t;
    }
  };
  Qt.exports = { AggregateError: Jt, kEmptyObject: Object.freeze({}), once(e) {
    let t = false;
    return function(...r) {
      t || (t = true, e.apply(this, r));
    };
  }, createDeferredPromise: function() {
    let e, t;
    return { promise: new Promise((n, i) => {
      e = n, t = i;
    }), resolve: e, reject: t };
  }, promisify(e) {
    return new Promise((t, r) => {
      e((n, ...i) => n ? r(n) : t(...i));
    });
  }, debuglog() {
    return function() {
    };
  }, format(e, ...t) {
    return e.replace(/%([sdifj])/g, function(...[r, n]) {
      let i = t.shift();
      return n === "f" ? i.toFixed(6) : n === "j" ? JSON.stringify(i) : n === "s" && typeof i == "object" ? `${i.constructor !== Object ? i.constructor.name : ""} {}`.trim() : i.toString();
    });
  }, inspect(e) {
    switch (typeof e) {
      case "string":
        if (e.includes("'"))
          if (e.includes('"')) {
            if (!e.includes("`") && !e.includes("${"))
              return `\`${e}\``;
          } else
            return `"${e}"`;
        return `'${e}'`;
      case "number":
        return isNaN(e) ? "NaN" : Object.is(e, -0) ? String(e) : e;
      case "bigint":
        return `${String(e)}n`;
      case "boolean":
      case "undefined":
        return String(e);
      case "object":
        return "{}";
    }
  }, types: { isAsyncFunction(e) {
    return e instanceof zl;
  }, isArrayBufferView(e) {
    return ArrayBuffer.isView(e);
  } }, isBlob: Xl };
  Qt.exports.promisify.custom = Symbol.for("nodejs.util.promisify.custom");
});
var ut = E((Vc, lt) => {
  var { AbortController: mn, AbortSignal: Jl } = typeof self < "u" ? self : typeof window < "u" ? window : undefined;
  lt.exports = mn;
  lt.exports.AbortSignal = Jl;
  lt.exports.default = mn;
});
var C = E((Yc, An) => {
  var { format: Ql, inspect: ft, AggregateError: Zl } = V(), eu = globalThis.AggregateError || Zl, tu = Symbol("kIsNodeError"), ru = ["string", "function", "number", "object", "Function", "Object", "boolean", "bigint", "symbol"], nu = /^([A-Z][a-z0-9]*)+$/, iu = "__node_internal_", st = {};
  function be(e, t) {
    if (!e)
      throw new st.ERR_INTERNAL_ASSERTION(t);
  }
  function xn(e) {
    let t = "", r = e.length, n = e[0] === "-" ? 1 : 0;
    for (;r >= n + 4; r -= 3)
      t = `_${e.slice(r - 3, r)}${t}`;
    return `${e.slice(0, r)}${t}`;
  }
  function ou(e, t, r) {
    if (typeof t == "function")
      return be(t.length <= r.length, `Code: ${e}; The provided arguments length (${r.length}) does not match the required ones (${t.length}).`), t(...r);
    let n = (t.match(/%[dfijoOs]/g) || []).length;
    return be(n === r.length, `Code: ${e}; The provided arguments length (${r.length}) does not match the required ones (${n}).`), r.length === 0 ? t : Ql(t, ...r);
  }
  function M(e, t, r) {
    r || (r = Error);

    class n extends r {
      constructor(...o) {
        super(ou(e, t, o));
      }
      toString() {
        return `${this.name} [${e}]: ${this.message}`;
      }
    }
    Object.defineProperties(n.prototype, { name: { value: r.name, writable: true, enumerable: false, configurable: true }, toString: { value() {
      return `${this.name} [${e}]: ${this.message}`;
    }, writable: true, enumerable: false, configurable: true } }), n.prototype.code = e, n.prototype[tu] = true, st[e] = n;
  }
  function Rn(e) {
    let t = iu + e.name;
    return Object.defineProperty(e, "name", { value: t }), e;
  }
  function lu(e, t) {
    if (e && t && e !== t) {
      if (Array.isArray(t.errors))
        return t.errors.push(e), t;
      let r = new eu([t, e], t.message);
      return r.code = t.code, r;
    }
    return e || t;
  }
  var Zt = class extends Error {
    constructor(t = "The operation was aborted", r = undefined) {
      if (r !== undefined && typeof r != "object")
        throw new st.ERR_INVALID_ARG_TYPE("options", "Object", r);
      super(t, r), this.code = "ABORT_ERR", this.name = "AbortError";
    }
  };
  M("ERR_ASSERTION", "%s", Error);
  M("ERR_INVALID_ARG_TYPE", (e, t, r) => {
    be(typeof e == "string", "'name' must be a string"), Array.isArray(t) || (t = [t]);
    let n = "The ";
    e.endsWith(" argument") ? n += `${e} ` : n += `"${e}" ${e.includes(".") ? "property" : "argument"} `, n += "must be ";
    let i = [], o = [], l = [];
    for (let f of t)
      be(typeof f == "string", "All expected entries have to be of type string"), ru.includes(f) ? i.push(f.toLowerCase()) : nu.test(f) ? o.push(f) : (be(f !== "object", 'The value "object" should be written as "Object"'), l.push(f));
    if (o.length > 0) {
      let f = i.indexOf("object");
      f !== -1 && (i.splice(i, f, 1), o.push("Object"));
    }
    if (i.length > 0) {
      switch (i.length) {
        case 1:
          n += `of type ${i[0]}`;
          break;
        case 2:
          n += `one of type ${i[0]} or ${i[1]}`;
          break;
        default: {
          let f = i.pop();
          n += `one of type ${i.join(", ")}, or ${f}`;
        }
      }
      (o.length > 0 || l.length > 0) && (n += " or ");
    }
    if (o.length > 0) {
      switch (o.length) {
        case 1:
          n += `an instance of ${o[0]}`;
          break;
        case 2:
          n += `an instance of ${o[0]} or ${o[1]}`;
          break;
        default: {
          let f = o.pop();
          n += `an instance of ${o.join(", ")}, or ${f}`;
        }
      }
      l.length > 0 && (n += " or ");
    }
    switch (l.length) {
      case 0:
        break;
      case 1:
        l[0].toLowerCase() !== l[0] && (n += "an "), n += `${l[0]}`;
        break;
      case 2:
        n += `one of ${l[0]} or ${l[1]}`;
        break;
      default: {
        let f = l.pop();
        n += `one of ${l.join(", ")}, or ${f}`;
      }
    }
    if (r == null)
      n += `. Received ${r}`;
    else if (typeof r == "function" && r.name)
      n += `. Received function ${r.name}`;
    else if (typeof r == "object") {
      var u;
      (u = r.constructor) !== null && u !== undefined && u.name ? n += `. Received an instance of ${r.constructor.name}` : n += `. Received ${ft(r, { depth: -1 })}`;
    } else {
      let f = ft(r, { colors: false });
      f.length > 25 && (f = `${f.slice(0, 25)}...`), n += `. Received type ${typeof r} (${f})`;
    }
    return n;
  }, TypeError);
  M("ERR_INVALID_ARG_VALUE", (e, t, r = "is invalid") => {
    let n = ft(t);
    return n.length > 128 && (n = n.slice(0, 128) + "..."), `The ${e.includes(".") ? "property" : "argument"} '${e}' ${r}. Received ${n}`;
  }, TypeError);
  M("ERR_INVALID_RETURN_VALUE", (e, t, r) => {
    var n;
    let i = r != null && (n = r.constructor) !== null && n !== undefined && n.name ? `instance of ${r.constructor.name}` : `type ${typeof r}`;
    return `Expected ${e} to be returned from the "${t}" function but got ${i}.`;
  }, TypeError);
  M("ERR_MISSING_ARGS", (...e) => {
    be(e.length > 0, "At least one arg needs to be specified");
    let t, r = e.length;
    switch (e = (Array.isArray(e) ? e : [e]).map((n) => `"${n}"`).join(" or "), r) {
      case 1:
        t += `The ${e[0]} argument`;
        break;
      case 2:
        t += `The ${e[0]} and ${e[1]} arguments`;
        break;
      default:
        {
          let n = e.pop();
          t += `The ${e.join(", ")}, and ${n} arguments`;
        }
        break;
    }
    return `${t} must be specified`;
  }, TypeError);
  M("ERR_OUT_OF_RANGE", (e, t, r) => {
    be(t, 'Missing "range" argument');
    let n;
    return Number.isInteger(r) && Math.abs(r) > 2 ** 32 ? n = xn(String(r)) : typeof r == "bigint" ? (n = String(r), (r > 2n ** 32n || r < -(2n ** 32n)) && (n = xn(n)), n += "n") : n = ft(r), `The value of "${e}" is out of range. It must be ${t}. Received ${n}`;
  }, RangeError);
  M("ERR_MULTIPLE_CALLBACK", "Callback called multiple times", Error);
  M("ERR_METHOD_NOT_IMPLEMENTED", "The %s method is not implemented", Error);
  M("ERR_STREAM_ALREADY_FINISHED", "Cannot call %s after a stream was finished", Error);
  M("ERR_STREAM_CANNOT_PIPE", "Cannot pipe, not readable", Error);
  M("ERR_STREAM_DESTROYED", "Cannot call %s after a stream was destroyed", Error);
  M("ERR_STREAM_NULL_VALUES", "May not write null values to stream", TypeError);
  M("ERR_STREAM_PREMATURE_CLOSE", "Premature close", Error);
  M("ERR_STREAM_PUSH_AFTER_EOF", "stream.push() after EOF", Error);
  M("ERR_STREAM_UNSHIFT_AFTER_END_EVENT", "stream.unshift() after end event", Error);
  M("ERR_STREAM_WRITE_AFTER_END", "write after end", Error);
  M("ERR_UNKNOWN_ENCODING", "Unknown encoding: %s", TypeError);
  An.exports = { AbortError: Zt, aggregateTwoErrors: Rn(lu), hideStackFrames: Rn, codes: st };
});
var He = E((Kc, Cn) => {
  var { ArrayIsArray: Bn, ArrayPrototypeIncludes: Ln, ArrayPrototypeJoin: Nn, ArrayPrototypeMap: uu, NumberIsInteger: tr, NumberIsNaN: fu, NumberMAX_SAFE_INTEGER: su, NumberMIN_SAFE_INTEGER: au, NumberParseInt: cu, ObjectPrototypeHasOwnProperty: du, RegExpPrototypeExec: hu, String: pu, StringPrototypeToUpperCase: yu, StringPrototypeTrim: wu } = I(), { hideStackFrames: W, codes: { ERR_SOCKET_BAD_PORT: bu, ERR_INVALID_ARG_TYPE: D, ERR_INVALID_ARG_VALUE: at, ERR_OUT_OF_RANGE: ge, ERR_UNKNOWN_SIGNAL: In } } = C(), { normalizeEncoding: gu } = V(), { isAsyncFunction: _u, isArrayBufferView: Eu } = V().types, Tn = {};
  function Su(e) {
    return e === (e | 0);
  }
  function mu(e) {
    return e === e >>> 0;
  }
  var xu = /^[0-7]+$/, Ru = "must be a 32-bit unsigned integer or an octal string";
  function Au(e, t, r) {
    if (typeof e > "u" && (e = r), typeof e == "string") {
      if (hu(xu, e) === null)
        throw new at(t, e, Ru);
      e = cu(e, 8);
    }
    return Fn(e, t), e;
  }
  var Iu = W((e, t, r = au, n = su) => {
    if (typeof e != "number")
      throw new D(t, "number", e);
    if (!tr(e))
      throw new ge(t, "an integer", e);
    if (e < r || e > n)
      throw new ge(t, `>= ${r} && <= ${n}`, e);
  }), Tu = W((e, t, r = -2147483648, n = 2147483647) => {
    if (typeof e != "number")
      throw new D(t, "number", e);
    if (!tr(e))
      throw new ge(t, "an integer", e);
    if (e < r || e > n)
      throw new ge(t, `>= ${r} && <= ${n}`, e);
  }), Fn = W((e, t, r = false) => {
    if (typeof e != "number")
      throw new D(t, "number", e);
    if (!tr(e))
      throw new ge(t, "an integer", e);
    let n = r ? 1 : 0, i = 4294967295;
    if (e < n || e > i)
      throw new ge(t, `>= ${n} && <= ${i}`, e);
  });
  function Mn(e, t) {
    if (typeof e != "string")
      throw new D(t, "string", e);
  }
  function Bu(e, t, r = undefined, n) {
    if (typeof e != "number")
      throw new D(t, "number", e);
    if (r != null && e < r || n != null && e > n || (r != null || n != null) && fu(e))
      throw new ge(t, `${r != null ? `>= ${r}` : ""}${r != null && n != null ? " && " : ""}${n != null ? `<= ${n}` : ""}`, e);
  }
  var Lu = W((e, t, r) => {
    if (!Ln(r, e)) {
      let n = Nn(uu(r, (o) => typeof o == "string" ? `'${o}'` : pu(o)), ", "), i = "must be one of: " + n;
      throw new at(t, e, i);
    }
  });
  function Nu(e, t) {
    if (typeof e != "boolean")
      throw new D(t, "boolean", e);
  }
  function er(e, t, r) {
    return e == null || !du(e, t) ? r : e[t];
  }
  var Fu = W((e, t, r = null) => {
    let n = er(r, "allowArray", false), i = er(r, "allowFunction", false);
    if (!er(r, "nullable", false) && e === null || !n && Bn(e) || typeof e != "object" && (!i || typeof e != "function"))
      throw new D(t, "Object", e);
  }), Mu = W((e, t, r = 0) => {
    if (!Bn(e))
      throw new D(t, "Array", e);
    if (e.length < r) {
      let n = `must be longer than ${r}`;
      throw new at(t, e, n);
    }
  });
  function Cu(e, t = "signal") {
    if (Mn(e, t), Tn[e] === undefined)
      throw Tn[yu(e)] !== undefined ? new In(e + " (signals must use all capital letters)") : new In(e);
  }
  var Ou = W((e, t = "buffer") => {
    if (!Eu(e))
      throw new D(t, ["Buffer", "TypedArray", "DataView"], e);
  });
  function Du(e, t) {
    let r = gu(t), n = e.length;
    if (r === "hex" && n % 2 !== 0)
      throw new at("encoding", t, `is invalid for data of length ${n}`);
  }
  function Pu(e, t = "Port", r = true) {
    if (typeof e != "number" && typeof e != "string" || typeof e == "string" && wu(e).length === 0 || +e !== +e >>> 0 || e > 65535 || e === 0 && !r)
      throw new bu(t, e, r);
    return e | 0;
  }
  var ku = W((e, t) => {
    if (e !== undefined && (e === null || typeof e != "object" || !("aborted" in e)))
      throw new D(t, "AbortSignal", e);
  }), Uu = W((e, t) => {
    if (typeof e != "function")
      throw new D(t, "Function", e);
  }), vu = W((e, t) => {
    if (typeof e != "function" || _u(e))
      throw new D(t, "Function", e);
  }), qu = W((e, t) => {
    if (e !== undefined)
      throw new D(t, "undefined", e);
  });
  function Wu(e, t, r) {
    if (!Ln(r, e))
      throw new D(t, `('${Nn(r, "|")}')`, e);
  }
  Cn.exports = { isInt32: Su, isUint32: mu, parseFileMode: Au, validateArray: Mu, validateBoolean: Nu, validateBuffer: Ou, validateEncoding: Du, validateFunction: Uu, validateInt32: Tu, validateInteger: Iu, validateNumber: Bu, validateObject: Fu, validateOneOf: Lu, validatePlainFunction: vu, validatePort: Pu, validateSignalName: Cu, validateString: Mn, validateUint32: Fn, validateUndefined: qu, validateUnion: Wu, validateAbortSignal: ku };
});
var ir = E((zc, kn) => {
  var x = kn.exports = {}, Y, K;
  function rr() {
    throw new Error("setTimeout has not been defined");
  }
  function nr() {
    throw new Error("clearTimeout has not been defined");
  }
  (function() {
    try {
      typeof setTimeout == "function" ? Y = setTimeout : Y = rr;
    } catch {
      Y = rr;
    }
    try {
      typeof clearTimeout == "function" ? K = clearTimeout : K = nr;
    } catch {
      K = nr;
    }
  })();
  function On(e) {
    if (Y === setTimeout)
      return setTimeout(e, 0);
    if ((Y === rr || !Y) && setTimeout)
      return Y = setTimeout, setTimeout(e, 0);
    try {
      return Y(e, 0);
    } catch {
      try {
        return Y.call(null, e, 0);
      } catch {
        return Y.call(this, e, 0);
      }
    }
  }
  function $u(e) {
    if (K === clearTimeout)
      return clearTimeout(e);
    if ((K === nr || !K) && clearTimeout)
      return K = clearTimeout, clearTimeout(e);
    try {
      return K(e);
    } catch {
      try {
        return K.call(null, e);
      } catch {
        return K.call(this, e);
      }
    }
  }
  var re = [], Me = false, _e, ct = -1;
  function ju() {
    !Me || !_e || (Me = false, _e.length ? re = _e.concat(re) : ct = -1, re.length && Dn());
  }
  function Dn() {
    if (!Me) {
      var e = On(ju);
      Me = true;
      for (var t = re.length;t; ) {
        for (_e = re, re = [];++ct < t; )
          _e && _e[ct].run();
        ct = -1, t = re.length;
      }
      _e = null, Me = false, $u(e);
    }
  }
  x.nextTick = function(e) {
    var t = new Array(arguments.length - 1);
    if (arguments.length > 1)
      for (var r = 1;r < arguments.length; r++)
        t[r - 1] = arguments[r];
    re.push(new Pn(e, t)), re.length === 1 && !Me && On(Dn);
  };
  function Pn(e, t) {
    this.fun = e, this.array = t;
  }
  Pn.prototype.run = function() {
    this.fun.apply(null, this.array);
  };
  x.title = "browser";
  x.browser = true;
  x.env = {};
  x.argv = [];
  x.version = "";
  x.versions = {};
  function ne() {
  }
  x.on = ne;
  x.addListener = ne;
  x.once = ne;
  x.off = ne;
  x.removeListener = ne;
  x.removeAllListeners = ne;
  x.emit = ne;
  x.prependListener = ne;
  x.prependOnceListener = ne;
  x.listeners = function(e) {
    return [];
  };
  x.binding = function(e) {
    throw new Error("process.binding is not supported");
  };
  x.cwd = function() {
    return "/";
  };
  x.chdir = function(e) {
    throw new Error("process.chdir is not supported");
  };
  x.umask = function() {
    return 0;
  };
});
var k = {};
Qr(k, { default: () => Gu });
var Gu;
var se = yl(() => {
  ue(k, rt(ir()));
  Gu = rt(ir());
});
var ae = E((Jc, zn) => {
  var { Symbol: dt, SymbolAsyncIterator: Un, SymbolIterator: vn } = I(), qn = dt("kDestroyed"), Wn = dt("kIsErrored"), or = dt("kIsReadable"), $n = dt("kIsDisturbed");
  function ht(e, t = false) {
    var r;
    return !!(e && typeof e.pipe == "function" && typeof e.on == "function" && (!t || typeof e.pause == "function" && typeof e.resume == "function") && (!e._writableState || ((r = e._readableState) === null || r === undefined ? undefined : r.readable) !== false) && (!e._writableState || e._readableState));
  }
  function pt(e) {
    var t;
    return !!(e && typeof e.write == "function" && typeof e.on == "function" && (!e._readableState || ((t = e._writableState) === null || t === undefined ? undefined : t.writable) !== false));
  }
  function Hu(e) {
    return !!(e && typeof e.pipe == "function" && e._readableState && typeof e.on == "function" && typeof e.write == "function");
  }
  function Ee(e) {
    return e && (e._readableState || e._writableState || typeof e.write == "function" && typeof e.on == "function" || typeof e.pipe == "function" && typeof e.on == "function");
  }
  function Vu(e, t) {
    return e == null ? false : t === true ? typeof e[Un] == "function" : t === false ? typeof e[vn] == "function" : typeof e[Un] == "function" || typeof e[vn] == "function";
  }
  function yt(e) {
    if (!Ee(e))
      return null;
    let { _writableState: t, _readableState: r } = e, n = t || r;
    return !!(e.destroyed || e[qn] || n != null && n.destroyed);
  }
  function jn(e) {
    if (!pt(e))
      return null;
    if (e.writableEnded === true)
      return true;
    let t = e._writableState;
    return t != null && t.errored ? false : typeof t?.ended != "boolean" ? null : t.ended;
  }
  function Yu(e, t) {
    if (!pt(e))
      return null;
    if (e.writableFinished === true)
      return true;
    let r = e._writableState;
    return r != null && r.errored ? false : typeof r?.finished != "boolean" ? null : !!(r.finished || t === false && r.ended === true && r.length === 0);
  }
  function Ku(e) {
    if (!ht(e))
      return null;
    if (e.readableEnded === true)
      return true;
    let t = e._readableState;
    return !t || t.errored ? false : typeof t?.ended != "boolean" ? null : t.ended;
  }
  function Gn(e, t) {
    if (!ht(e))
      return null;
    let r = e._readableState;
    return r != null && r.errored ? false : typeof r?.endEmitted != "boolean" ? null : !!(r.endEmitted || t === false && r.ended === true && r.length === 0);
  }
  function Hn(e) {
    return e && e[or] != null ? e[or] : typeof e?.readable != "boolean" ? null : yt(e) ? false : ht(e) && e.readable && !Gn(e);
  }
  function Vn(e) {
    return typeof e?.writable != "boolean" ? null : yt(e) ? false : pt(e) && e.writable && !jn(e);
  }
  function zu(e, t) {
    return Ee(e) ? yt(e) ? true : !(t?.readable !== false && Hn(e) || t?.writable !== false && Vn(e)) : null;
  }
  function Xu(e) {
    var t, r;
    return Ee(e) ? e.writableErrored ? e.writableErrored : (t = (r = e._writableState) === null || r === undefined ? undefined : r.errored) !== null && t !== undefined ? t : null : null;
  }
  function Ju(e) {
    var t, r;
    return Ee(e) ? e.readableErrored ? e.readableErrored : (t = (r = e._readableState) === null || r === undefined ? undefined : r.errored) !== null && t !== undefined ? t : null : null;
  }
  function Qu(e) {
    if (!Ee(e))
      return null;
    if (typeof e.closed == "boolean")
      return e.closed;
    let { _writableState: t, _readableState: r } = e;
    return typeof t?.closed == "boolean" || typeof r?.closed == "boolean" ? t?.closed || r?.closed : typeof e._closed == "boolean" && Yn(e) ? e._closed : null;
  }
  function Yn(e) {
    return typeof e._closed == "boolean" && typeof e._defaultKeepAlive == "boolean" && typeof e._removedConnection == "boolean" && typeof e._removedContLen == "boolean";
  }
  function Kn(e) {
    return typeof e._sent100 == "boolean" && Yn(e);
  }
  function Zu(e) {
    var t;
    return typeof e._consuming == "boolean" && typeof e._dumped == "boolean" && ((t = e.req) === null || t === undefined ? undefined : t.upgradeOrConnect) === undefined;
  }
  function ef(e) {
    if (!Ee(e))
      return null;
    let { _writableState: t, _readableState: r } = e, n = t || r;
    return !n && Kn(e) || !!(n && n.autoDestroy && n.emitClose && n.closed === false);
  }
  function tf(e) {
    var t;
    return !!(e && ((t = e[$n]) !== null && t !== undefined ? t : e.readableDidRead || e.readableAborted));
  }
  function rf(e) {
    var t, r, n, i, o, l, u, f, s, d;
    return !!(e && ((t = (r = (n = (i = (o = (l = e[Wn]) !== null && l !== undefined ? l : e.readableErrored) !== null && o !== undefined ? o : e.writableErrored) !== null && i !== undefined ? i : (u = e._readableState) === null || u === undefined ? undefined : u.errorEmitted) !== null && n !== undefined ? n : (f = e._writableState) === null || f === undefined ? undefined : f.errorEmitted) !== null && r !== undefined ? r : (s = e._readableState) === null || s === undefined ? undefined : s.errored) !== null && t !== undefined ? t : (d = e._writableState) === null || d === undefined ? undefined : d.errored));
  }
  zn.exports = { kDestroyed: qn, isDisturbed: tf, kIsDisturbed: $n, isErrored: rf, kIsErrored: Wn, isReadable: Hn, kIsReadable: or, isClosed: Qu, isDestroyed: yt, isDuplexNodeStream: Hu, isFinished: zu, isIterable: Vu, isReadableNodeStream: ht, isReadableEnded: Ku, isReadableFinished: Gn, isReadableErrored: Ju, isNodeStream: Ee, isWritable: Vn, isWritableNodeStream: pt, isWritableEnded: jn, isWritableFinished: Yu, isWritableErrored: Xu, isServerRequest: Zu, isServerResponse: Kn, willEmitClose: ef };
});
var ce = E((Qc, ur) => {
  var Ce = (se(), pe(k)), { AbortError: nf, codes: of } = C(), { ERR_INVALID_ARG_TYPE: lf, ERR_STREAM_PREMATURE_CLOSE: Xn } = of, { kEmptyObject: Jn, once: Qn } = V(), { validateAbortSignal: uf, validateFunction: ff, validateObject: sf } = He(), { Promise: af } = I(), { isClosed: cf, isReadable: Zn, isReadableNodeStream: lr, isReadableFinished: ei, isReadableErrored: df, isWritable: ti, isWritableNodeStream: ri, isWritableFinished: ni, isWritableErrored: hf, isNodeStream: pf, willEmitClose: yf } = ae();
  function wf(e) {
    return e.setHeader && typeof e.abort == "function";
  }
  var bf = () => {
  };
  function ii(e, t, r) {
    var n, i;
    arguments.length === 2 ? (r = t, t = Jn) : t == null ? t = Jn : sf(t, "options"), ff(r, "callback"), uf(t.signal, "options.signal"), r = Qn(r);
    let o = (n = t.readable) !== null && n !== undefined ? n : lr(e), l = (i = t.writable) !== null && i !== undefined ? i : ri(e);
    if (!pf(e))
      throw new lf("stream", "Stream", e);
    let { _writableState: u, _readableState: f } = e, s = () => {
      e.writable || y();
    }, d = yf(e) && lr(e) === o && ri(e) === l, c = ni(e, false), y = () => {
      c = true, e.destroyed && (d = false), !(d && (!e.readable || o)) && (!o || h) && r.call(e);
    }, h = ei(e, false), p = () => {
      h = true, e.destroyed && (d = false), !(d && (!e.writable || l)) && (!l || c) && r.call(e);
    }, B = (N) => {
      r.call(e, N);
    }, v = cf(e), w = () => {
      v = true;
      let N = hf(e) || df(e);
      if (N && typeof N != "boolean")
        return r.call(e, N);
      if (o && !h && lr(e, true) && !ei(e, false))
        return r.call(e, new Xn);
      if (l && !c && !ni(e, false))
        return r.call(e, new Xn);
      r.call(e);
    }, b = () => {
      e.req.on("finish", y);
    };
    wf(e) ? (e.on("complete", y), d || e.on("abort", w), e.req ? b() : e.on("request", b)) : l && !u && (e.on("end", s), e.on("close", s)), !d && typeof e.aborted == "boolean" && e.on("aborted", w), e.on("end", p), e.on("finish", y), t.error !== false && e.on("error", B), e.on("close", w), v ? Ce.nextTick(w) : u != null && u.errorEmitted || f != null && f.errorEmitted ? d || Ce.nextTick(w) : (!o && (!d || Zn(e)) && (c || ti(e) === false) || !l && (!d || ti(e)) && (h || Zn(e) === false) || f && e.req && e.aborted) && Ce.nextTick(w);
    let L = () => {
      r = bf, e.removeListener("aborted", w), e.removeListener("complete", y), e.removeListener("abort", w), e.removeListener("request", b), e.req && e.req.removeListener("finish", y), e.removeListener("end", s), e.removeListener("close", s), e.removeListener("finish", y), e.removeListener("end", p), e.removeListener("error", B), e.removeListener("close", w);
    };
    if (t.signal && !v) {
      let N = () => {
        let Q = r;
        L(), Q.call(e, new nf(undefined, { cause: t.signal.reason }));
      };
      if (t.signal.aborted)
        Ce.nextTick(N);
      else {
        let Q = r;
        r = Qn((...Ie) => {
          t.signal.removeEventListener("abort", N), Q.apply(e, Ie);
        }), t.signal.addEventListener("abort", N);
      }
    }
    return L;
  }
  function gf(e, t) {
    return new af((r, n) => {
      ii(e, t, (i) => {
        i ? n(i) : r();
      });
    });
  }
  ur.exports = ii;
  ur.exports.finished = gf;
});
var di = E((Zc, ar) => {
  var fi = globalThis.AbortController || ut().AbortController, { codes: { ERR_INVALID_ARG_TYPE: Ve, ERR_MISSING_ARGS: _f, ERR_OUT_OF_RANGE: Ef }, AbortError: z } = C(), { validateAbortSignal: Oe, validateInteger: Sf, validateObject: De } = He(), mf = I().Symbol("kWeak"), { finished: xf } = ce(), { ArrayPrototypePush: Rf, MathFloor: Af, Number: If, NumberIsNaN: Tf, Promise: oi, PromiseReject: li, PromisePrototypeThen: Bf, Symbol: si } = I(), wt = si("kEmpty"), ui = si("kEof");
  function bt(e, t) {
    if (typeof e != "function")
      throw new Ve("fn", ["Function", "AsyncFunction"], e);
    t != null && De(t, "options"), t?.signal != null && Oe(t.signal, "options.signal");
    let r = 1;
    return t?.concurrency != null && (r = Af(t.concurrency)), Sf(r, "concurrency", 1), async function* () {
      var i, o;
      let l = new fi, u = this, f = [], s = l.signal, d = { signal: s }, c = () => l.abort();
      t != null && (i = t.signal) !== null && i !== undefined && i.aborted && c(), t == null || (o = t.signal) === null || o === undefined || o.addEventListener("abort", c);
      let y, h, p = false;
      function B() {
        p = true;
      }
      async function v() {
        try {
          for await (let L of u) {
            var w;
            if (p)
              return;
            if (s.aborted)
              throw new z;
            try {
              L = e(L, d);
            } catch (N) {
              L = li(N);
            }
            L !== wt && (typeof ((w = L) === null || w === undefined ? undefined : w.catch) == "function" && L.catch(B), f.push(L), y && (y(), y = null), !p && f.length && f.length >= r && await new oi((N) => {
              h = N;
            }));
          }
          f.push(ui);
        } catch (L) {
          let N = li(L);
          Bf(N, undefined, B), f.push(N);
        } finally {
          var b;
          p = true, y && (y(), y = null), t == null || (b = t.signal) === null || b === undefined || b.removeEventListener("abort", c);
        }
      }
      v();
      try {
        for (;; ) {
          for (;f.length > 0; ) {
            let w = await f[0];
            if (w === ui)
              return;
            if (s.aborted)
              throw new z;
            w !== wt && (yield w), f.shift(), h && (h(), h = null);
          }
          await new oi((w) => {
            y = w;
          });
        }
      } finally {
        l.abort(), p = true, h && (h(), h = null);
      }
    }.call(this);
  }
  function Lf(e = undefined) {
    return e != null && De(e, "options"), e?.signal != null && Oe(e.signal, "options.signal"), async function* () {
      let r = 0;
      for await (let i of this) {
        var n;
        if (e != null && (n = e.signal) !== null && n !== undefined && n.aborted)
          throw new z({ cause: e.signal.reason });
        yield [r++, i];
      }
    }.call(this);
  }
  async function ai(e, t = undefined) {
    for await (let r of sr.call(this, e, t))
      return true;
    return false;
  }
  async function Nf(e, t = undefined) {
    if (typeof e != "function")
      throw new Ve("fn", ["Function", "AsyncFunction"], e);
    return !await ai.call(this, async (...r) => !await e(...r), t);
  }
  async function Ff(e, t) {
    for await (let r of sr.call(this, e, t))
      return r;
  }
  async function Mf(e, t) {
    if (typeof e != "function")
      throw new Ve("fn", ["Function", "AsyncFunction"], e);
    async function r(n, i) {
      return await e(n, i), wt;
    }
    for await (let n of bt.call(this, r, t))
      ;
  }
  function sr(e, t) {
    if (typeof e != "function")
      throw new Ve("fn", ["Function", "AsyncFunction"], e);
    async function r(n, i) {
      return await e(n, i) ? n : wt;
    }
    return bt.call(this, r, t);
  }
  var fr = class extends _f {
    constructor() {
      super("reduce"), this.message = "Reduce of an empty stream requires an initial value";
    }
  };
  async function Cf(e, t, r) {
    var n;
    if (typeof e != "function")
      throw new Ve("reducer", ["Function", "AsyncFunction"], e);
    r != null && De(r, "options"), r?.signal != null && Oe(r.signal, "options.signal");
    let i = arguments.length > 1;
    if (r != null && (n = r.signal) !== null && n !== undefined && n.aborted) {
      let s = new z(undefined, { cause: r.signal.reason });
      throw this.once("error", () => {
      }), await xf(this.destroy(s)), s;
    }
    let o = new fi, l = o.signal;
    if (r != null && r.signal) {
      let s = { once: true, [mf]: this };
      r.signal.addEventListener("abort", () => o.abort(), s);
    }
    let u = false;
    try {
      for await (let s of this) {
        var f;
        if (u = true, r != null && (f = r.signal) !== null && f !== undefined && f.aborted)
          throw new z;
        i ? t = await e(t, s, { signal: l }) : (t = s, i = true);
      }
      if (!u && !i)
        throw new fr;
    } finally {
      o.abort();
    }
    return t;
  }
  async function Of(e) {
    e != null && De(e, "options"), e?.signal != null && Oe(e.signal, "options.signal");
    let t = [];
    for await (let n of this) {
      var r;
      if (e != null && (r = e.signal) !== null && r !== undefined && r.aborted)
        throw new z(undefined, { cause: e.signal.reason });
      Rf(t, n);
    }
    return t;
  }
  function Df(e, t) {
    let r = bt.call(this, e, t);
    return async function* () {
      for await (let i of r)
        yield* i;
    }.call(this);
  }
  function ci(e) {
    if (e = If(e), Tf(e))
      return 0;
    if (e < 0)
      throw new Ef("number", ">= 0", e);
    return e;
  }
  function Pf(e, t = undefined) {
    return t != null && De(t, "options"), t?.signal != null && Oe(t.signal, "options.signal"), e = ci(e), async function* () {
      var n;
      if (t != null && (n = t.signal) !== null && n !== undefined && n.aborted)
        throw new z;
      for await (let o of this) {
        var i;
        if (t != null && (i = t.signal) !== null && i !== undefined && i.aborted)
          throw new z;
        e-- <= 0 && (yield o);
      }
    }.call(this);
  }
  function kf(e, t = undefined) {
    return t != null && De(t, "options"), t?.signal != null && Oe(t.signal, "options.signal"), e = ci(e), async function* () {
      var n;
      if (t != null && (n = t.signal) !== null && n !== undefined && n.aborted)
        throw new z;
      for await (let o of this) {
        var i;
        if (t != null && (i = t.signal) !== null && i !== undefined && i.aborted)
          throw new z;
        if (e-- > 0)
          yield o;
        else
          return;
      }
    }.call(this);
  }
  ar.exports.streamReturningOperators = { asIndexedPairs: Lf, drop: Pf, filter: sr, flatMap: Df, map: bt, take: kf };
  ar.exports.promiseReturningOperators = { every: Nf, forEach: Mf, reduce: Cf, toArray: Of, some: ai, find: Ff };
});
var Se = E((ed, Ei) => {
  var de = (se(), pe(k)), { aggregateTwoErrors: Uf, codes: { ERR_MULTIPLE_CALLBACK: vf }, AbortError: qf } = C(), { Symbol: yi } = I(), { kDestroyed: Wf, isDestroyed: $f, isFinished: jf, isServerRequest: Gf } = ae(), wi = yi("kDestroy"), cr = yi("kConstruct");
  function bi(e, t, r) {
    e && (e.stack, t && !t.errored && (t.errored = e), r && !r.errored && (r.errored = e));
  }
  function Hf(e, t) {
    let r = this._readableState, n = this._writableState, i = n || r;
    return n && n.destroyed || r && r.destroyed ? (typeof t == "function" && t(), this) : (bi(e, n, r), n && (n.destroyed = true), r && (r.destroyed = true), i.constructed ? hi(this, e, t) : this.once(wi, function(o) {
      hi(this, Uf(o, e), t);
    }), this);
  }
  function hi(e, t, r) {
    let n = false;
    function i(o) {
      if (n)
        return;
      n = true;
      let { _readableState: l, _writableState: u } = e;
      bi(o, u, l), u && (u.closed = true), l && (l.closed = true), typeof r == "function" && r(o), o ? de.nextTick(Vf, e, o) : de.nextTick(gi, e);
    }
    try {
      e._destroy(t || null, i);
    } catch (o) {
      i(o);
    }
  }
  function Vf(e, t) {
    dr(e, t), gi(e);
  }
  function gi(e) {
    let { _readableState: t, _writableState: r } = e;
    r && (r.closeEmitted = true), t && (t.closeEmitted = true), (r && r.emitClose || t && t.emitClose) && e.emit("close");
  }
  function dr(e, t) {
    let { _readableState: r, _writableState: n } = e;
    n && n.errorEmitted || r && r.errorEmitted || (n && (n.errorEmitted = true), r && (r.errorEmitted = true), e.emit("error", t));
  }
  function Yf() {
    let e = this._readableState, t = this._writableState;
    e && (e.constructed = true, e.closed = false, e.closeEmitted = false, e.destroyed = false, e.errored = null, e.errorEmitted = false, e.reading = false, e.ended = e.readable === false, e.endEmitted = e.readable === false), t && (t.constructed = true, t.destroyed = false, t.closed = false, t.closeEmitted = false, t.errored = null, t.errorEmitted = false, t.finalCalled = false, t.prefinished = false, t.ended = t.writable === false, t.ending = t.writable === false, t.finished = t.writable === false);
  }
  function hr(e, t, r) {
    let { _readableState: n, _writableState: i } = e;
    if (i && i.destroyed || n && n.destroyed)
      return this;
    n && n.autoDestroy || i && i.autoDestroy ? e.destroy(t) : t && (t.stack, i && !i.errored && (i.errored = t), n && !n.errored && (n.errored = t), r ? de.nextTick(dr, e, t) : dr(e, t));
  }
  function Kf(e, t) {
    if (typeof e._construct != "function")
      return;
    let { _readableState: r, _writableState: n } = e;
    r && (r.constructed = false), n && (n.constructed = false), e.once(cr, t), !(e.listenerCount(cr) > 1) && de.nextTick(zf, e);
  }
  function zf(e) {
    let t = false;
    function r(n) {
      if (t) {
        hr(e, n ?? new vf);
        return;
      }
      t = true;
      let { _readableState: i, _writableState: o } = e, l = o || i;
      i && (i.constructed = true), o && (o.constructed = true), l.destroyed ? e.emit(wi, n) : n ? hr(e, n, true) : de.nextTick(Xf, e);
    }
    try {
      e._construct(r);
    } catch (n) {
      r(n);
    }
  }
  function Xf(e) {
    e.emit(cr);
  }
  function pi(e) {
    return e && e.setHeader && typeof e.abort == "function";
  }
  function _i(e) {
    e.emit("close");
  }
  function Jf(e, t) {
    e.emit("error", t), de.nextTick(_i, e);
  }
  function Qf(e, t) {
    !e || $f(e) || (!t && !jf(e) && (t = new qf), Gf(e) ? (e.socket = null, e.destroy(t)) : pi(e) ? e.abort() : pi(e.req) ? e.req.abort() : typeof e.destroy == "function" ? e.destroy(t) : typeof e.close == "function" ? e.close() : t ? de.nextTick(Jf, e, t) : de.nextTick(_i, e), e.destroyed || (e[Wf] = true));
  }
  Ei.exports = { construct: Kf, destroyer: Qf, destroy: Hf, undestroy: Yf, errorOrDestroy: hr };
});
var Et = E((td, pr) => {
  var Pe = typeof Reflect == "object" ? Reflect : null, Si = Pe && typeof Pe.apply == "function" ? Pe.apply : function(t, r, n) {
    return Function.prototype.apply.call(t, r, n);
  }, gt;
  Pe && typeof Pe.ownKeys == "function" ? gt = Pe.ownKeys : Object.getOwnPropertySymbols ? gt = function(t) {
    return Object.getOwnPropertyNames(t).concat(Object.getOwnPropertySymbols(t));
  } : gt = function(t) {
    return Object.getOwnPropertyNames(t);
  };
  function Zf(e) {
    console && console.warn && console.warn(e);
  }
  var xi = Number.isNaN || function(t) {
    return t !== t;
  };
  function S() {
    S.init.call(this);
  }
  pr.exports = S;
  pr.exports.once = ns;
  S.EventEmitter = S;
  S.prototype._events = undefined;
  S.prototype._eventsCount = 0;
  S.prototype._maxListeners = undefined;
  var mi = 10;
  function _t(e) {
    if (typeof e != "function")
      throw new TypeError('The "listener" argument must be of type Function. Received type ' + typeof e);
  }
  Object.defineProperty(S, "defaultMaxListeners", { enumerable: true, get: function() {
    return mi;
  }, set: function(e) {
    if (typeof e != "number" || e < 0 || xi(e))
      throw new RangeError('The value of "defaultMaxListeners" is out of range. It must be a non-negative number. Received ' + e + ".");
    mi = e;
  } });
  S.init = function() {
    (this._events === undefined || this._events === Object.getPrototypeOf(this)._events) && (this._events = Object.create(null), this._eventsCount = 0), this._maxListeners = this._maxListeners || undefined;
  };
  S.prototype.setMaxListeners = function(t) {
    if (typeof t != "number" || t < 0 || xi(t))
      throw new RangeError('The value of "n" is out of range. It must be a non-negative number. Received ' + t + ".");
    return this._maxListeners = t, this;
  };
  function Ri(e) {
    return e._maxListeners === undefined ? S.defaultMaxListeners : e._maxListeners;
  }
  S.prototype.getMaxListeners = function() {
    return Ri(this);
  };
  S.prototype.emit = function(t) {
    for (var r = [], n = 1;n < arguments.length; n++)
      r.push(arguments[n]);
    var i = t === "error", o = this._events;
    if (o !== undefined)
      i = i && o.error === undefined;
    else if (!i)
      return false;
    if (i) {
      var l;
      if (r.length > 0 && (l = r[0]), l instanceof Error)
        throw l;
      var u = new Error("Unhandled error." + (l ? " (" + l.message + ")" : ""));
      throw u.context = l, u;
    }
    var f = o[t];
    if (f === undefined)
      return false;
    if (typeof f == "function")
      Si(f, this, r);
    else
      for (var s = f.length, d = Li(f, s), n = 0;n < s; ++n)
        Si(d[n], this, r);
    return true;
  };
  function Ai(e, t, r, n) {
    var i, o, l;
    if (_t(r), o = e._events, o === undefined ? (o = e._events = Object.create(null), e._eventsCount = 0) : (o.newListener !== undefined && (e.emit("newListener", t, r.listener ? r.listener : r), o = e._events), l = o[t]), l === undefined)
      l = o[t] = r, ++e._eventsCount;
    else if (typeof l == "function" ? l = o[t] = n ? [r, l] : [l, r] : n ? l.unshift(r) : l.push(r), i = Ri(e), i > 0 && l.length > i && !l.warned) {
      l.warned = true;
      var u = new Error("Possible EventEmitter memory leak detected. " + l.length + " " + String(t) + " listeners added. Use emitter.setMaxListeners() to increase limit");
      u.name = "MaxListenersExceededWarning", u.emitter = e, u.type = t, u.count = l.length, Zf(u);
    }
    return e;
  }
  S.prototype.addListener = function(t, r) {
    return Ai(this, t, r, false);
  };
  S.prototype.on = S.prototype.addListener;
  S.prototype.prependListener = function(t, r) {
    return Ai(this, t, r, true);
  };
  function es() {
    if (!this.fired)
      return this.target.removeListener(this.type, this.wrapFn), this.fired = true, arguments.length === 0 ? this.listener.call(this.target) : this.listener.apply(this.target, arguments);
  }
  function Ii(e, t, r) {
    var n = { fired: false, wrapFn: undefined, target: e, type: t, listener: r }, i = es.bind(n);
    return i.listener = r, n.wrapFn = i, i;
  }
  S.prototype.once = function(t, r) {
    return _t(r), this.on(t, Ii(this, t, r)), this;
  };
  S.prototype.prependOnceListener = function(t, r) {
    return _t(r), this.prependListener(t, Ii(this, t, r)), this;
  };
  S.prototype.removeListener = function(t, r) {
    var n, i, o, l, u;
    if (_t(r), i = this._events, i === undefined)
      return this;
    if (n = i[t], n === undefined)
      return this;
    if (n === r || n.listener === r)
      --this._eventsCount === 0 ? this._events = Object.create(null) : (delete i[t], i.removeListener && this.emit("removeListener", t, n.listener || r));
    else if (typeof n != "function") {
      for (o = -1, l = n.length - 1;l >= 0; l--)
        if (n[l] === r || n[l].listener === r) {
          u = n[l].listener, o = l;
          break;
        }
      if (o < 0)
        return this;
      o === 0 ? n.shift() : ts(n, o), n.length === 1 && (i[t] = n[0]), i.removeListener !== undefined && this.emit("removeListener", t, u || r);
    }
    return this;
  };
  S.prototype.off = S.prototype.removeListener;
  S.prototype.removeAllListeners = function(t) {
    var r, n, i;
    if (n = this._events, n === undefined)
      return this;
    if (n.removeListener === undefined)
      return arguments.length === 0 ? (this._events = Object.create(null), this._eventsCount = 0) : n[t] !== undefined && (--this._eventsCount === 0 ? this._events = Object.create(null) : delete n[t]), this;
    if (arguments.length === 0) {
      var o = Object.keys(n), l;
      for (i = 0;i < o.length; ++i)
        l = o[i], l !== "removeListener" && this.removeAllListeners(l);
      return this.removeAllListeners("removeListener"), this._events = Object.create(null), this._eventsCount = 0, this;
    }
    if (r = n[t], typeof r == "function")
      this.removeListener(t, r);
    else if (r !== undefined)
      for (i = r.length - 1;i >= 0; i--)
        this.removeListener(t, r[i]);
    return this;
  };
  function Ti(e, t, r) {
    var n = e._events;
    if (n === undefined)
      return [];
    var i = n[t];
    return i === undefined ? [] : typeof i == "function" ? r ? [i.listener || i] : [i] : r ? rs(i) : Li(i, i.length);
  }
  S.prototype.listeners = function(t) {
    return Ti(this, t, true);
  };
  S.prototype.rawListeners = function(t) {
    return Ti(this, t, false);
  };
  S.listenerCount = function(e, t) {
    return typeof e.listenerCount == "function" ? e.listenerCount(t) : Bi.call(e, t);
  };
  S.prototype.listenerCount = Bi;
  function Bi(e) {
    var t = this._events;
    if (t !== undefined) {
      var r = t[e];
      if (typeof r == "function")
        return 1;
      if (r !== undefined)
        return r.length;
    }
    return 0;
  }
  S.prototype.eventNames = function() {
    return this._eventsCount > 0 ? gt(this._events) : [];
  };
  function Li(e, t) {
    for (var r = new Array(t), n = 0;n < t; ++n)
      r[n] = e[n];
    return r;
  }
  function ts(e, t) {
    for (;t + 1 < e.length; t++)
      e[t] = e[t + 1];
    e.pop();
  }
  function rs(e) {
    for (var t = new Array(e.length), r = 0;r < t.length; ++r)
      t[r] = e[r].listener || e[r];
    return t;
  }
  function ns(e, t) {
    return new Promise(function(r, n) {
      function i(l) {
        e.removeListener(t, o), n(l);
      }
      function o() {
        typeof e.removeListener == "function" && e.removeListener("error", i), r([].slice.call(arguments));
      }
      Ni(e, t, o, { once: true }), t !== "error" && is(e, i, { once: true });
    });
  }
  function is(e, t, r) {
    typeof e.on == "function" && Ni(e, "error", t, r);
  }
  function Ni(e, t, r, n) {
    if (typeof e.on == "function")
      n.once ? e.once(t, r) : e.on(t, r);
    else if (typeof e.addEventListener == "function")
      e.addEventListener(t, function i(o) {
        n.once && e.removeEventListener(t, i), r(o);
      });
    else
      throw new TypeError('The "emitter" argument must be of type EventEmitter. Received type ' + typeof e);
  }
});
var xt = E((rd, Mi) => {
  var { ArrayIsArray: os, ObjectSetPrototypeOf: Fi } = I(), { EventEmitter: St } = Et();
  function mt(e) {
    St.call(this, e);
  }
  Fi(mt.prototype, St.prototype);
  Fi(mt, St);
  mt.prototype.pipe = function(e, t) {
    let r = this;
    function n(d) {
      e.writable && e.write(d) === false && r.pause && r.pause();
    }
    r.on("data", n);
    function i() {
      r.readable && r.resume && r.resume();
    }
    e.on("drain", i), !e._isStdio && (!t || t.end !== false) && (r.on("end", l), r.on("close", u));
    let o = false;
    function l() {
      o || (o = true, e.end());
    }
    function u() {
      o || (o = true, typeof e.destroy == "function" && e.destroy());
    }
    function f(d) {
      s(), St.listenerCount(this, "error") === 0 && this.emit("error", d);
    }
    yr(r, "error", f), yr(e, "error", f);
    function s() {
      r.removeListener("data", n), e.removeListener("drain", i), r.removeListener("end", l), r.removeListener("close", u), r.removeListener("error", f), e.removeListener("error", f), r.removeListener("end", s), r.removeListener("close", s), e.removeListener("close", s);
    }
    return r.on("end", s), r.on("close", s), e.on("close", s), e.emit("pipe", r), e;
  };
  function yr(e, t, r) {
    if (typeof e.prependListener == "function")
      return e.prependListener(t, r);
    !e._events || !e._events[t] ? e.on(t, r) : os(e._events[t]) ? e._events[t].unshift(r) : e._events[t] = [r, e._events[t]];
  }
  Mi.exports = { Stream: mt, prependListener: yr };
});
var At = E((nd, Rt) => {
  var { AbortError: ls, codes: us } = C(), fs = ce(), { ERR_INVALID_ARG_TYPE: Ci } = us, ss = (e, t) => {
    if (typeof e != "object" || !("aborted" in e))
      throw new Ci(t, "AbortSignal", e);
  };
  function as(e) {
    return !!(e && typeof e.pipe == "function");
  }
  Rt.exports.addAbortSignal = function(t, r) {
    if (ss(t, "signal"), !as(r))
      throw new Ci("stream", "stream.Stream", r);
    return Rt.exports.addAbortSignalNoValidate(t, r);
  };
  Rt.exports.addAbortSignalNoValidate = function(e, t) {
    if (typeof e != "object" || !("aborted" in e))
      return t;
    let r = () => {
      t.destroy(new ls(undefined, { cause: e.reason }));
    };
    return e.aborted ? r() : (e.addEventListener("abort", r), fs(t, () => e.removeEventListener("abort", r))), t;
  };
});
var Pi = E((od, Di) => {
  var { StringPrototypeSlice: Oi, SymbolIterator: cs, TypedArrayPrototypeSet: It, Uint8Array: ds } = I(), { Buffer: wr } = te(), { inspect: hs } = V();
  Di.exports = class {
    constructor() {
      this.head = null, this.tail = null, this.length = 0;
    }
    push(t) {
      let r = { data: t, next: null };
      this.length > 0 ? this.tail.next = r : this.head = r, this.tail = r, ++this.length;
    }
    unshift(t) {
      let r = { data: t, next: this.head };
      this.length === 0 && (this.tail = r), this.head = r, ++this.length;
    }
    shift() {
      if (this.length === 0)
        return;
      let t = this.head.data;
      return this.length === 1 ? this.head = this.tail = null : this.head = this.head.next, --this.length, t;
    }
    clear() {
      this.head = this.tail = null, this.length = 0;
    }
    join(t) {
      if (this.length === 0)
        return "";
      let r = this.head, n = "" + r.data;
      for (;(r = r.next) !== null; )
        n += t + r.data;
      return n;
    }
    concat(t) {
      if (this.length === 0)
        return wr.alloc(0);
      let r = wr.allocUnsafe(t >>> 0), n = this.head, i = 0;
      for (;n; )
        It(r, n.data, i), i += n.data.length, n = n.next;
      return r;
    }
    consume(t, r) {
      let n = this.head.data;
      if (t < n.length) {
        let i = n.slice(0, t);
        return this.head.data = n.slice(t), i;
      }
      return t === n.length ? this.shift() : r ? this._getString(t) : this._getBuffer(t);
    }
    first() {
      return this.head.data;
    }
    *[cs]() {
      for (let t = this.head;t; t = t.next)
        yield t.data;
    }
    _getString(t) {
      let r = "", n = this.head, i = 0;
      do {
        let o = n.data;
        if (t > o.length)
          r += o, t -= o.length;
        else {
          t === o.length ? (r += o, ++i, n.next ? this.head = n.next : this.head = this.tail = null) : (r += Oi(o, 0, t), this.head = n, n.data = Oi(o, t));
          break;
        }
        ++i;
      } while ((n = n.next) !== null);
      return this.length -= i, r;
    }
    _getBuffer(t) {
      let r = wr.allocUnsafe(t), n = t, i = this.head, o = 0;
      do {
        let l = i.data;
        if (t > l.length)
          It(r, l, n - t), t -= l.length;
        else {
          t === l.length ? (It(r, l, n - t), ++o, i.next ? this.head = i.next : this.head = this.tail = null) : (It(r, new ds(l.buffer, l.byteOffset, t), n - t), this.head = i, i.data = l.slice(t));
          break;
        }
        ++o;
      } while ((i = i.next) !== null);
      return this.length -= o, r;
    }
    [Symbol.for("nodejs.util.inspect.custom")](t, r) {
      return hs(this, { ...r, depth: 0, customInspect: false });
    }
  };
});
var Tt = E((ld, Ui) => {
  var { MathFloor: ps, NumberIsInteger: ys } = I(), { ERR_INVALID_ARG_VALUE: ws } = C().codes;
  function bs(e, t, r) {
    return e.highWaterMark != null ? e.highWaterMark : t ? e[r] : null;
  }
  function ki(e) {
    return e ? 16 : 16 * 1024;
  }
  function gs(e, t, r, n) {
    let i = bs(t, n, r);
    if (i != null) {
      if (!ys(i) || i < 0) {
        let o = n ? `options.${r}` : "options.highWaterMark";
        throw new ws(o, i);
      }
      return ps(i);
    }
    return ki(e.objectMode);
  }
  Ui.exports = { getHighWaterMark: gs, getDefaultHighWaterMark: ki };
});
var Wi = E((br, qi) => {
  var Bt = te(), X = Bt.Buffer;
  function vi(e, t) {
    for (var r in e)
      t[r] = e[r];
  }
  X.from && X.alloc && X.allocUnsafe && X.allocUnsafeSlow ? qi.exports = Bt : (vi(Bt, br), br.Buffer = me);
  function me(e, t, r) {
    return X(e, t, r);
  }
  me.prototype = Object.create(X.prototype);
  vi(X, me);
  me.from = function(e, t, r) {
    if (typeof e == "number")
      throw new TypeError("Argument must not be a number");
    return X(e, t, r);
  };
  me.alloc = function(e, t, r) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    var n = X(e);
    return t !== undefined ? typeof r == "string" ? n.fill(t, r) : n.fill(t) : n.fill(0), n;
  };
  me.allocUnsafe = function(e) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    return X(e);
  };
  me.allocUnsafeSlow = function(e) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    return Bt.SlowBuffer(e);
  };
});
var Gi = E((ji) => {
  var _r = Wi().Buffer, $i = _r.isEncoding || function(e) {
    switch (e = "" + e, e && e.toLowerCase()) {
      case "hex":
      case "utf8":
      case "utf-8":
      case "ascii":
      case "binary":
      case "base64":
      case "ucs2":
      case "ucs-2":
      case "utf16le":
      case "utf-16le":
      case "raw":
        return true;
      default:
        return false;
    }
  };
  function _s(e) {
    if (!e)
      return "utf8";
    for (var t;; )
      switch (e) {
        case "utf8":
        case "utf-8":
          return "utf8";
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return "utf16le";
        case "latin1":
        case "binary":
          return "latin1";
        case "base64":
        case "ascii":
        case "hex":
          return e;
        default:
          if (t)
            return;
          e = ("" + e).toLowerCase(), t = true;
      }
  }
  function Es(e) {
    var t = _s(e);
    if (typeof t != "string" && (_r.isEncoding === $i || !$i(e)))
      throw new Error("Unknown encoding: " + e);
    return t || e;
  }
  ji.StringDecoder = Ye;
  function Ye(e) {
    this.encoding = Es(e);
    var t;
    switch (this.encoding) {
      case "utf16le":
        this.text = Is, this.end = Ts, t = 4;
        break;
      case "utf8":
        this.fillLast = xs, t = 4;
        break;
      case "base64":
        this.text = Bs, this.end = Ls, t = 3;
        break;
      default:
        this.write = Ns, this.end = Fs;
        return;
    }
    this.lastNeed = 0, this.lastTotal = 0, this.lastChar = _r.allocUnsafe(t);
  }
  Ye.prototype.write = function(e) {
    if (e.length === 0)
      return "";
    var t, r;
    if (this.lastNeed) {
      if (t = this.fillLast(e), t === undefined)
        return "";
      r = this.lastNeed, this.lastNeed = 0;
    } else
      r = 0;
    return r < e.length ? t ? t + this.text(e, r) : this.text(e, r) : t || "";
  };
  Ye.prototype.end = As;
  Ye.prototype.text = Rs;
  Ye.prototype.fillLast = function(e) {
    if (this.lastNeed <= e.length)
      return e.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
    e.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, e.length), this.lastNeed -= e.length;
  };
  function gr(e) {
    return e <= 127 ? 0 : e >> 5 === 6 ? 2 : e >> 4 === 14 ? 3 : e >> 3 === 30 ? 4 : e >> 6 === 2 ? -1 : -2;
  }
  function Ss(e, t, r) {
    var n = t.length - 1;
    if (n < r)
      return 0;
    var i = gr(t[n]);
    return i >= 0 ? (i > 0 && (e.lastNeed = i - 1), i) : --n < r || i === -2 ? 0 : (i = gr(t[n]), i >= 0 ? (i > 0 && (e.lastNeed = i - 2), i) : --n < r || i === -2 ? 0 : (i = gr(t[n]), i >= 0 ? (i > 0 && (i === 2 ? i = 0 : e.lastNeed = i - 3), i) : 0));
  }
  function ms(e, t, r) {
    if ((t[0] & 192) !== 128)
      return e.lastNeed = 0, "�";
    if (e.lastNeed > 1 && t.length > 1) {
      if ((t[1] & 192) !== 128)
        return e.lastNeed = 1, "�";
      if (e.lastNeed > 2 && t.length > 2 && (t[2] & 192) !== 128)
        return e.lastNeed = 2, "�";
    }
  }
  function xs(e) {
    var t = this.lastTotal - this.lastNeed, r = ms(this, e, t);
    if (r !== undefined)
      return r;
    if (this.lastNeed <= e.length)
      return e.copy(this.lastChar, t, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
    e.copy(this.lastChar, t, 0, e.length), this.lastNeed -= e.length;
  }
  function Rs(e, t) {
    var r = Ss(this, e, t);
    if (!this.lastNeed)
      return e.toString("utf8", t);
    this.lastTotal = r;
    var n = e.length - (r - this.lastNeed);
    return e.copy(this.lastChar, 0, n), e.toString("utf8", t, n);
  }
  function As(e) {
    var t = e && e.length ? this.write(e) : "";
    return this.lastNeed ? t + "�" : t;
  }
  function Is(e, t) {
    if ((e.length - t) % 2 === 0) {
      var r = e.toString("utf16le", t);
      if (r) {
        var n = r.charCodeAt(r.length - 1);
        if (n >= 55296 && n <= 56319)
          return this.lastNeed = 2, this.lastTotal = 4, this.lastChar[0] = e[e.length - 2], this.lastChar[1] = e[e.length - 1], r.slice(0, -1);
      }
      return r;
    }
    return this.lastNeed = 1, this.lastTotal = 2, this.lastChar[0] = e[e.length - 1], e.toString("utf16le", t, e.length - 1);
  }
  function Ts(e) {
    var t = e && e.length ? this.write(e) : "";
    if (this.lastNeed) {
      var r = this.lastTotal - this.lastNeed;
      return t + this.lastChar.toString("utf16le", 0, r);
    }
    return t;
  }
  function Bs(e, t) {
    var r = (e.length - t) % 3;
    return r === 0 ? e.toString("base64", t) : (this.lastNeed = 3 - r, this.lastTotal = 3, r === 1 ? this.lastChar[0] = e[e.length - 1] : (this.lastChar[0] = e[e.length - 2], this.lastChar[1] = e[e.length - 1]), e.toString("base64", t, e.length - r));
  }
  function Ls(e) {
    var t = e && e.length ? this.write(e) : "";
    return this.lastNeed ? t + this.lastChar.toString("base64", 0, 3 - this.lastNeed) : t;
  }
  function Ns(e) {
    return e.toString(this.encoding);
  }
  function Fs(e) {
    return e && e.length ? this.write(e) : "";
  }
});
var Er = E((fd, Ki) => {
  var Hi = (se(), pe(k)), { PromisePrototypeThen: Ms, SymbolAsyncIterator: Vi, SymbolIterator: Yi } = I(), { Buffer: Cs } = te(), { ERR_INVALID_ARG_TYPE: Os, ERR_STREAM_NULL_VALUES: Ds } = C().codes;
  function Ps(e, t, r) {
    let n;
    if (typeof t == "string" || t instanceof Cs)
      return new e({ objectMode: true, ...r, read() {
        this.push(t), this.push(null);
      } });
    let i;
    if (t && t[Vi])
      i = true, n = t[Vi]();
    else if (t && t[Yi])
      i = false, n = t[Yi]();
    else
      throw new Os("iterable", ["Iterable"], t);
    let o = new e({ objectMode: true, highWaterMark: 1, ...r }), l = false;
    o._read = function() {
      l || (l = true, f());
    }, o._destroy = function(s, d) {
      Ms(u(s), () => Hi.nextTick(d, s), (c) => Hi.nextTick(d, c || s));
    };
    async function u(s) {
      let d = s != null, c = typeof n.throw == "function";
      if (d && c) {
        let { value: y, done: h } = await n.throw(s);
        if (await y, h)
          return;
      }
      if (typeof n.return == "function") {
        let { value: y } = await n.return();
        await y;
      }
    }
    async function f() {
      for (;; ) {
        try {
          let { value: s, done: d } = i ? await n.next() : n.next();
          if (d)
            o.push(null);
          else {
            let c = s && typeof s.then == "function" ? await s : s;
            if (c === null)
              throw l = false, new Ds;
            if (o.push(c))
              continue;
            l = false;
          }
        } catch (s) {
          o.destroy(s);
        }
        break;
      }
    }
    return o;
  }
  Ki.exports = Ps;
});
var Ke = E((sd, uo) => {
  var $ = (se(), pe(k)), { ArrayPrototypeIndexOf: ks, NumberIsInteger: Us, NumberIsNaN: vs, NumberParseInt: qs, ObjectDefineProperties: Ji, ObjectKeys: Ws, ObjectSetPrototypeOf: Qi, Promise: $s, SafeSet: js, SymbolAsyncIterator: Gs, Symbol: Hs } = I();
  uo.exports = g;
  g.ReadableState = Ir;
  var { EventEmitter: Vs } = Et(), { Stream: he, prependListener: Ys } = xt(), { Buffer: Sr } = te(), { addAbortSignal: Ks } = At(), zs = ce(), _ = V().debuglog("stream", (e) => {
    _ = e;
  }), Xs = Pi(), Ue = Se(), { getHighWaterMark: Js, getDefaultHighWaterMark: Qs } = Tt(), { aggregateTwoErrors: zi, codes: { ERR_INVALID_ARG_TYPE: Zs, ERR_METHOD_NOT_IMPLEMENTED: ea, ERR_OUT_OF_RANGE: ta, ERR_STREAM_PUSH_AFTER_EOF: ra, ERR_STREAM_UNSHIFT_AFTER_END_EVENT: na } } = C(), { validateObject: ia } = He(), xe = Hs("kPaused"), { StringDecoder: Zi } = Gi(), oa = Er();
  Qi(g.prototype, he.prototype);
  Qi(g, he);
  var mr = () => {
  }, { errorOrDestroy: ke } = Ue;
  function Ir(e, t, r) {
    typeof r != "boolean" && (r = t instanceof J()), this.objectMode = !!(e && e.objectMode), r && (this.objectMode = this.objectMode || !!(e && e.readableObjectMode)), this.highWaterMark = e ? Js(this, e, "readableHighWaterMark", r) : Qs(false), this.buffer = new Xs, this.length = 0, this.pipes = [], this.flowing = null, this.ended = false, this.endEmitted = false, this.reading = false, this.constructed = true, this.sync = true, this.needReadable = false, this.emittedReadable = false, this.readableListening = false, this.resumeScheduled = false, this[xe] = null, this.errorEmitted = false, this.emitClose = !e || e.emitClose !== false, this.autoDestroy = !e || e.autoDestroy !== false, this.destroyed = false, this.errored = null, this.closed = false, this.closeEmitted = false, this.defaultEncoding = e && e.defaultEncoding || "utf8", this.awaitDrainWriters = null, this.multiAwaitDrain = false, this.readingMore = false, this.dataEmitted = false, this.decoder = null, this.encoding = null, e && e.encoding && (this.decoder = new Zi(e.encoding), this.encoding = e.encoding);
  }
  function g(e) {
    if (!(this instanceof g))
      return new g(e);
    let t = this instanceof J();
    this._readableState = new Ir(e, this, t), e && (typeof e.read == "function" && (this._read = e.read), typeof e.destroy == "function" && (this._destroy = e.destroy), typeof e.construct == "function" && (this._construct = e.construct), e.signal && !t && Ks(e.signal, this)), he.call(this, e), Ue.construct(this, () => {
      this._readableState.needReadable && Lt(this, this._readableState);
    });
  }
  g.prototype.destroy = Ue.destroy;
  g.prototype._undestroy = Ue.undestroy;
  g.prototype._destroy = function(e, t) {
    t(e);
  };
  g.prototype[Vs.captureRejectionSymbol] = function(e) {
    this.destroy(e);
  };
  g.prototype.push = function(e, t) {
    return eo(this, e, t, false);
  };
  g.prototype.unshift = function(e, t) {
    return eo(this, e, t, true);
  };
  function eo(e, t, r, n) {
    _("readableAddChunk", t);
    let i = e._readableState, o;
    if (i.objectMode || (typeof t == "string" ? (r = r || i.defaultEncoding, i.encoding !== r && (n && i.encoding ? t = Sr.from(t, r).toString(i.encoding) : (t = Sr.from(t, r), r = ""))) : t instanceof Sr ? r = "" : he._isUint8Array(t) ? (t = he._uint8ArrayToBuffer(t), r = "") : t != null && (o = new Zs("chunk", ["string", "Buffer", "Uint8Array"], t))), o)
      ke(e, o);
    else if (t === null)
      i.reading = false, fa(e, i);
    else if (i.objectMode || t && t.length > 0)
      if (n)
        if (i.endEmitted)
          ke(e, new na);
        else {
          if (i.destroyed || i.errored)
            return false;
          xr(e, i, t, true);
        }
      else if (i.ended)
        ke(e, new ra);
      else {
        if (i.destroyed || i.errored)
          return false;
        i.reading = false, i.decoder && !r ? (t = i.decoder.write(t), i.objectMode || t.length !== 0 ? xr(e, i, t, false) : Lt(e, i)) : xr(e, i, t, false);
      }
    else
      n || (i.reading = false, Lt(e, i));
    return !i.ended && (i.length < i.highWaterMark || i.length === 0);
  }
  function xr(e, t, r, n) {
    t.flowing && t.length === 0 && !t.sync && e.listenerCount("data") > 0 ? (t.multiAwaitDrain ? t.awaitDrainWriters.clear() : t.awaitDrainWriters = null, t.dataEmitted = true, e.emit("data", r)) : (t.length += t.objectMode ? 1 : r.length, n ? t.buffer.unshift(r) : t.buffer.push(r), t.needReadable && Nt(e)), Lt(e, t);
  }
  g.prototype.isPaused = function() {
    let e = this._readableState;
    return e[xe] === true || e.flowing === false;
  };
  g.prototype.setEncoding = function(e) {
    let t = new Zi(e);
    this._readableState.decoder = t, this._readableState.encoding = this._readableState.decoder.encoding;
    let r = this._readableState.buffer, n = "";
    for (let i of r)
      n += t.write(i);
    return r.clear(), n !== "" && r.push(n), this._readableState.length = n.length, this;
  };
  var la = 1073741824;
  function ua(e) {
    if (e > la)
      throw new ta("size", "<= 1GiB", e);
    return e--, e |= e >>> 1, e |= e >>> 2, e |= e >>> 4, e |= e >>> 8, e |= e >>> 16, e++, e;
  }
  function Xi(e, t) {
    return e <= 0 || t.length === 0 && t.ended ? 0 : t.objectMode ? 1 : vs(e) ? t.flowing && t.length ? t.buffer.first().length : t.length : e <= t.length ? e : t.ended ? t.length : 0;
  }
  g.prototype.read = function(e) {
    _("read", e), e === undefined ? e = NaN : Us(e) || (e = qs(e, 10));
    let t = this._readableState, r = e;
    if (e > t.highWaterMark && (t.highWaterMark = ua(e)), e !== 0 && (t.emittedReadable = false), e === 0 && t.needReadable && ((t.highWaterMark !== 0 ? t.length >= t.highWaterMark : t.length > 0) || t.ended))
      return _("read: emitReadable", t.length, t.ended), t.length === 0 && t.ended ? Rr(this) : Nt(this), null;
    if (e = Xi(e, t), e === 0 && t.ended)
      return t.length === 0 && Rr(this), null;
    let n = t.needReadable;
    if (_("need readable", n), (t.length === 0 || t.length - e < t.highWaterMark) && (n = true, _("length less than watermark", n)), t.ended || t.reading || t.destroyed || t.errored || !t.constructed)
      n = false, _("reading, ended or constructing", n);
    else if (n) {
      _("do read"), t.reading = true, t.sync = true, t.length === 0 && (t.needReadable = true);
      try {
        this._read(t.highWaterMark);
      } catch (o) {
        ke(this, o);
      }
      t.sync = false, t.reading || (e = Xi(r, t));
    }
    let i;
    return e > 0 ? i = oo(e, t) : i = null, i === null ? (t.needReadable = t.length <= t.highWaterMark, e = 0) : (t.length -= e, t.multiAwaitDrain ? t.awaitDrainWriters.clear() : t.awaitDrainWriters = null), t.length === 0 && (t.ended || (t.needReadable = true), r !== e && t.ended && Rr(this)), i !== null && !t.errorEmitted && !t.closeEmitted && (t.dataEmitted = true, this.emit("data", i)), i;
  };
  function fa(e, t) {
    if (_("onEofChunk"), !t.ended) {
      if (t.decoder) {
        let r = t.decoder.end();
        r && r.length && (t.buffer.push(r), t.length += t.objectMode ? 1 : r.length);
      }
      t.ended = true, t.sync ? Nt(e) : (t.needReadable = false, t.emittedReadable = true, to(e));
    }
  }
  function Nt(e) {
    let t = e._readableState;
    _("emitReadable", t.needReadable, t.emittedReadable), t.needReadable = false, t.emittedReadable || (_("emitReadable", t.flowing), t.emittedReadable = true, $.nextTick(to, e));
  }
  function to(e) {
    let t = e._readableState;
    _("emitReadable_", t.destroyed, t.length, t.ended), !t.destroyed && !t.errored && (t.length || t.ended) && (e.emit("readable"), t.emittedReadable = false), t.needReadable = !t.flowing && !t.ended && t.length <= t.highWaterMark, no(e);
  }
  function Lt(e, t) {
    !t.readingMore && t.constructed && (t.readingMore = true, $.nextTick(sa, e, t));
  }
  function sa(e, t) {
    for (;!t.reading && !t.ended && (t.length < t.highWaterMark || t.flowing && t.length === 0); ) {
      let r = t.length;
      if (_("maybeReadMore read 0"), e.read(0), r === t.length)
        break;
    }
    t.readingMore = false;
  }
  g.prototype._read = function(e) {
    throw new ea("_read()");
  };
  g.prototype.pipe = function(e, t) {
    let r = this, n = this._readableState;
    n.pipes.length === 1 && (n.multiAwaitDrain || (n.multiAwaitDrain = true, n.awaitDrainWriters = new js(n.awaitDrainWriters ? [n.awaitDrainWriters] : []))), n.pipes.push(e), _("pipe count=%d opts=%j", n.pipes.length, t);
    let o = (!t || t.end !== false) && e !== $.stdout && e !== $.stderr ? u : v;
    n.endEmitted ? $.nextTick(o) : r.once("end", o), e.on("unpipe", l);
    function l(w, b) {
      _("onunpipe"), w === r && b && b.hasUnpiped === false && (b.hasUnpiped = true, d());
    }
    function u() {
      _("onend"), e.end();
    }
    let f, s = false;
    function d() {
      _("cleanup"), e.removeListener("close", p), e.removeListener("finish", B), f && e.removeListener("drain", f), e.removeListener("error", h), e.removeListener("unpipe", l), r.removeListener("end", u), r.removeListener("end", v), r.removeListener("data", y), s = true, f && n.awaitDrainWriters && (!e._writableState || e._writableState.needDrain) && f();
    }
    function c() {
      s || (n.pipes.length === 1 && n.pipes[0] === e ? (_("false write response, pause", 0), n.awaitDrainWriters = e, n.multiAwaitDrain = false) : n.pipes.length > 1 && n.pipes.includes(e) && (_("false write response, pause", n.awaitDrainWriters.size), n.awaitDrainWriters.add(e)), r.pause()), f || (f = aa(r, e), e.on("drain", f));
    }
    r.on("data", y);
    function y(w) {
      _("ondata");
      let b = e.write(w);
      _("dest.write", b), b === false && c();
    }
    function h(w) {
      if (_("onerror", w), v(), e.removeListener("error", h), e.listenerCount("error") === 0) {
        let b = e._writableState || e._readableState;
        b && !b.errorEmitted ? ke(e, w) : e.emit("error", w);
      }
    }
    Ys(e, "error", h);
    function p() {
      e.removeListener("finish", B), v();
    }
    e.once("close", p);
    function B() {
      _("onfinish"), e.removeListener("close", p), v();
    }
    e.once("finish", B);
    function v() {
      _("unpipe"), r.unpipe(e);
    }
    return e.emit("pipe", r), e.writableNeedDrain === true ? n.flowing && c() : n.flowing || (_("pipe resume"), r.resume()), e;
  };
  function aa(e, t) {
    return function() {
      let n = e._readableState;
      n.awaitDrainWriters === t ? (_("pipeOnDrain", 1), n.awaitDrainWriters = null) : n.multiAwaitDrain && (_("pipeOnDrain", n.awaitDrainWriters.size), n.awaitDrainWriters.delete(t)), (!n.awaitDrainWriters || n.awaitDrainWriters.size === 0) && e.listenerCount("data") && e.resume();
    };
  }
  g.prototype.unpipe = function(e) {
    let t = this._readableState, r = { hasUnpiped: false };
    if (t.pipes.length === 0)
      return this;
    if (!e) {
      let i = t.pipes;
      t.pipes = [], this.pause();
      for (let o = 0;o < i.length; o++)
        i[o].emit("unpipe", this, { hasUnpiped: false });
      return this;
    }
    let n = ks(t.pipes, e);
    return n === -1 ? this : (t.pipes.splice(n, 1), t.pipes.length === 0 && this.pause(), e.emit("unpipe", this, r), this);
  };
  g.prototype.on = function(e, t) {
    let r = he.prototype.on.call(this, e, t), n = this._readableState;
    return e === "data" ? (n.readableListening = this.listenerCount("readable") > 0, n.flowing !== false && this.resume()) : e === "readable" && !n.endEmitted && !n.readableListening && (n.readableListening = n.needReadable = true, n.flowing = false, n.emittedReadable = false, _("on readable", n.length, n.reading), n.length ? Nt(this) : n.reading || $.nextTick(ca, this)), r;
  };
  g.prototype.addListener = g.prototype.on;
  g.prototype.removeListener = function(e, t) {
    let r = he.prototype.removeListener.call(this, e, t);
    return e === "readable" && $.nextTick(ro, this), r;
  };
  g.prototype.off = g.prototype.removeListener;
  g.prototype.removeAllListeners = function(e) {
    let t = he.prototype.removeAllListeners.apply(this, arguments);
    return (e === "readable" || e === undefined) && $.nextTick(ro, this), t;
  };
  function ro(e) {
    let t = e._readableState;
    t.readableListening = e.listenerCount("readable") > 0, t.resumeScheduled && t[xe] === false ? t.flowing = true : e.listenerCount("data") > 0 ? e.resume() : t.readableListening || (t.flowing = null);
  }
  function ca(e) {
    _("readable nexttick read 0"), e.read(0);
  }
  g.prototype.resume = function() {
    let e = this._readableState;
    return e.flowing || (_("resume"), e.flowing = !e.readableListening, da(this, e)), e[xe] = false, this;
  };
  function da(e, t) {
    t.resumeScheduled || (t.resumeScheduled = true, $.nextTick(ha, e, t));
  }
  function ha(e, t) {
    _("resume", t.reading), t.reading || e.read(0), t.resumeScheduled = false, e.emit("resume"), no(e), t.flowing && !t.reading && e.read(0);
  }
  g.prototype.pause = function() {
    return _("call pause flowing=%j", this._readableState.flowing), this._readableState.flowing !== false && (_("pause"), this._readableState.flowing = false, this.emit("pause")), this._readableState[xe] = true, this;
  };
  function no(e) {
    let t = e._readableState;
    for (_("flow", t.flowing);t.flowing && e.read() !== null; )
      ;
  }
  g.prototype.wrap = function(e) {
    let t = false;
    e.on("data", (n) => {
      !this.push(n) && e.pause && (t = true, e.pause());
    }), e.on("end", () => {
      this.push(null);
    }), e.on("error", (n) => {
      ke(this, n);
    }), e.on("close", () => {
      this.destroy();
    }), e.on("destroy", () => {
      this.destroy();
    }), this._read = () => {
      t && e.resume && (t = false, e.resume());
    };
    let r = Ws(e);
    for (let n = 1;n < r.length; n++) {
      let i = r[n];
      this[i] === undefined && typeof e[i] == "function" && (this[i] = e[i].bind(e));
    }
    return this;
  };
  g.prototype[Gs] = function() {
    return io(this);
  };
  g.prototype.iterator = function(e) {
    return e !== undefined && ia(e, "options"), io(this, e);
  };
  function io(e, t) {
    typeof e.read != "function" && (e = g.wrap(e, { objectMode: true }));
    let r = pa(e, t);
    return r.stream = e, r;
  }
  async function* pa(e, t) {
    let r = mr;
    function n(l) {
      this === e ? (r(), r = mr) : r = l;
    }
    e.on("readable", n);
    let i, o = zs(e, { writable: false }, (l) => {
      i = l ? zi(i, l) : null, r(), r = mr;
    });
    try {
      for (;; ) {
        let l = e.destroyed ? null : e.read();
        if (l !== null)
          yield l;
        else {
          if (i)
            throw i;
          if (i === null)
            return;
          await new $s(n);
        }
      }
    } catch (l) {
      throw i = zi(i, l), i;
    } finally {
      (i || t?.destroyOnReturn !== false) && (i === undefined || e._readableState.autoDestroy) ? Ue.destroyer(e, null) : (e.off("readable", n), o());
    }
  }
  Ji(g.prototype, { readable: { __proto__: null, get() {
    let e = this._readableState;
    return !!e && e.readable !== false && !e.destroyed && !e.errorEmitted && !e.endEmitted;
  }, set(e) {
    this._readableState && (this._readableState.readable = !!e);
  } }, readableDidRead: { __proto__: null, enumerable: false, get: function() {
    return this._readableState.dataEmitted;
  } }, readableAborted: { __proto__: null, enumerable: false, get: function() {
    return !!(this._readableState.readable !== false && (this._readableState.destroyed || this._readableState.errored) && !this._readableState.endEmitted);
  } }, readableHighWaterMark: { __proto__: null, enumerable: false, get: function() {
    return this._readableState.highWaterMark;
  } }, readableBuffer: { __proto__: null, enumerable: false, get: function() {
    return this._readableState && this._readableState.buffer;
  } }, readableFlowing: { __proto__: null, enumerable: false, get: function() {
    return this._readableState.flowing;
  }, set: function(e) {
    this._readableState && (this._readableState.flowing = e);
  } }, readableLength: { __proto__: null, enumerable: false, get() {
    return this._readableState.length;
  } }, readableObjectMode: { __proto__: null, enumerable: false, get() {
    return this._readableState ? this._readableState.objectMode : false;
  } }, readableEncoding: { __proto__: null, enumerable: false, get() {
    return this._readableState ? this._readableState.encoding : null;
  } }, errored: { __proto__: null, enumerable: false, get() {
    return this._readableState ? this._readableState.errored : null;
  } }, closed: { __proto__: null, get() {
    return this._readableState ? this._readableState.closed : false;
  } }, destroyed: { __proto__: null, enumerable: false, get() {
    return this._readableState ? this._readableState.destroyed : false;
  }, set(e) {
    !this._readableState || (this._readableState.destroyed = e);
  } }, readableEnded: { __proto__: null, enumerable: false, get() {
    return this._readableState ? this._readableState.endEmitted : false;
  } } });
  Ji(Ir.prototype, { pipesCount: { __proto__: null, get() {
    return this.pipes.length;
  } }, paused: { __proto__: null, get() {
    return this[xe] !== false;
  }, set(e) {
    this[xe] = !!e;
  } } });
  g._fromList = oo;
  function oo(e, t) {
    if (t.length === 0)
      return null;
    let r;
    return t.objectMode ? r = t.buffer.shift() : !e || e >= t.length ? (t.decoder ? r = t.buffer.join("") : t.buffer.length === 1 ? r = t.buffer.first() : r = t.buffer.concat(t.length), t.buffer.clear()) : r = t.buffer.consume(e, t.decoder), r;
  }
  function Rr(e) {
    let t = e._readableState;
    _("endReadable", t.endEmitted), t.endEmitted || (t.ended = true, $.nextTick(ya, t, e));
  }
  function ya(e, t) {
    if (_("endReadableNT", e.endEmitted, e.length), !e.errored && !e.closeEmitted && !e.endEmitted && e.length === 0) {
      if (e.endEmitted = true, t.emit("end"), t.writable && t.allowHalfOpen === false)
        $.nextTick(wa, t);
      else if (e.autoDestroy) {
        let r = t._writableState;
        (!r || r.autoDestroy && (r.finished || r.writable === false)) && t.destroy();
      }
    }
  }
  function wa(e) {
    e.writable && !e.writableEnded && !e.destroyed && e.end();
  }
  g.from = function(e, t) {
    return oa(g, e, t);
  };
  var Ar;
  function lo() {
    return Ar === undefined && (Ar = {}), Ar;
  }
  g.fromWeb = function(e, t) {
    return lo().newStreamReadableFromReadableStream(e, t);
  };
  g.toWeb = function(e, t) {
    return lo().newReadableStreamFromStreamReadable(e, t);
  };
  g.wrap = function(e, t) {
    var r, n;
    return new g({ objectMode: (r = (n = e.readableObjectMode) !== null && n !== undefined ? n : e.objectMode) !== null && r !== undefined ? r : true, ...t, destroy(i, o) {
      Ue.destroyer(e, i), o(i);
    } }).wrap(e);
  };
});
var Cr = E((ad, Eo) => {
  var Re = (se(), pe(k)), { ArrayPrototypeSlice: ao, Error: ba, FunctionPrototypeSymbolHasInstance: co, ObjectDefineProperty: ho, ObjectDefineProperties: ga, ObjectSetPrototypeOf: po, StringPrototypeToLowerCase: _a, Symbol: Ea, SymbolHasInstance: Sa } = I();
  Eo.exports = m;
  m.WritableState = Je;
  var { EventEmitter: ma } = Et(), ze = xt().Stream, { Buffer: Ft } = te(), Ot = Se(), { addAbortSignal: xa } = At(), { getHighWaterMark: Ra, getDefaultHighWaterMark: Aa } = Tt(), { ERR_INVALID_ARG_TYPE: Ia, ERR_METHOD_NOT_IMPLEMENTED: Ta, ERR_MULTIPLE_CALLBACK: yo, ERR_STREAM_CANNOT_PIPE: Ba, ERR_STREAM_DESTROYED: Xe, ERR_STREAM_ALREADY_FINISHED: La, ERR_STREAM_NULL_VALUES: Na, ERR_STREAM_WRITE_AFTER_END: Fa, ERR_UNKNOWN_ENCODING: wo } = C().codes, { errorOrDestroy: ve } = Ot;
  po(m.prototype, ze.prototype);
  po(m, ze);
  function Lr() {
  }
  var qe = Ea("kOnFinished");
  function Je(e, t, r) {
    typeof r != "boolean" && (r = t instanceof J()), this.objectMode = !!(e && e.objectMode), r && (this.objectMode = this.objectMode || !!(e && e.writableObjectMode)), this.highWaterMark = e ? Ra(this, e, "writableHighWaterMark", r) : Aa(false), this.finalCalled = false, this.needDrain = false, this.ending = false, this.ended = false, this.finished = false, this.destroyed = false;
    let n = !!(e && e.decodeStrings === false);
    this.decodeStrings = !n, this.defaultEncoding = e && e.defaultEncoding || "utf8", this.length = 0, this.writing = false, this.corked = 0, this.sync = true, this.bufferProcessing = false, this.onwrite = Ca.bind(undefined, t), this.writecb = null, this.writelen = 0, this.afterWriteTickInfo = null, Ct(this), this.pendingcb = 0, this.constructed = true, this.prefinished = false, this.errorEmitted = false, this.emitClose = !e || e.emitClose !== false, this.autoDestroy = !e || e.autoDestroy !== false, this.errored = null, this.closed = false, this.closeEmitted = false, this[qe] = [];
  }
  function Ct(e) {
    e.buffered = [], e.bufferedIndex = 0, e.allBuffers = true, e.allNoop = true;
  }
  Je.prototype.getBuffer = function() {
    return ao(this.buffered, this.bufferedIndex);
  };
  ho(Je.prototype, "bufferedRequestCount", { __proto__: null, get() {
    return this.buffered.length - this.bufferedIndex;
  } });
  function m(e) {
    let t = this instanceof J();
    if (!t && !co(m, this))
      return new m(e);
    this._writableState = new Je(e, this, t), e && (typeof e.write == "function" && (this._write = e.write), typeof e.writev == "function" && (this._writev = e.writev), typeof e.destroy == "function" && (this._destroy = e.destroy), typeof e.final == "function" && (this._final = e.final), typeof e.construct == "function" && (this._construct = e.construct), e.signal && xa(e.signal, this)), ze.call(this, e), Ot.construct(this, () => {
      let r = this._writableState;
      r.writing || Fr(this, r), Mr(this, r);
    });
  }
  ho(m, Sa, { __proto__: null, value: function(e) {
    return co(this, e) ? true : this !== m ? false : e && e._writableState instanceof Je;
  } });
  m.prototype.pipe = function() {
    ve(this, new Ba);
  };
  function bo(e, t, r, n) {
    let i = e._writableState;
    if (typeof r == "function")
      n = r, r = i.defaultEncoding;
    else {
      if (!r)
        r = i.defaultEncoding;
      else if (r !== "buffer" && !Ft.isEncoding(r))
        throw new wo(r);
      typeof n != "function" && (n = Lr);
    }
    if (t === null)
      throw new Na;
    if (!i.objectMode)
      if (typeof t == "string")
        i.decodeStrings !== false && (t = Ft.from(t, r), r = "buffer");
      else if (t instanceof Ft)
        r = "buffer";
      else if (ze._isUint8Array(t))
        t = ze._uint8ArrayToBuffer(t), r = "buffer";
      else
        throw new Ia("chunk", ["string", "Buffer", "Uint8Array"], t);
    let o;
    return i.ending ? o = new Fa : i.destroyed && (o = new Xe("write")), o ? (Re.nextTick(n, o), ve(e, o, true), o) : (i.pendingcb++, Ma(e, i, t, r, n));
  }
  m.prototype.write = function(e, t, r) {
    return bo(this, e, t, r) === true;
  };
  m.prototype.cork = function() {
    this._writableState.corked++;
  };
  m.prototype.uncork = function() {
    let e = this._writableState;
    e.corked && (e.corked--, e.writing || Fr(this, e));
  };
  m.prototype.setDefaultEncoding = function(t) {
    if (typeof t == "string" && (t = _a(t)), !Ft.isEncoding(t))
      throw new wo(t);
    return this._writableState.defaultEncoding = t, this;
  };
  function Ma(e, t, r, n, i) {
    let o = t.objectMode ? 1 : r.length;
    t.length += o;
    let l = t.length < t.highWaterMark;
    return l || (t.needDrain = true), t.writing || t.corked || t.errored || !t.constructed ? (t.buffered.push({ chunk: r, encoding: n, callback: i }), t.allBuffers && n !== "buffer" && (t.allBuffers = false), t.allNoop && i !== Lr && (t.allNoop = false)) : (t.writelen = o, t.writecb = i, t.writing = true, t.sync = true, e._write(r, n, t.onwrite), t.sync = false), l && !t.errored && !t.destroyed;
  }
  function fo(e, t, r, n, i, o, l) {
    t.writelen = n, t.writecb = l, t.writing = true, t.sync = true, t.destroyed ? t.onwrite(new Xe("write")) : r ? e._writev(i, t.onwrite) : e._write(i, o, t.onwrite), t.sync = false;
  }
  function so(e, t, r, n) {
    --t.pendingcb, n(r), Nr(t), ve(e, r);
  }
  function Ca(e, t) {
    let r = e._writableState, n = r.sync, i = r.writecb;
    if (typeof i != "function") {
      ve(e, new yo);
      return;
    }
    r.writing = false, r.writecb = null, r.length -= r.writelen, r.writelen = 0, t ? (t.stack, r.errored || (r.errored = t), e._readableState && !e._readableState.errored && (e._readableState.errored = t), n ? Re.nextTick(so, e, r, t, i) : so(e, r, t, i)) : (r.buffered.length > r.bufferedIndex && Fr(e, r), n ? r.afterWriteTickInfo !== null && r.afterWriteTickInfo.cb === i ? r.afterWriteTickInfo.count++ : (r.afterWriteTickInfo = { count: 1, cb: i, stream: e, state: r }, Re.nextTick(Oa, r.afterWriteTickInfo)) : go(e, r, 1, i));
  }
  function Oa({ stream: e, state: t, count: r, cb: n }) {
    return t.afterWriteTickInfo = null, go(e, t, r, n);
  }
  function go(e, t, r, n) {
    for (!t.ending && !e.destroyed && t.length === 0 && t.needDrain && (t.needDrain = false, e.emit("drain"));r-- > 0; )
      t.pendingcb--, n();
    t.destroyed && Nr(t), Mr(e, t);
  }
  function Nr(e) {
    if (e.writing)
      return;
    for (let i = e.bufferedIndex;i < e.buffered.length; ++i) {
      var t;
      let { chunk: o, callback: l } = e.buffered[i], u = e.objectMode ? 1 : o.length;
      e.length -= u, l((t = e.errored) !== null && t !== undefined ? t : new Xe("write"));
    }
    let r = e[qe].splice(0);
    for (let i = 0;i < r.length; i++) {
      var n;
      r[i]((n = e.errored) !== null && n !== undefined ? n : new Xe("end"));
    }
    Ct(e);
  }
  function Fr(e, t) {
    if (t.corked || t.bufferProcessing || t.destroyed || !t.constructed)
      return;
    let { buffered: r, bufferedIndex: n, objectMode: i } = t, o = r.length - n;
    if (!o)
      return;
    let l = n;
    if (t.bufferProcessing = true, o > 1 && e._writev) {
      t.pendingcb -= o - 1;
      let u = t.allNoop ? Lr : (s) => {
        for (let d = l;d < r.length; ++d)
          r[d].callback(s);
      }, f = t.allNoop && l === 0 ? r : ao(r, l);
      f.allBuffers = t.allBuffers, fo(e, t, true, t.length, f, "", u), Ct(t);
    } else {
      do {
        let { chunk: u, encoding: f, callback: s } = r[l];
        r[l++] = null;
        let d = i ? 1 : u.length;
        fo(e, t, false, d, u, f, s);
      } while (l < r.length && !t.writing);
      l === r.length ? Ct(t) : l > 256 ? (r.splice(0, l), t.bufferedIndex = 0) : t.bufferedIndex = l;
    }
    t.bufferProcessing = false;
  }
  m.prototype._write = function(e, t, r) {
    if (this._writev)
      this._writev([{ chunk: e, encoding: t }], r);
    else
      throw new Ta("_write()");
  };
  m.prototype._writev = null;
  m.prototype.end = function(e, t, r) {
    let n = this._writableState;
    typeof e == "function" ? (r = e, e = null, t = null) : typeof t == "function" && (r = t, t = null);
    let i;
    if (e != null) {
      let o = bo(this, e, t);
      o instanceof ba && (i = o);
    }
    return n.corked && (n.corked = 1, this.uncork()), i || (!n.errored && !n.ending ? (n.ending = true, Mr(this, n, true), n.ended = true) : n.finished ? i = new La("end") : n.destroyed && (i = new Xe("end"))), typeof r == "function" && (i || n.finished ? Re.nextTick(r, i) : n[qe].push(r)), this;
  };
  function Mt(e) {
    return e.ending && !e.destroyed && e.constructed && e.length === 0 && !e.errored && e.buffered.length === 0 && !e.finished && !e.writing && !e.errorEmitted && !e.closeEmitted;
  }
  function Da(e, t) {
    let r = false;
    function n(i) {
      if (r) {
        ve(e, i ?? yo());
        return;
      }
      if (r = true, t.pendingcb--, i) {
        let o = t[qe].splice(0);
        for (let l = 0;l < o.length; l++)
          o[l](i);
        ve(e, i, t.sync);
      } else
        Mt(t) && (t.prefinished = true, e.emit("prefinish"), t.pendingcb++, Re.nextTick(Br, e, t));
    }
    t.sync = true, t.pendingcb++;
    try {
      e._final(n);
    } catch (i) {
      n(i);
    }
    t.sync = false;
  }
  function Pa(e, t) {
    !t.prefinished && !t.finalCalled && (typeof e._final == "function" && !t.destroyed ? (t.finalCalled = true, Da(e, t)) : (t.prefinished = true, e.emit("prefinish")));
  }
  function Mr(e, t, r) {
    Mt(t) && (Pa(e, t), t.pendingcb === 0 && (r ? (t.pendingcb++, Re.nextTick((n, i) => {
      Mt(i) ? Br(n, i) : i.pendingcb--;
    }, e, t)) : Mt(t) && (t.pendingcb++, Br(e, t))));
  }
  function Br(e, t) {
    t.pendingcb--, t.finished = true;
    let r = t[qe].splice(0);
    for (let n = 0;n < r.length; n++)
      r[n]();
    if (e.emit("finish"), t.autoDestroy) {
      let n = e._readableState;
      (!n || n.autoDestroy && (n.endEmitted || n.readable === false)) && e.destroy();
    }
  }
  ga(m.prototype, { closed: { __proto__: null, get() {
    return this._writableState ? this._writableState.closed : false;
  } }, destroyed: { __proto__: null, get() {
    return this._writableState ? this._writableState.destroyed : false;
  }, set(e) {
    this._writableState && (this._writableState.destroyed = e);
  } }, writable: { __proto__: null, get() {
    let e = this._writableState;
    return !!e && e.writable !== false && !e.destroyed && !e.errored && !e.ending && !e.ended;
  }, set(e) {
    this._writableState && (this._writableState.writable = !!e);
  } }, writableFinished: { __proto__: null, get() {
    return this._writableState ? this._writableState.finished : false;
  } }, writableObjectMode: { __proto__: null, get() {
    return this._writableState ? this._writableState.objectMode : false;
  } }, writableBuffer: { __proto__: null, get() {
    return this._writableState && this._writableState.getBuffer();
  } }, writableEnded: { __proto__: null, get() {
    return this._writableState ? this._writableState.ending : false;
  } }, writableNeedDrain: { __proto__: null, get() {
    let e = this._writableState;
    return e ? !e.destroyed && !e.ending && e.needDrain : false;
  } }, writableHighWaterMark: { __proto__: null, get() {
    return this._writableState && this._writableState.highWaterMark;
  } }, writableCorked: { __proto__: null, get() {
    return this._writableState ? this._writableState.corked : 0;
  } }, writableLength: { __proto__: null, get() {
    return this._writableState && this._writableState.length;
  } }, errored: { __proto__: null, enumerable: false, get() {
    return this._writableState ? this._writableState.errored : null;
  } }, writableAborted: { __proto__: null, enumerable: false, get: function() {
    return !!(this._writableState.writable !== false && (this._writableState.destroyed || this._writableState.errored) && !this._writableState.finished);
  } } });
  var ka = Ot.destroy;
  m.prototype.destroy = function(e, t) {
    let r = this._writableState;
    return !r.destroyed && (r.bufferedIndex < r.buffered.length || r[qe].length) && Re.nextTick(Nr, r), ka.call(this, e, t), this;
  };
  m.prototype._undestroy = Ot.undestroy;
  m.prototype._destroy = function(e, t) {
    t(e);
  };
  m.prototype[ma.captureRejectionSymbol] = function(e) {
    this.destroy(e);
  };
  var Tr;
  function _o() {
    return Tr === undefined && (Tr = {}), Tr;
  }
  m.fromWeb = function(e, t) {
    return _o().newStreamWritableFromWritableStream(e, t);
  };
  m.toWeb = function(e) {
    return _o().newWritableStreamFromStreamWritable(e);
  };
});
var Mo = E((cd, Fo) => {
  var Or = (se(), pe(k)), Ua = te(), { isReadable: va, isWritable: qa, isIterable: So, isNodeStream: Wa, isReadableNodeStream: mo, isWritableNodeStream: xo, isDuplexNodeStream: $a } = ae(), Ro = ce(), { AbortError: No, codes: { ERR_INVALID_ARG_TYPE: ja, ERR_INVALID_RETURN_VALUE: Ao } } = C(), { destroyer: We } = Se(), Ga = J(), Ha = Ke(), { createDeferredPromise: Io } = V(), To = Er(), Bo = globalThis.Blob || Ua.Blob, Va = typeof Bo < "u" ? function(t) {
    return t instanceof Bo;
  } : function(t) {
    return false;
  }, Ya = globalThis.AbortController || ut().AbortController, { FunctionPrototypeCall: Lo } = I(), Ae = class extends Ga {
    constructor(t) {
      super(t), t?.readable === false && (this._readableState.readable = false, this._readableState.ended = true, this._readableState.endEmitted = true), t?.writable === false && (this._writableState.writable = false, this._writableState.ending = true, this._writableState.ended = true, this._writableState.finished = true);
    }
  };
  Fo.exports = function e(t, r) {
    if ($a(t))
      return t;
    if (mo(t))
      return Dt({ readable: t });
    if (xo(t))
      return Dt({ writable: t });
    if (Wa(t))
      return Dt({ writable: false, readable: false });
    if (typeof t == "function") {
      let { value: i, write: o, final: l, destroy: u } = Ka(t);
      if (So(i))
        return To(Ae, i, { objectMode: true, write: o, final: l, destroy: u });
      let f = i?.then;
      if (typeof f == "function") {
        let s, d = Lo(f, i, (c) => {
          if (c != null)
            throw new Ao("nully", "body", c);
        }, (c) => {
          We(s, c);
        });
        return s = new Ae({ objectMode: true, readable: false, write: o, final(c) {
          l(async () => {
            try {
              await d, Or.nextTick(c, null);
            } catch (y) {
              Or.nextTick(c, y);
            }
          });
        }, destroy: u });
      }
      throw new Ao("Iterable, AsyncIterable or AsyncFunction", r, i);
    }
    if (Va(t))
      return e(t.arrayBuffer());
    if (So(t))
      return To(Ae, t, { objectMode: true, writable: false });
    if (typeof t?.writable == "object" || typeof t?.readable == "object") {
      let i = t != null && t.readable ? mo(t?.readable) ? t?.readable : e(t.readable) : undefined, o = t != null && t.writable ? xo(t?.writable) ? t?.writable : e(t.writable) : undefined;
      return Dt({ readable: i, writable: o });
    }
    let n = t?.then;
    if (typeof n == "function") {
      let i;
      return Lo(n, t, (o) => {
        o != null && i.push(o), i.push(null);
      }, (o) => {
        We(i, o);
      }), i = new Ae({ objectMode: true, writable: false, read() {
      } });
    }
    throw new ja(r, ["Blob", "ReadableStream", "WritableStream", "Stream", "Iterable", "AsyncIterable", "Function", "{ readable, writable } pair", "Promise"], t);
  };
  function Ka(e) {
    let { promise: t, resolve: r } = Io(), n = new Ya, i = n.signal;
    return { value: e(async function* () {
      for (;; ) {
        let l = t;
        t = null;
        let { chunk: u, done: f, cb: s } = await l;
        if (Or.nextTick(s), f)
          return;
        if (i.aborted)
          throw new No(undefined, { cause: i.reason });
        ({ promise: t, resolve: r } = Io()), yield u;
      }
    }(), { signal: i }), write(l, u, f) {
      let s = r;
      r = null, s({ chunk: l, done: false, cb: f });
    }, final(l) {
      let u = r;
      r = null, u({ done: true, cb: l });
    }, destroy(l, u) {
      n.abort(), u(l);
    } };
  }
  function Dt(e) {
    let t = e.readable && typeof e.readable.read != "function" ? Ha.wrap(e.readable) : e.readable, r = e.writable, n = !!va(t), i = !!qa(r), o, l, u, f, s;
    function d(c) {
      let y = f;
      f = null, y ? y(c) : c ? s.destroy(c) : !n && !i && s.destroy();
    }
    return s = new Ae({ readableObjectMode: !!(t != null && t.readableObjectMode), writableObjectMode: !!(r != null && r.writableObjectMode), readable: n, writable: i }), i && (Ro(r, (c) => {
      i = false, c && We(t, c), d(c);
    }), s._write = function(c, y, h) {
      r.write(c, y) ? h() : o = h;
    }, s._final = function(c) {
      r.end(), l = c;
    }, r.on("drain", function() {
      if (o) {
        let c = o;
        o = null, c();
      }
    }), r.on("finish", function() {
      if (l) {
        let c = l;
        l = null, c();
      }
    })), n && (Ro(t, (c) => {
      n = false, c && We(t, c), d(c);
    }), t.on("readable", function() {
      if (u) {
        let c = u;
        u = null, c();
      }
    }), t.on("end", function() {
      s.push(null);
    }), s._read = function() {
      for (;; ) {
        let c = t.read();
        if (c === null) {
          u = s._read;
          return;
        }
        if (!s.push(c))
          return;
      }
    }), s._destroy = function(c, y) {
      !c && f !== null && (c = new No), u = null, o = null, l = null, f === null ? y(c) : (f = y, We(r, c), We(t, c));
    }, s;
  }
});
var J = E((dd, Do) => {
  var { ObjectDefineProperties: za, ObjectGetOwnPropertyDescriptor: ie, ObjectKeys: Xa, ObjectSetPrototypeOf: Co } = I();
  Do.exports = j;
  var kr = Ke(), U = Cr();
  Co(j.prototype, kr.prototype);
  Co(j, kr);
  {
    let e = Xa(U.prototype);
    for (let t = 0;t < e.length; t++) {
      let r = e[t];
      j.prototype[r] || (j.prototype[r] = U.prototype[r]);
    }
  }
  function j(e) {
    if (!(this instanceof j))
      return new j(e);
    kr.call(this, e), U.call(this, e), e ? (this.allowHalfOpen = e.allowHalfOpen !== false, e.readable === false && (this._readableState.readable = false, this._readableState.ended = true, this._readableState.endEmitted = true), e.writable === false && (this._writableState.writable = false, this._writableState.ending = true, this._writableState.ended = true, this._writableState.finished = true)) : this.allowHalfOpen = true;
  }
  za(j.prototype, { writable: { __proto__: null, ...ie(U.prototype, "writable") }, writableHighWaterMark: { __proto__: null, ...ie(U.prototype, "writableHighWaterMark") }, writableObjectMode: { __proto__: null, ...ie(U.prototype, "writableObjectMode") }, writableBuffer: { __proto__: null, ...ie(U.prototype, "writableBuffer") }, writableLength: { __proto__: null, ...ie(U.prototype, "writableLength") }, writableFinished: { __proto__: null, ...ie(U.prototype, "writableFinished") }, writableCorked: { __proto__: null, ...ie(U.prototype, "writableCorked") }, writableEnded: { __proto__: null, ...ie(U.prototype, "writableEnded") }, writableNeedDrain: { __proto__: null, ...ie(U.prototype, "writableNeedDrain") }, destroyed: { __proto__: null, get() {
    return this._readableState === undefined || this._writableState === undefined ? false : this._readableState.destroyed && this._writableState.destroyed;
  }, set(e) {
    this._readableState && this._writableState && (this._readableState.destroyed = e, this._writableState.destroyed = e);
  } } });
  var Dr;
  function Oo() {
    return Dr === undefined && (Dr = {}), Dr;
  }
  j.fromWeb = function(e, t) {
    return Oo().newStreamDuplexFromReadableWritablePair(e, t);
  };
  j.toWeb = function(e) {
    return Oo().newReadableWritablePairFromDuplex(e);
  };
  var Pr;
  j.from = function(e) {
    return Pr || (Pr = Mo()), Pr(e, "body");
  };
});
var qr = E((hd, ko) => {
  var { ObjectSetPrototypeOf: Po, Symbol: Ja } = I();
  ko.exports = oe;
  var { ERR_METHOD_NOT_IMPLEMENTED: Qa } = C().codes, vr = J(), { getHighWaterMark: Za } = Tt();
  Po(oe.prototype, vr.prototype);
  Po(oe, vr);
  var Qe = Ja("kCallback");
  function oe(e) {
    if (!(this instanceof oe))
      return new oe(e);
    let t = e ? Za(this, e, "readableHighWaterMark", true) : null;
    t === 0 && (e = { ...e, highWaterMark: null, readableHighWaterMark: t, writableHighWaterMark: e.writableHighWaterMark || 0 }), vr.call(this, e), this._readableState.sync = false, this[Qe] = null, e && (typeof e.transform == "function" && (this._transform = e.transform), typeof e.flush == "function" && (this._flush = e.flush)), this.on("prefinish", ec);
  }
  function Ur(e) {
    typeof this._flush == "function" && !this.destroyed ? this._flush((t, r) => {
      if (t) {
        e ? e(t) : this.destroy(t);
        return;
      }
      r != null && this.push(r), this.push(null), e && e();
    }) : (this.push(null), e && e());
  }
  function ec() {
    this._final !== Ur && Ur.call(this);
  }
  oe.prototype._final = Ur;
  oe.prototype._transform = function(e, t, r) {
    throw new Qa("_transform()");
  };
  oe.prototype._write = function(e, t, r) {
    let n = this._readableState, i = this._writableState, o = n.length;
    this._transform(e, t, (l, u) => {
      if (l) {
        r(l);
        return;
      }
      u != null && this.push(u), i.ended || o === n.length || n.length < n.highWaterMark ? r() : this[Qe] = r;
    });
  };
  oe.prototype._read = function() {
    if (this[Qe]) {
      let e = this[Qe];
      this[Qe] = null, e();
    }
  };
});
var $r = E((pd, vo) => {
  var { ObjectSetPrototypeOf: Uo } = I();
  vo.exports = $e;
  var Wr = qr();
  Uo($e.prototype, Wr.prototype);
  Uo($e, Wr);
  function $e(e) {
    if (!(this instanceof $e))
      return new $e(e);
    Wr.call(this, e);
  }
  $e.prototype._transform = function(e, t, r) {
    r(null, e);
  };
});
var Ut = E((yd, Vo) => {
  var Pt = (se(), pe(k)), { ArrayIsArray: tc, Promise: rc, SymbolAsyncIterator: nc } = I(), kt = ce(), { once: ic } = V(), oc = Se(), qo = J(), { aggregateTwoErrors: lc, codes: { ERR_INVALID_ARG_TYPE: Go, ERR_INVALID_RETURN_VALUE: jr, ERR_MISSING_ARGS: uc, ERR_STREAM_DESTROYED: fc, ERR_STREAM_PREMATURE_CLOSE: sc }, AbortError: ac } = C(), { validateFunction: cc, validateAbortSignal: dc } = He(), { isIterable: je, isReadable: Gr, isReadableNodeStream: Yr, isNodeStream: Wo } = ae(), hc = globalThis.AbortController || ut().AbortController, Hr, Vr;
  function $o(e, t, r) {
    let n = false;
    e.on("close", () => {
      n = true;
    });
    let i = kt(e, { readable: t, writable: r }, (o) => {
      n = !o;
    });
    return { destroy: (o) => {
      n || (n = true, oc.destroyer(e, o || new fc("pipe")));
    }, cleanup: i };
  }
  function pc(e) {
    return cc(e[e.length - 1], "streams[stream.length - 1]"), e.pop();
  }
  function yc(e) {
    if (je(e))
      return e;
    if (Yr(e))
      return wc(e);
    throw new Go("val", ["Readable", "Iterable", "AsyncIterable"], e);
  }
  async function* wc(e) {
    Vr || (Vr = Ke()), yield* Vr.prototype[nc].call(e);
  }
  async function jo(e, t, r, { end: n }) {
    let i, o = null, l = (s) => {
      if (s && (i = s), o) {
        let d = o;
        o = null, d();
      }
    }, u = () => new rc((s, d) => {
      i ? d(i) : o = () => {
        i ? d(i) : s();
      };
    });
    t.on("drain", l);
    let f = kt(t, { readable: false }, l);
    try {
      t.writableNeedDrain && await u();
      for await (let s of e)
        t.write(s) || await u();
      n && t.end(), await u(), r();
    } catch (s) {
      r(i !== s ? lc(i, s) : s);
    } finally {
      f(), t.off("drain", l);
    }
  }
  function bc(...e) {
    return Ho(e, ic(pc(e)));
  }
  function Ho(e, t, r) {
    if (e.length === 1 && tc(e[0]) && (e = e[0]), e.length < 2)
      throw new uc("streams");
    let n = new hc, i = n.signal, o = r?.signal, l = [];
    dc(o, "options.signal");
    function u() {
      h(new ac);
    }
    o?.addEventListener("abort", u);
    let f, s, d = [], c = 0;
    function y(w) {
      h(w, --c === 0);
    }
    function h(w, b) {
      if (w && (!f || f.code === "ERR_STREAM_PREMATURE_CLOSE") && (f = w), !(!f && !b)) {
        for (;d.length; )
          d.shift()(f);
        o?.removeEventListener("abort", u), n.abort(), b && (f || l.forEach((L) => L()), Pt.nextTick(t, f, s));
      }
    }
    let p;
    for (let w = 0;w < e.length; w++) {
      let b = e[w], L = w < e.length - 1, N = w > 0, Q = L || r?.end !== false, Ie = w === e.length - 1;
      if (Wo(b)) {
        let q = function(Z) {
          Z && Z.name !== "AbortError" && Z.code !== "ERR_STREAM_PREMATURE_CLOSE" && y(Z);
        };
        var v = q;
        if (Q) {
          let { destroy: Z, cleanup: qt } = $o(b, L, N);
          d.push(Z), Gr(b) && Ie && l.push(qt);
        }
        b.on("error", q), Gr(b) && Ie && l.push(() => {
          b.removeListener("error", q);
        });
      }
      if (w === 0)
        if (typeof b == "function") {
          if (p = b({ signal: i }), !je(p))
            throw new jr("Iterable, AsyncIterable or Stream", "source", p);
        } else
          je(b) || Yr(b) ? p = b : p = qo.from(b);
      else if (typeof b == "function")
        if (p = yc(p), p = b(p, { signal: i }), L) {
          if (!je(p, true))
            throw new jr("AsyncIterable", `transform[${w - 1}]`, p);
        } else {
          var B;
          Hr || (Hr = $r());
          let q = new Hr({ objectMode: true }), Z = (B = p) === null || B === undefined ? undefined : B.then;
          if (typeof Z == "function")
            c++, Z.call(p, (Te) => {
              s = Te, Te != null && q.write(Te), Q && q.end(), Pt.nextTick(y);
            }, (Te) => {
              q.destroy(Te), Pt.nextTick(y, Te);
            });
          else if (je(p, true))
            c++, jo(p, q, y, { end: Q });
          else
            throw new jr("AsyncIterable or Promise", "destination", p);
          p = q;
          let { destroy: qt, cleanup: sl } = $o(p, false, true);
          d.push(qt), Ie && l.push(sl);
        }
      else if (Wo(b)) {
        if (Yr(p)) {
          c += 2;
          let q = gc(p, b, y, { end: Q });
          Gr(b) && Ie && l.push(q);
        } else if (je(p))
          c++, jo(p, b, y, { end: Q });
        else
          throw new Go("val", ["Readable", "Iterable", "AsyncIterable"], p);
        p = b;
      } else
        p = qo.from(b);
    }
    return (i != null && i.aborted || o != null && o.aborted) && Pt.nextTick(u), p;
  }
  function gc(e, t, r, { end: n }) {
    let i = false;
    return t.on("close", () => {
      i || r(new sc);
    }), e.pipe(t, { end: n }), n ? e.once("end", () => {
      i = true, t.end();
    }) : r(), kt(e, { readable: true, writable: false }, (o) => {
      let l = e._readableState;
      o && o.code === "ERR_STREAM_PREMATURE_CLOSE" && l && l.ended && !l.errored && !l.errorEmitted ? e.once("end", r).once("error", r) : r(o);
    }), kt(t, { readable: false, writable: true }, r);
  }
  Vo.exports = { pipelineImpl: Ho, pipeline: bc };
});
var Jo = E((wd, Xo) => {
  var { pipeline: _c } = Ut(), vt = J(), { destroyer: Ec } = Se(), { isNodeStream: Sc, isReadable: Yo, isWritable: Ko } = ae(), { AbortError: mc, codes: { ERR_INVALID_ARG_VALUE: zo, ERR_MISSING_ARGS: xc } } = C();
  Xo.exports = function(...t) {
    if (t.length === 0)
      throw new xc("streams");
    if (t.length === 1)
      return vt.from(t[0]);
    let r = [...t];
    if (typeof t[0] == "function" && (t[0] = vt.from(t[0])), typeof t[t.length - 1] == "function") {
      let h = t.length - 1;
      t[h] = vt.from(t[h]);
    }
    for (let h = 0;h < t.length; ++h)
      if (!!Sc(t[h])) {
        if (h < t.length - 1 && !Yo(t[h]))
          throw new zo(`streams[${h}]`, r[h], "must be readable");
        if (h > 0 && !Ko(t[h]))
          throw new zo(`streams[${h}]`, r[h], "must be writable");
      }
    let n, i, o, l, u;
    function f(h) {
      let p = l;
      l = null, p ? p(h) : h ? u.destroy(h) : !y && !c && u.destroy();
    }
    let s = t[0], d = _c(t, f), c = !!Ko(s), y = !!Yo(d);
    return u = new vt({ writableObjectMode: !!(s != null && s.writableObjectMode), readableObjectMode: !!(d != null && d.writableObjectMode), writable: c, readable: y }), c && (u._write = function(h, p, B) {
      s.write(h, p) ? B() : n = B;
    }, u._final = function(h) {
      s.end(), i = h;
    }, s.on("drain", function() {
      if (n) {
        let h = n;
        n = null, h();
      }
    }), d.on("finish", function() {
      if (i) {
        let h = i;
        i = null, h();
      }
    })), y && (d.on("readable", function() {
      if (o) {
        let h = o;
        o = null, h();
      }
    }), d.on("end", function() {
      u.push(null);
    }), u._read = function() {
      for (;; ) {
        let h = d.read();
        if (h === null) {
          o = u._read;
          return;
        }
        if (!u.push(h))
          return;
      }
    }), u._destroy = function(h, p) {
      !h && l !== null && (h = new mc), o = null, n = null, i = null, l === null ? p(h) : (l = p, Ec(d, h));
    }, u;
  };
});
var Kr = E((bd, Qo) => {
  var { ArrayPrototypePop: Rc, Promise: Ac } = I(), { isIterable: Ic, isNodeStream: Tc } = ae(), { pipelineImpl: Bc } = Ut(), { finished: Lc } = ce();
  function Nc(...e) {
    return new Ac((t, r) => {
      let n, i, o = e[e.length - 1];
      if (o && typeof o == "object" && !Tc(o) && !Ic(o)) {
        let l = Rc(e);
        n = l.signal, i = l.end;
      }
      Bc(e, (l, u) => {
        l ? r(l) : t(u);
      }, { signal: n, end: i });
    });
  }
  Qo.exports = { finished: Lc, pipeline: Nc };
});
var fl = E((gd, ul) => {
  var { Buffer: Fc } = te(), { ObjectDefineProperty: le, ObjectKeys: tl, ReflectApply: rl } = I(), { promisify: { custom: nl } } = V(), { streamReturningOperators: Zo, promiseReturningOperators: el } = di(), { codes: { ERR_ILLEGAL_CONSTRUCTOR: il } } = C(), Mc = Jo(), { pipeline: ol } = Ut(), { destroyer: Cc } = Se(), ll = ce(), zr = Kr(), Xr = ae(), R = ul.exports = xt().Stream;
  R.isDisturbed = Xr.isDisturbed;
  R.isErrored = Xr.isErrored;
  R.isReadable = Xr.isReadable;
  R.Readable = Ke();
  for (let e of tl(Zo)) {
    let r = function(...n) {
      if (new.target)
        throw il();
      return R.Readable.from(rl(t, this, n));
    };
    Dc = r;
    let t = Zo[e];
    le(r, "name", { __proto__: null, value: t.name }), le(r, "length", { __proto__: null, value: t.length }), le(R.Readable.prototype, e, { __proto__: null, value: r, enumerable: false, configurable: true, writable: true });
  }
  var Dc;
  for (let e of tl(el)) {
    let r = function(...i) {
      if (new.target)
        throw il();
      return rl(t, this, i);
    };
    Dc = r;
    let t = el[e];
    le(r, "name", { __proto__: null, value: t.name }), le(r, "length", { __proto__: null, value: t.length }), le(R.Readable.prototype, e, { __proto__: null, value: r, enumerable: false, configurable: true, writable: true });
  }
  var Dc;
  R.Writable = Cr();
  R.Duplex = J();
  R.Transform = qr();
  R.PassThrough = $r();
  R.pipeline = ol;
  var { addAbortSignal: Oc } = At();
  R.addAbortSignal = Oc;
  R.finished = ll;
  R.destroy = Cc;
  R.compose = Mc;
  le(R, "promises", { __proto__: null, configurable: true, enumerable: true, get() {
    return zr;
  } });
  le(ol, nl, { __proto__: null, enumerable: true, get() {
    return zr.pipeline;
  } });
  le(ll, nl, { __proto__: null, enumerable: true, get() {
    return zr.finished;
  } });
  R.Stream = R;
  R._isUint8Array = function(t) {
    return t instanceof Uint8Array;
  };
  R._uint8ArrayToBuffer = function(t) {
    return Fc.from(t.buffer, t.byteOffset, t.byteLength);
  };
});
var Jr = E((_d, A) => {
  var T = fl(), Pc = Kr(), kc = T.Readable.destroy;
  A.exports = T.Readable;
  A.exports._uint8ArrayToBuffer = T._uint8ArrayToBuffer;
  A.exports._isUint8Array = T._isUint8Array;
  A.exports.isDisturbed = T.isDisturbed;
  A.exports.isErrored = T.isErrored;
  A.exports.isReadable = T.isReadable;
  A.exports.Readable = T.Readable;
  A.exports.Writable = T.Writable;
  A.exports.Duplex = T.Duplex;
  A.exports.Transform = T.Transform;
  A.exports.PassThrough = T.PassThrough;
  A.exports.addAbortSignal = T.addAbortSignal;
  A.exports.finished = T.finished;
  A.exports.destroy = T.destroy;
  A.exports.destroy = kc;
  A.exports.pipeline = T.pipeline;
  A.exports.compose = T.compose;
  Object.defineProperty(T, "promises", { configurable: true, enumerable: true, get() {
    return Pc;
  } });
  A.exports.Stream = T.Stream;
  A.exports.default = A.exports;
});
var Ze = {};
Qr(Ze, { default: () => Uc });
ue(Ze, rt(Jr()));
var Uc = rt(Jr());
/*!
 * The buffer module from node.js, for the browser.
 *
 * @author   Feross Aboukhadijeh <https://feross.org>
 * @license  MIT
 */
/*! ieee754. BSD-3-Clause License. Feross Aboukhadijeh <https://feross.org/opensource> */
/*! safe-buffer. MIT License. Feross Aboukhadijeh <https://feross.org/opensource> */
export {
  Uc as default
};
