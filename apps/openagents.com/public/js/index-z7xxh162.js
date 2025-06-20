import"./chat-client-13a4mv5g.js";

// node:zlib
var Vy = Object.create;
var Dn = Object.defineProperty;
var Yy = Object.getOwnPropertyDescriptor;
var Ky = Object.getOwnPropertyNames;
var Xy = Object.getPrototypeOf;
var Jy = Object.prototype.hasOwnProperty;
var wo = (e, t) => () => (e && (t = e(e = 0)), t);
var g = (e, t) => () => (t || e((t = { exports: {} }).exports, t), t.exports);
var Bn = (e, t) => {
  for (var r in t)
    Dn(e, r, { get: t[r], enumerable: true });
};
var Ln = (e, t, r, n) => {
  if (t && typeof t == "object" || typeof t == "function")
    for (let i of Ky(t))
      !Jy.call(e, i) && i !== r && Dn(e, i, { get: () => t[i], enumerable: !(n = Yy(t, i)) || n.enumerable });
  return e;
};
var X = (e, t, r) => (Ln(e, t, "default"), r && Ln(r, t, "default"));
var vt = (e, t, r) => (r = e != null ? Vy(Xy(e)) : {}, Ln(t || !e || !e.__esModule ? Dn(r, "default", { value: e, enumerable: true }) : r, e));
var se = (e) => Ln(Dn({}, "__esModule", { value: true }), e);
var sl = g((Pn) => {
  Pn.byteLength = e_;
  Pn.toByteArray = r_;
  Pn.fromByteArray = o_;
  var De = [], pe = [], Qy = typeof Uint8Array < "u" ? Uint8Array : Array, Eo = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  for (mt = 0, ll = Eo.length;mt < ll; ++mt)
    De[mt] = Eo[mt], pe[Eo.charCodeAt(mt)] = mt;
  var mt, ll;
  pe[45] = 62;
  pe[95] = 63;
  function ul(e) {
    var t = e.length;
    if (t % 4 > 0)
      throw new Error("Invalid string. Length must be a multiple of 4");
    var r = e.indexOf("=");
    r === -1 && (r = t);
    var n = r === t ? 0 : 4 - r % 4;
    return [r, n];
  }
  function e_(e) {
    var t = ul(e), r = t[0], n = t[1];
    return (r + n) * 3 / 4 - n;
  }
  function t_(e, t, r) {
    return (t + r) * 3 / 4 - r;
  }
  function r_(e) {
    var t, r = ul(e), n = r[0], i = r[1], o = new Qy(t_(e, n, i)), a = 0, f = i > 0 ? n - 4 : n, u;
    for (u = 0;u < f; u += 4)
      t = pe[e.charCodeAt(u)] << 18 | pe[e.charCodeAt(u + 1)] << 12 | pe[e.charCodeAt(u + 2)] << 6 | pe[e.charCodeAt(u + 3)], o[a++] = t >> 16 & 255, o[a++] = t >> 8 & 255, o[a++] = t & 255;
    return i === 2 && (t = pe[e.charCodeAt(u)] << 2 | pe[e.charCodeAt(u + 1)] >> 4, o[a++] = t & 255), i === 1 && (t = pe[e.charCodeAt(u)] << 10 | pe[e.charCodeAt(u + 1)] << 4 | pe[e.charCodeAt(u + 2)] >> 2, o[a++] = t >> 8 & 255, o[a++] = t & 255), o;
  }
  function n_(e) {
    return De[e >> 18 & 63] + De[e >> 12 & 63] + De[e >> 6 & 63] + De[e & 63];
  }
  function i_(e, t, r) {
    for (var n, i = [], o = t;o < r; o += 3)
      n = (e[o] << 16 & 16711680) + (e[o + 1] << 8 & 65280) + (e[o + 2] & 255), i.push(n_(n));
    return i.join("");
  }
  function o_(e) {
    for (var t, r = e.length, n = r % 3, i = [], o = 16383, a = 0, f = r - n;a < f; a += o)
      i.push(i_(e, a, a + o > f ? f : a + o));
    return n === 1 ? (t = e[r - 1], i.push(De[t >> 2] + De[t << 4 & 63] + "==")) : n === 2 && (t = (e[r - 2] << 8) + e[r - 1], i.push(De[t >> 10] + De[t >> 4 & 63] + De[t << 2 & 63] + "=")), i.join("");
  }
});
var cl = g((vo) => {
  vo.read = function(e, t, r, n, i) {
    var o, a, f = i * 8 - n - 1, u = (1 << f) - 1, l = u >> 1, s = -7, c = r ? i - 1 : 0, h = r ? -1 : 1, d = e[t + c];
    for (c += h, o = d & (1 << -s) - 1, d >>= -s, s += f;s > 0; o = o * 256 + e[t + c], c += h, s -= 8)
      ;
    for (a = o & (1 << -s) - 1, o >>= -s, s += n;s > 0; a = a * 256 + e[t + c], c += h, s -= 8)
      ;
    if (o === 0)
      o = 1 - l;
    else {
      if (o === u)
        return a ? NaN : (d ? -1 : 1) * (1 / 0);
      a = a + Math.pow(2, n), o = o - l;
    }
    return (d ? -1 : 1) * a * Math.pow(2, o - n);
  };
  vo.write = function(e, t, r, n, i, o) {
    var a, f, u, l = o * 8 - i - 1, s = (1 << l) - 1, c = s >> 1, h = i === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0, d = n ? 0 : o - 1, y = n ? 1 : -1, b = t < 0 || t === 0 && 1 / t < 0 ? 1 : 0;
    for (t = Math.abs(t), isNaN(t) || t === 1 / 0 ? (f = isNaN(t) ? 1 : 0, a = s) : (a = Math.floor(Math.log(t) / Math.LN2), t * (u = Math.pow(2, -a)) < 1 && (a--, u *= 2), a + c >= 1 ? t += h / u : t += h * Math.pow(2, 1 - c), t * u >= 2 && (a++, u /= 2), a + c >= s ? (f = 0, a = s) : a + c >= 1 ? (f = (t * u - 1) * Math.pow(2, i), a = a + c) : (f = t * Math.pow(2, c - 1) * Math.pow(2, i), a = 0));i >= 8; e[r + d] = f & 255, d += y, f /= 256, i -= 8)
      ;
    for (a = a << i | f, l += i;l > 0; e[r + d] = a & 255, d += y, a /= 256, l -= 8)
      ;
    e[r + d - y] |= b * 128;
  };
});
var xe = g((or) => {
  var mo = sl(), nr = cl(), dl = typeof Symbol == "function" && typeof Symbol.for == "function" ? Symbol.for("nodejs.util.inspect.custom") : null;
  or.Buffer = p;
  or.SlowBuffer = c_;
  or.INSPECT_MAX_BYTES = 50;
  var Mn = 2147483647;
  or.kMaxLength = Mn;
  p.TYPED_ARRAY_SUPPORT = a_();
  !p.TYPED_ARRAY_SUPPORT && typeof console < "u" && typeof console.error == "function" && console.error("This browser lacks typed array (Uint8Array) support which is required by `buffer` v5.x. Use `buffer` v4.x if you require old browser support.");
  function a_() {
    try {
      let e = new Uint8Array(1), t = { foo: function() {
        return 42;
      } };
      return Object.setPrototypeOf(t, Uint8Array.prototype), Object.setPrototypeOf(e, t), e.foo() === 42;
    } catch {
      return false;
    }
  }
  Object.defineProperty(p.prototype, "parent", { enumerable: true, get: function() {
    if (!!p.isBuffer(this))
      return this.buffer;
  } });
  Object.defineProperty(p.prototype, "offset", { enumerable: true, get: function() {
    if (!!p.isBuffer(this))
      return this.byteOffset;
  } });
  function Ye(e) {
    if (e > Mn)
      throw new RangeError('The value "' + e + '" is invalid for option "size"');
    let t = new Uint8Array(e);
    return Object.setPrototypeOf(t, p.prototype), t;
  }
  function p(e, t, r) {
    if (typeof e == "number") {
      if (typeof t == "string")
        throw new TypeError('The "string" argument must be of type string. Received type number');
      return Ro(e);
    }
    return _l(e, t, r);
  }
  p.poolSize = 8192;
  function _l(e, t, r) {
    if (typeof e == "string")
      return l_(e, t);
    if (ArrayBuffer.isView(e))
      return u_(e);
    if (e == null)
      throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof e);
    if (Be(e, ArrayBuffer) || e && Be(e.buffer, ArrayBuffer) || typeof SharedArrayBuffer < "u" && (Be(e, SharedArrayBuffer) || e && Be(e.buffer, SharedArrayBuffer)))
      return Ao(e, t, r);
    if (typeof e == "number")
      throw new TypeError('The "value" argument must not be of type number. Received type number');
    let n = e.valueOf && e.valueOf();
    if (n != null && n !== e)
      return p.from(n, t, r);
    let i = s_(e);
    if (i)
      return i;
    if (typeof Symbol < "u" && Symbol.toPrimitive != null && typeof e[Symbol.toPrimitive] == "function")
      return p.from(e[Symbol.toPrimitive]("string"), t, r);
    throw new TypeError("The first argument must be one of type string, Buffer, ArrayBuffer, Array, or Array-like Object. Received type " + typeof e);
  }
  p.from = function(e, t, r) {
    return _l(e, t, r);
  };
  Object.setPrototypeOf(p.prototype, Uint8Array.prototype);
  Object.setPrototypeOf(p, Uint8Array);
  function gl(e) {
    if (typeof e != "number")
      throw new TypeError('"size" argument must be of type number');
    if (e < 0)
      throw new RangeError('The value "' + e + '" is invalid for option "size"');
  }
  function f_(e, t, r) {
    return gl(e), e <= 0 ? Ye(e) : t !== undefined ? typeof r == "string" ? Ye(e).fill(t, r) : Ye(e).fill(t) : Ye(e);
  }
  p.alloc = function(e, t, r) {
    return f_(e, t, r);
  };
  function Ro(e) {
    return gl(e), Ye(e < 0 ? 0 : Io(e) | 0);
  }
  p.allocUnsafe = function(e) {
    return Ro(e);
  };
  p.allocUnsafeSlow = function(e) {
    return Ro(e);
  };
  function l_(e, t) {
    if ((typeof t != "string" || t === "") && (t = "utf8"), !p.isEncoding(t))
      throw new TypeError("Unknown encoding: " + t);
    let r = bl(e, t) | 0, n = Ye(r), i = n.write(e, t);
    return i !== r && (n = n.slice(0, i)), n;
  }
  function So(e) {
    let t = e.length < 0 ? 0 : Io(e.length) | 0, r = Ye(t);
    for (let n = 0;n < t; n += 1)
      r[n] = e[n] & 255;
    return r;
  }
  function u_(e) {
    if (Be(e, Uint8Array)) {
      let t = new Uint8Array(e);
      return Ao(t.buffer, t.byteOffset, t.byteLength);
    }
    return So(e);
  }
  function Ao(e, t, r) {
    if (t < 0 || e.byteLength < t)
      throw new RangeError('"offset" is outside of buffer bounds');
    if (e.byteLength < t + (r || 0))
      throw new RangeError('"length" is outside of buffer bounds');
    let n;
    return t === undefined && r === undefined ? n = new Uint8Array(e) : r === undefined ? n = new Uint8Array(e, t) : n = new Uint8Array(e, t, r), Object.setPrototypeOf(n, p.prototype), n;
  }
  function s_(e) {
    if (p.isBuffer(e)) {
      let t = Io(e.length) | 0, r = Ye(t);
      return r.length === 0 || e.copy(r, 0, 0, t), r;
    }
    if (e.length !== undefined)
      return typeof e.length != "number" || Oo(e.length) ? Ye(0) : So(e);
    if (e.type === "Buffer" && Array.isArray(e.data))
      return So(e.data);
  }
  function Io(e) {
    if (e >= Mn)
      throw new RangeError("Attempt to allocate Buffer larger than maximum size: 0x" + Mn.toString(16) + " bytes");
    return e | 0;
  }
  function c_(e) {
    return +e != e && (e = 0), p.alloc(+e);
  }
  p.isBuffer = function(t) {
    return t != null && t._isBuffer === true && t !== p.prototype;
  };
  p.compare = function(t, r) {
    if (Be(t, Uint8Array) && (t = p.from(t, t.offset, t.byteLength)), Be(r, Uint8Array) && (r = p.from(r, r.offset, r.byteLength)), !p.isBuffer(t) || !p.isBuffer(r))
      throw new TypeError('The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array');
    if (t === r)
      return 0;
    let n = t.length, i = r.length;
    for (let o = 0, a = Math.min(n, i);o < a; ++o)
      if (t[o] !== r[o]) {
        n = t[o], i = r[o];
        break;
      }
    return n < i ? -1 : i < n ? 1 : 0;
  };
  p.isEncoding = function(t) {
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
  p.concat = function(t, r) {
    if (!Array.isArray(t))
      throw new TypeError('"list" argument must be an Array of Buffers');
    if (t.length === 0)
      return p.alloc(0);
    let n;
    if (r === undefined)
      for (r = 0, n = 0;n < t.length; ++n)
        r += t[n].length;
    let i = p.allocUnsafe(r), o = 0;
    for (n = 0;n < t.length; ++n) {
      let a = t[n];
      if (Be(a, Uint8Array))
        o + a.length > i.length ? (p.isBuffer(a) || (a = p.from(a)), a.copy(i, o)) : Uint8Array.prototype.set.call(i, a, o);
      else if (p.isBuffer(a))
        a.copy(i, o);
      else
        throw new TypeError('"list" argument must be an Array of Buffers');
      o += a.length;
    }
    return i;
  };
  function bl(e, t) {
    if (p.isBuffer(e))
      return e.length;
    if (ArrayBuffer.isView(e) || Be(e, ArrayBuffer))
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
          return xo(e).length;
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return r * 2;
        case "hex":
          return r >>> 1;
        case "base64":
          return Il(e).length;
        default:
          if (i)
            return n ? -1 : xo(e).length;
          t = ("" + t).toLowerCase(), i = true;
      }
  }
  p.byteLength = bl;
  function d_(e, t, r) {
    let n = false;
    if ((t === undefined || t < 0) && (t = 0), t > this.length || ((r === undefined || r > this.length) && (r = this.length), r <= 0) || (r >>>= 0, t >>>= 0, r <= t))
      return "";
    for (e || (e = "utf8");; )
      switch (e) {
        case "hex":
          return m_(this, t, r);
        case "utf8":
        case "utf-8":
          return El(this, t, r);
        case "ascii":
          return E_(this, t, r);
        case "latin1":
        case "binary":
          return v_(this, t, r);
        case "base64":
          return b_(this, t, r);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return S_(this, t, r);
        default:
          if (n)
            throw new TypeError("Unknown encoding: " + e);
          e = (e + "").toLowerCase(), n = true;
      }
  }
  p.prototype._isBuffer = true;
  function St(e, t, r) {
    let n = e[t];
    e[t] = e[r], e[r] = n;
  }
  p.prototype.swap16 = function() {
    let t = this.length;
    if (t % 2 !== 0)
      throw new RangeError("Buffer size must be a multiple of 16-bits");
    for (let r = 0;r < t; r += 2)
      St(this, r, r + 1);
    return this;
  };
  p.prototype.swap32 = function() {
    let t = this.length;
    if (t % 4 !== 0)
      throw new RangeError("Buffer size must be a multiple of 32-bits");
    for (let r = 0;r < t; r += 4)
      St(this, r, r + 3), St(this, r + 1, r + 2);
    return this;
  };
  p.prototype.swap64 = function() {
    let t = this.length;
    if (t % 8 !== 0)
      throw new RangeError("Buffer size must be a multiple of 64-bits");
    for (let r = 0;r < t; r += 8)
      St(this, r, r + 7), St(this, r + 1, r + 6), St(this, r + 2, r + 5), St(this, r + 3, r + 4);
    return this;
  };
  p.prototype.toString = function() {
    let t = this.length;
    return t === 0 ? "" : arguments.length === 0 ? El(this, 0, t) : d_.apply(this, arguments);
  };
  p.prototype.toLocaleString = p.prototype.toString;
  p.prototype.equals = function(t) {
    if (!p.isBuffer(t))
      throw new TypeError("Argument must be a Buffer");
    return this === t ? true : p.compare(this, t) === 0;
  };
  p.prototype.inspect = function() {
    let t = "", r = or.INSPECT_MAX_BYTES;
    return t = this.toString("hex", 0, r).replace(/(.{2})/g, "$1 ").trim(), this.length > r && (t += " ... "), "<Buffer " + t + ">";
  };
  dl && (p.prototype[dl] = p.prototype.inspect);
  p.prototype.compare = function(t, r, n, i, o) {
    if (Be(t, Uint8Array) && (t = p.from(t, t.offset, t.byteLength)), !p.isBuffer(t))
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
    let a = o - i, f = n - r, u = Math.min(a, f), l = this.slice(i, o), s = t.slice(r, n);
    for (let c = 0;c < u; ++c)
      if (l[c] !== s[c]) {
        a = l[c], f = s[c];
        break;
      }
    return a < f ? -1 : f < a ? 1 : 0;
  };
  function wl(e, t, r, n, i) {
    if (e.length === 0)
      return -1;
    if (typeof r == "string" ? (n = r, r = 0) : r > 2147483647 ? r = 2147483647 : r < -2147483648 && (r = -2147483648), r = +r, Oo(r) && (r = i ? 0 : e.length - 1), r < 0 && (r = e.length + r), r >= e.length) {
      if (i)
        return -1;
      r = e.length - 1;
    } else if (r < 0)
      if (i)
        r = 0;
      else
        return -1;
    if (typeof t == "string" && (t = p.from(t, n)), p.isBuffer(t))
      return t.length === 0 ? -1 : hl(e, t, r, n, i);
    if (typeof t == "number")
      return t = t & 255, typeof Uint8Array.prototype.indexOf == "function" ? i ? Uint8Array.prototype.indexOf.call(e, t, r) : Uint8Array.prototype.lastIndexOf.call(e, t, r) : hl(e, [t], r, n, i);
    throw new TypeError("val must be string, number or Buffer");
  }
  function hl(e, t, r, n, i) {
    let o = 1, a = e.length, f = t.length;
    if (n !== undefined && (n = String(n).toLowerCase(), n === "ucs2" || n === "ucs-2" || n === "utf16le" || n === "utf-16le")) {
      if (e.length < 2 || t.length < 2)
        return -1;
      o = 2, a /= 2, f /= 2, r /= 2;
    }
    function u(s, c) {
      return o === 1 ? s[c] : s.readUInt16BE(c * o);
    }
    let l;
    if (i) {
      let s = -1;
      for (l = r;l < a; l++)
        if (u(e, l) === u(t, s === -1 ? 0 : l - s)) {
          if (s === -1 && (s = l), l - s + 1 === f)
            return s * o;
        } else
          s !== -1 && (l -= l - s), s = -1;
    } else
      for (r + f > a && (r = a - f), l = r;l >= 0; l--) {
        let s = true;
        for (let c = 0;c < f; c++)
          if (u(e, l + c) !== u(t, c)) {
            s = false;
            break;
          }
        if (s)
          return l;
      }
    return -1;
  }
  p.prototype.includes = function(t, r, n) {
    return this.indexOf(t, r, n) !== -1;
  };
  p.prototype.indexOf = function(t, r, n) {
    return wl(this, t, r, n, true);
  };
  p.prototype.lastIndexOf = function(t, r, n) {
    return wl(this, t, r, n, false);
  };
  function h_(e, t, r, n) {
    r = Number(r) || 0;
    let i = e.length - r;
    n ? (n = Number(n), n > i && (n = i)) : n = i;
    let o = t.length;
    n > o / 2 && (n = o / 2);
    let a;
    for (a = 0;a < n; ++a) {
      let f = parseInt(t.substr(a * 2, 2), 16);
      if (Oo(f))
        return a;
      e[r + a] = f;
    }
    return a;
  }
  function p_(e, t, r, n) {
    return jn(xo(t, e.length - r), e, r, n);
  }
  function y_(e, t, r, n) {
    return jn(I_(t), e, r, n);
  }
  function __(e, t, r, n) {
    return jn(Il(t), e, r, n);
  }
  function g_(e, t, r, n) {
    return jn(T_(t, e.length - r), e, r, n);
  }
  p.prototype.write = function(t, r, n, i) {
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
    let a = false;
    for (;; )
      switch (i) {
        case "hex":
          return h_(this, t, r, n);
        case "utf8":
        case "utf-8":
          return p_(this, t, r, n);
        case "ascii":
        case "latin1":
        case "binary":
          return y_(this, t, r, n);
        case "base64":
          return __(this, t, r, n);
        case "ucs2":
        case "ucs-2":
        case "utf16le":
        case "utf-16le":
          return g_(this, t, r, n);
        default:
          if (a)
            throw new TypeError("Unknown encoding: " + i);
          i = ("" + i).toLowerCase(), a = true;
      }
  };
  p.prototype.toJSON = function() {
    return { type: "Buffer", data: Array.prototype.slice.call(this._arr || this, 0) };
  };
  function b_(e, t, r) {
    return t === 0 && r === e.length ? mo.fromByteArray(e) : mo.fromByteArray(e.slice(t, r));
  }
  function El(e, t, r) {
    r = Math.min(e.length, r);
    let n = [], i = t;
    for (;i < r; ) {
      let o = e[i], a = null, f = o > 239 ? 4 : o > 223 ? 3 : o > 191 ? 2 : 1;
      if (i + f <= r) {
        let u, l, s, c;
        switch (f) {
          case 1:
            o < 128 && (a = o);
            break;
          case 2:
            u = e[i + 1], (u & 192) === 128 && (c = (o & 31) << 6 | u & 63, c > 127 && (a = c));
            break;
          case 3:
            u = e[i + 1], l = e[i + 2], (u & 192) === 128 && (l & 192) === 128 && (c = (o & 15) << 12 | (u & 63) << 6 | l & 63, c > 2047 && (c < 55296 || c > 57343) && (a = c));
            break;
          case 4:
            u = e[i + 1], l = e[i + 2], s = e[i + 3], (u & 192) === 128 && (l & 192) === 128 && (s & 192) === 128 && (c = (o & 15) << 18 | (u & 63) << 12 | (l & 63) << 6 | s & 63, c > 65535 && c < 1114112 && (a = c));
        }
      }
      a === null ? (a = 65533, f = 1) : a > 65535 && (a -= 65536, n.push(a >>> 10 & 1023 | 55296), a = 56320 | a & 1023), n.push(a), i += f;
    }
    return w_(n);
  }
  var pl = 4096;
  function w_(e) {
    let t = e.length;
    if (t <= pl)
      return String.fromCharCode.apply(String, e);
    let r = "", n = 0;
    for (;n < t; )
      r += String.fromCharCode.apply(String, e.slice(n, n += pl));
    return r;
  }
  function E_(e, t, r) {
    let n = "";
    r = Math.min(e.length, r);
    for (let i = t;i < r; ++i)
      n += String.fromCharCode(e[i] & 127);
    return n;
  }
  function v_(e, t, r) {
    let n = "";
    r = Math.min(e.length, r);
    for (let i = t;i < r; ++i)
      n += String.fromCharCode(e[i]);
    return n;
  }
  function m_(e, t, r) {
    let n = e.length;
    (!t || t < 0) && (t = 0), (!r || r < 0 || r > n) && (r = n);
    let i = "";
    for (let o = t;o < r; ++o)
      i += O_[e[o]];
    return i;
  }
  function S_(e, t, r) {
    let n = e.slice(t, r), i = "";
    for (let o = 0;o < n.length - 1; o += 2)
      i += String.fromCharCode(n[o] + n[o + 1] * 256);
    return i;
  }
  p.prototype.slice = function(t, r) {
    let n = this.length;
    t = ~~t, r = r === undefined ? n : ~~r, t < 0 ? (t += n, t < 0 && (t = 0)) : t > n && (t = n), r < 0 ? (r += n, r < 0 && (r = 0)) : r > n && (r = n), r < t && (r = t);
    let i = this.subarray(t, r);
    return Object.setPrototypeOf(i, p.prototype), i;
  };
  function J(e, t, r) {
    if (e % 1 !== 0 || e < 0)
      throw new RangeError("offset is not uint");
    if (e + t > r)
      throw new RangeError("Trying to access beyond buffer length");
  }
  p.prototype.readUintLE = p.prototype.readUIntLE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || J(t, r, this.length);
    let i = this[t], o = 1, a = 0;
    for (;++a < r && (o *= 256); )
      i += this[t + a] * o;
    return i;
  };
  p.prototype.readUintBE = p.prototype.readUIntBE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || J(t, r, this.length);
    let i = this[t + --r], o = 1;
    for (;r > 0 && (o *= 256); )
      i += this[t + --r] * o;
    return i;
  };
  p.prototype.readUint8 = p.prototype.readUInt8 = function(t, r) {
    return t = t >>> 0, r || J(t, 1, this.length), this[t];
  };
  p.prototype.readUint16LE = p.prototype.readUInt16LE = function(t, r) {
    return t = t >>> 0, r || J(t, 2, this.length), this[t] | this[t + 1] << 8;
  };
  p.prototype.readUint16BE = p.prototype.readUInt16BE = function(t, r) {
    return t = t >>> 0, r || J(t, 2, this.length), this[t] << 8 | this[t + 1];
  };
  p.prototype.readUint32LE = p.prototype.readUInt32LE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), (this[t] | this[t + 1] << 8 | this[t + 2] << 16) + this[t + 3] * 16777216;
  };
  p.prototype.readUint32BE = p.prototype.readUInt32BE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), this[t] * 16777216 + (this[t + 1] << 16 | this[t + 2] << 8 | this[t + 3]);
  };
  p.prototype.readBigUInt64LE = nt(function(t) {
    t = t >>> 0, ir(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && jr(t, this.length - 8);
    let i = r + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + this[++t] * 2 ** 24, o = this[++t] + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + n * 2 ** 24;
    return BigInt(i) + (BigInt(o) << BigInt(32));
  });
  p.prototype.readBigUInt64BE = nt(function(t) {
    t = t >>> 0, ir(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && jr(t, this.length - 8);
    let i = r * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + this[++t], o = this[++t] * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + n;
    return (BigInt(i) << BigInt(32)) + BigInt(o);
  });
  p.prototype.readIntLE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || J(t, r, this.length);
    let i = this[t], o = 1, a = 0;
    for (;++a < r && (o *= 256); )
      i += this[t + a] * o;
    return o *= 128, i >= o && (i -= Math.pow(2, 8 * r)), i;
  };
  p.prototype.readIntBE = function(t, r, n) {
    t = t >>> 0, r = r >>> 0, n || J(t, r, this.length);
    let i = r, o = 1, a = this[t + --i];
    for (;i > 0 && (o *= 256); )
      a += this[t + --i] * o;
    return o *= 128, a >= o && (a -= Math.pow(2, 8 * r)), a;
  };
  p.prototype.readInt8 = function(t, r) {
    return t = t >>> 0, r || J(t, 1, this.length), this[t] & 128 ? (255 - this[t] + 1) * -1 : this[t];
  };
  p.prototype.readInt16LE = function(t, r) {
    t = t >>> 0, r || J(t, 2, this.length);
    let n = this[t] | this[t + 1] << 8;
    return n & 32768 ? n | 4294901760 : n;
  };
  p.prototype.readInt16BE = function(t, r) {
    t = t >>> 0, r || J(t, 2, this.length);
    let n = this[t + 1] | this[t] << 8;
    return n & 32768 ? n | 4294901760 : n;
  };
  p.prototype.readInt32LE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), this[t] | this[t + 1] << 8 | this[t + 2] << 16 | this[t + 3] << 24;
  };
  p.prototype.readInt32BE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), this[t] << 24 | this[t + 1] << 16 | this[t + 2] << 8 | this[t + 3];
  };
  p.prototype.readBigInt64LE = nt(function(t) {
    t = t >>> 0, ir(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && jr(t, this.length - 8);
    let i = this[t + 4] + this[t + 5] * 2 ** 8 + this[t + 6] * 2 ** 16 + (n << 24);
    return (BigInt(i) << BigInt(32)) + BigInt(r + this[++t] * 2 ** 8 + this[++t] * 2 ** 16 + this[++t] * 2 ** 24);
  });
  p.prototype.readBigInt64BE = nt(function(t) {
    t = t >>> 0, ir(t, "offset");
    let r = this[t], n = this[t + 7];
    (r === undefined || n === undefined) && jr(t, this.length - 8);
    let i = (r << 24) + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + this[++t];
    return (BigInt(i) << BigInt(32)) + BigInt(this[++t] * 2 ** 24 + this[++t] * 2 ** 16 + this[++t] * 2 ** 8 + n);
  });
  p.prototype.readFloatLE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), nr.read(this, t, true, 23, 4);
  };
  p.prototype.readFloatBE = function(t, r) {
    return t = t >>> 0, r || J(t, 4, this.length), nr.read(this, t, false, 23, 4);
  };
  p.prototype.readDoubleLE = function(t, r) {
    return t = t >>> 0, r || J(t, 8, this.length), nr.read(this, t, true, 52, 8);
  };
  p.prototype.readDoubleBE = function(t, r) {
    return t = t >>> 0, r || J(t, 8, this.length), nr.read(this, t, false, 52, 8);
  };
  function fe(e, t, r, n, i, o) {
    if (!p.isBuffer(e))
      throw new TypeError('"buffer" argument must be a Buffer instance');
    if (t > i || t < o)
      throw new RangeError('"value" argument is out of bounds');
    if (r + n > e.length)
      throw new RangeError("Index out of range");
  }
  p.prototype.writeUintLE = p.prototype.writeUIntLE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, n = n >>> 0, !i) {
      let f = Math.pow(2, 8 * n) - 1;
      fe(this, t, r, n, f, 0);
    }
    let o = 1, a = 0;
    for (this[r] = t & 255;++a < n && (o *= 256); )
      this[r + a] = t / o & 255;
    return r + n;
  };
  p.prototype.writeUintBE = p.prototype.writeUIntBE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, n = n >>> 0, !i) {
      let f = Math.pow(2, 8 * n) - 1;
      fe(this, t, r, n, f, 0);
    }
    let o = n - 1, a = 1;
    for (this[r + o] = t & 255;--o >= 0 && (a *= 256); )
      this[r + o] = t / a & 255;
    return r + n;
  };
  p.prototype.writeUint8 = p.prototype.writeUInt8 = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 1, 255, 0), this[r] = t & 255, r + 1;
  };
  p.prototype.writeUint16LE = p.prototype.writeUInt16LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 2, 65535, 0), this[r] = t & 255, this[r + 1] = t >>> 8, r + 2;
  };
  p.prototype.writeUint16BE = p.prototype.writeUInt16BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 2, 65535, 0), this[r] = t >>> 8, this[r + 1] = t & 255, r + 2;
  };
  p.prototype.writeUint32LE = p.prototype.writeUInt32LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 4, 4294967295, 0), this[r + 3] = t >>> 24, this[r + 2] = t >>> 16, this[r + 1] = t >>> 8, this[r] = t & 255, r + 4;
  };
  p.prototype.writeUint32BE = p.prototype.writeUInt32BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 4, 4294967295, 0), this[r] = t >>> 24, this[r + 1] = t >>> 16, this[r + 2] = t >>> 8, this[r + 3] = t & 255, r + 4;
  };
  function vl(e, t, r, n, i) {
    Rl(t, n, i, e, r, 7);
    let o = Number(t & BigInt(4294967295));
    e[r++] = o, o = o >> 8, e[r++] = o, o = o >> 8, e[r++] = o, o = o >> 8, e[r++] = o;
    let a = Number(t >> BigInt(32) & BigInt(4294967295));
    return e[r++] = a, a = a >> 8, e[r++] = a, a = a >> 8, e[r++] = a, a = a >> 8, e[r++] = a, r;
  }
  function ml(e, t, r, n, i) {
    Rl(t, n, i, e, r, 7);
    let o = Number(t & BigInt(4294967295));
    e[r + 7] = o, o = o >> 8, e[r + 6] = o, o = o >> 8, e[r + 5] = o, o = o >> 8, e[r + 4] = o;
    let a = Number(t >> BigInt(32) & BigInt(4294967295));
    return e[r + 3] = a, a = a >> 8, e[r + 2] = a, a = a >> 8, e[r + 1] = a, a = a >> 8, e[r] = a, r + 8;
  }
  p.prototype.writeBigUInt64LE = nt(function(t, r = 0) {
    return vl(this, t, r, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  p.prototype.writeBigUInt64BE = nt(function(t, r = 0) {
    return ml(this, t, r, BigInt(0), BigInt("0xffffffffffffffff"));
  });
  p.prototype.writeIntLE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, !i) {
      let u = Math.pow(2, 8 * n - 1);
      fe(this, t, r, n, u - 1, -u);
    }
    let o = 0, a = 1, f = 0;
    for (this[r] = t & 255;++o < n && (a *= 256); )
      t < 0 && f === 0 && this[r + o - 1] !== 0 && (f = 1), this[r + o] = (t / a >> 0) - f & 255;
    return r + n;
  };
  p.prototype.writeIntBE = function(t, r, n, i) {
    if (t = +t, r = r >>> 0, !i) {
      let u = Math.pow(2, 8 * n - 1);
      fe(this, t, r, n, u - 1, -u);
    }
    let o = n - 1, a = 1, f = 0;
    for (this[r + o] = t & 255;--o >= 0 && (a *= 256); )
      t < 0 && f === 0 && this[r + o + 1] !== 0 && (f = 1), this[r + o] = (t / a >> 0) - f & 255;
    return r + n;
  };
  p.prototype.writeInt8 = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 1, 127, -128), t < 0 && (t = 255 + t + 1), this[r] = t & 255, r + 1;
  };
  p.prototype.writeInt16LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 2, 32767, -32768), this[r] = t & 255, this[r + 1] = t >>> 8, r + 2;
  };
  p.prototype.writeInt16BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 2, 32767, -32768), this[r] = t >>> 8, this[r + 1] = t & 255, r + 2;
  };
  p.prototype.writeInt32LE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 4, 2147483647, -2147483648), this[r] = t & 255, this[r + 1] = t >>> 8, this[r + 2] = t >>> 16, this[r + 3] = t >>> 24, r + 4;
  };
  p.prototype.writeInt32BE = function(t, r, n) {
    return t = +t, r = r >>> 0, n || fe(this, t, r, 4, 2147483647, -2147483648), t < 0 && (t = 4294967295 + t + 1), this[r] = t >>> 24, this[r + 1] = t >>> 16, this[r + 2] = t >>> 8, this[r + 3] = t & 255, r + 4;
  };
  p.prototype.writeBigInt64LE = nt(function(t, r = 0) {
    return vl(this, t, r, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  p.prototype.writeBigInt64BE = nt(function(t, r = 0) {
    return ml(this, t, r, -BigInt("0x8000000000000000"), BigInt("0x7fffffffffffffff"));
  });
  function Sl(e, t, r, n, i, o) {
    if (r + n > e.length)
      throw new RangeError("Index out of range");
    if (r < 0)
      throw new RangeError("Index out of range");
  }
  function Al(e, t, r, n, i) {
    return t = +t, r = r >>> 0, i || Sl(e, t, r, 4, 340282346638528860000000000000000000000, -340282346638528860000000000000000000000), nr.write(e, t, r, n, 23, 4), r + 4;
  }
  p.prototype.writeFloatLE = function(t, r, n) {
    return Al(this, t, r, true, n);
  };
  p.prototype.writeFloatBE = function(t, r, n) {
    return Al(this, t, r, false, n);
  };
  function xl(e, t, r, n, i) {
    return t = +t, r = r >>> 0, i || Sl(e, t, r, 8, 179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000, -179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000), nr.write(e, t, r, n, 52, 8), r + 8;
  }
  p.prototype.writeDoubleLE = function(t, r, n) {
    return xl(this, t, r, true, n);
  };
  p.prototype.writeDoubleBE = function(t, r, n) {
    return xl(this, t, r, false, n);
  };
  p.prototype.copy = function(t, r, n, i) {
    if (!p.isBuffer(t))
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
  p.prototype.fill = function(t, r, n, i) {
    if (typeof t == "string") {
      if (typeof r == "string" ? (i = r, r = 0, n = this.length) : typeof n == "string" && (i = n, n = this.length), i !== undefined && typeof i != "string")
        throw new TypeError("encoding must be a string");
      if (typeof i == "string" && !p.isEncoding(i))
        throw new TypeError("Unknown encoding: " + i);
      if (t.length === 1) {
        let a = t.charCodeAt(0);
        (i === "utf8" && a < 128 || i === "latin1") && (t = a);
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
      let a = p.isBuffer(t) ? t : p.from(t, i), f = a.length;
      if (f === 0)
        throw new TypeError('The value "' + t + '" is invalid for argument "value"');
      for (o = 0;o < n - r; ++o)
        this[o + r] = a[o % f];
    }
    return this;
  };
  var rr = {};
  function To(e, t, r) {
    rr[e] = class extends r {
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
  To("ERR_BUFFER_OUT_OF_BOUNDS", function(e) {
    return e ? `${e} is outside of buffer bounds` : "Attempt to access memory outside buffer bounds";
  }, RangeError);
  To("ERR_INVALID_ARG_TYPE", function(e, t) {
    return `The "${e}" argument must be of type number. Received type ${typeof t}`;
  }, TypeError);
  To("ERR_OUT_OF_RANGE", function(e, t, r) {
    let n = `The value of "${e}" is out of range.`, i = r;
    return Number.isInteger(r) && Math.abs(r) > 2 ** 32 ? i = yl(String(r)) : typeof r == "bigint" && (i = String(r), (r > BigInt(2) ** BigInt(32) || r < -(BigInt(2) ** BigInt(32))) && (i = yl(i)), i += "n"), n += ` It must be ${t}. Received ${i}`, n;
  }, RangeError);
  function yl(e) {
    let t = "", r = e.length, n = e[0] === "-" ? 1 : 0;
    for (;r >= n + 4; r -= 3)
      t = `_${e.slice(r - 3, r)}${t}`;
    return `${e.slice(0, r)}${t}`;
  }
  function A_(e, t, r) {
    ir(t, "offset"), (e[t] === undefined || e[t + r] === undefined) && jr(t, e.length - (r + 1));
  }
  function Rl(e, t, r, n, i, o) {
    if (e > r || e < t) {
      let a = typeof t == "bigint" ? "n" : "", f;
      throw o > 3 ? t === 0 || t === BigInt(0) ? f = `>= 0${a} and < 2${a} ** ${(o + 1) * 8}${a}` : f = `>= -(2${a} ** ${(o + 1) * 8 - 1}${a}) and < 2 ** ${(o + 1) * 8 - 1}${a}` : f = `>= ${t}${a} and <= ${r}${a}`, new rr.ERR_OUT_OF_RANGE("value", f, e);
    }
    A_(n, i, o);
  }
  function ir(e, t) {
    if (typeof e != "number")
      throw new rr.ERR_INVALID_ARG_TYPE(t, "number", e);
  }
  function jr(e, t, r) {
    throw Math.floor(e) !== e ? (ir(e, r), new rr.ERR_OUT_OF_RANGE(r || "offset", "an integer", e)) : t < 0 ? new rr.ERR_BUFFER_OUT_OF_BOUNDS : new rr.ERR_OUT_OF_RANGE(r || "offset", `>= ${r ? 1 : 0} and <= ${t}`, e);
  }
  var x_ = /[^+/0-9A-Za-z-_]/g;
  function R_(e) {
    if (e = e.split("=")[0], e = e.trim().replace(x_, ""), e.length < 2)
      return "";
    for (;e.length % 4 !== 0; )
      e = e + "=";
    return e;
  }
  function xo(e, t) {
    t = t || 1 / 0;
    let r, n = e.length, i = null, o = [];
    for (let a = 0;a < n; ++a) {
      if (r = e.charCodeAt(a), r > 55295 && r < 57344) {
        if (!i) {
          if (r > 56319) {
            (t -= 3) > -1 && o.push(239, 191, 189);
            continue;
          } else if (a + 1 === n) {
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
  function I_(e) {
    let t = [];
    for (let r = 0;r < e.length; ++r)
      t.push(e.charCodeAt(r) & 255);
    return t;
  }
  function T_(e, t) {
    let r, n, i, o = [];
    for (let a = 0;a < e.length && !((t -= 2) < 0); ++a)
      r = e.charCodeAt(a), n = r >> 8, i = r % 256, o.push(i), o.push(n);
    return o;
  }
  function Il(e) {
    return mo.toByteArray(R_(e));
  }
  function jn(e, t, r, n) {
    let i;
    for (i = 0;i < n && !(i + r >= t.length || i >= e.length); ++i)
      t[i + r] = e[i];
    return i;
  }
  function Be(e, t) {
    return e instanceof t || e != null && e.constructor != null && e.constructor.name != null && e.constructor.name === t.name;
  }
  function Oo(e) {
    return e !== e;
  }
  var O_ = function() {
    let e = "0123456789abcdef", t = new Array(256);
    for (let r = 0;r < 16; ++r) {
      let n = r * 16;
      for (let i = 0;i < 16; ++i)
        t[n + i] = e[r] + e[i];
    }
    return t;
  }();
  function nt(e) {
    return typeof BigInt > "u" ? N_ : e;
  }
  function N_() {
    throw new Error("BigInt not supported");
  }
});
var V = g((RA, Tl) => {
  Tl.exports = { ArrayIsArray(e) {
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
var Pe = g((IA, ko) => {
  var k_ = xe(), F_ = Object.getPrototypeOf(async function() {
  }).constructor, Ol = globalThis.Blob || k_.Blob, L_ = typeof Ol < "u" ? function(t) {
    return t instanceof Ol;
  } : function(t) {
    return false;
  }, No = class extends Error {
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
  ko.exports = { AggregateError: No, kEmptyObject: Object.freeze({}), once(e) {
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
    return e instanceof F_;
  }, isArrayBufferView(e) {
    return ArrayBuffer.isView(e);
  } }, isBlob: L_ };
  ko.exports.promisify.custom = Symbol.for("nodejs.util.promisify.custom");
});
var qn = g((TA, Un) => {
  var { AbortController: Nl, AbortSignal: D_ } = typeof self < "u" ? self : typeof window < "u" ? window : undefined;
  Un.exports = Nl;
  Un.exports.AbortSignal = D_;
  Un.exports.default = Nl;
});
var ne = g((OA, Ll) => {
  var { format: B_, inspect: Cn, AggregateError: P_ } = Pe(), M_ = globalThis.AggregateError || P_, j_ = Symbol("kIsNodeError"), U_ = ["string", "function", "number", "object", "Function", "Object", "boolean", "bigint", "symbol"], q_ = /^([A-Z][a-z0-9]*)+$/, C_ = "__node_internal_", zn = {};
  function At(e, t) {
    if (!e)
      throw new zn.ERR_INTERNAL_ASSERTION(t);
  }
  function kl(e) {
    let t = "", r = e.length, n = e[0] === "-" ? 1 : 0;
    for (;r >= n + 4; r -= 3)
      t = `_${e.slice(r - 3, r)}${t}`;
    return `${e.slice(0, r)}${t}`;
  }
  function z_(e, t, r) {
    if (typeof t == "function")
      return At(t.length <= r.length, `Code: ${e}; The provided arguments length (${r.length}) does not match the required ones (${t.length}).`), t(...r);
    let n = (t.match(/%[dfijoOs]/g) || []).length;
    return At(n === r.length, `Code: ${e}; The provided arguments length (${r.length}) does not match the required ones (${n}).`), r.length === 0 ? t : B_(t, ...r);
  }
  function Q(e, t, r) {
    r || (r = Error);

    class n extends r {
      constructor(...o) {
        super(z_(e, t, o));
      }
      toString() {
        return `${this.name} [${e}]: ${this.message}`;
      }
    }
    Object.defineProperties(n.prototype, { name: { value: r.name, writable: true, enumerable: false, configurable: true }, toString: { value() {
      return `${this.name} [${e}]: ${this.message}`;
    }, writable: true, enumerable: false, configurable: true } }), n.prototype.code = e, n.prototype[j_] = true, zn[e] = n;
  }
  function Fl(e) {
    let t = C_ + e.name;
    return Object.defineProperty(e, "name", { value: t }), e;
  }
  function W_(e, t) {
    if (e && t && e !== t) {
      if (Array.isArray(t.errors))
        return t.errors.push(e), t;
      let r = new M_([t, e], t.message);
      return r.code = t.code, r;
    }
    return e || t;
  }
  var Fo = class extends Error {
    constructor(t = "The operation was aborted", r = undefined) {
      if (r !== undefined && typeof r != "object")
        throw new zn.ERR_INVALID_ARG_TYPE("options", "Object", r);
      super(t, r), this.code = "ABORT_ERR", this.name = "AbortError";
    }
  };
  Q("ERR_ASSERTION", "%s", Error);
  Q("ERR_INVALID_ARG_TYPE", (e, t, r) => {
    At(typeof e == "string", "'name' must be a string"), Array.isArray(t) || (t = [t]);
    let n = "The ";
    e.endsWith(" argument") ? n += `${e} ` : n += `"${e}" ${e.includes(".") ? "property" : "argument"} `, n += "must be ";
    let i = [], o = [], a = [];
    for (let u of t)
      At(typeof u == "string", "All expected entries have to be of type string"), U_.includes(u) ? i.push(u.toLowerCase()) : q_.test(u) ? o.push(u) : (At(u !== "object", 'The value "object" should be written as "Object"'), a.push(u));
    if (o.length > 0) {
      let u = i.indexOf("object");
      u !== -1 && (i.splice(i, u, 1), o.push("Object"));
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
          let u = i.pop();
          n += `one of type ${i.join(", ")}, or ${u}`;
        }
      }
      (o.length > 0 || a.length > 0) && (n += " or ");
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
          let u = o.pop();
          n += `an instance of ${o.join(", ")}, or ${u}`;
        }
      }
      a.length > 0 && (n += " or ");
    }
    switch (a.length) {
      case 0:
        break;
      case 1:
        a[0].toLowerCase() !== a[0] && (n += "an "), n += `${a[0]}`;
        break;
      case 2:
        n += `one of ${a[0]} or ${a[1]}`;
        break;
      default: {
        let u = a.pop();
        n += `one of ${a.join(", ")}, or ${u}`;
      }
    }
    if (r == null)
      n += `. Received ${r}`;
    else if (typeof r == "function" && r.name)
      n += `. Received function ${r.name}`;
    else if (typeof r == "object") {
      var f;
      (f = r.constructor) !== null && f !== undefined && f.name ? n += `. Received an instance of ${r.constructor.name}` : n += `. Received ${Cn(r, { depth: -1 })}`;
    } else {
      let u = Cn(r, { colors: false });
      u.length > 25 && (u = `${u.slice(0, 25)}...`), n += `. Received type ${typeof r} (${u})`;
    }
    return n;
  }, TypeError);
  Q("ERR_INVALID_ARG_VALUE", (e, t, r = "is invalid") => {
    let n = Cn(t);
    return n.length > 128 && (n = n.slice(0, 128) + "..."), `The ${e.includes(".") ? "property" : "argument"} '${e}' ${r}. Received ${n}`;
  }, TypeError);
  Q("ERR_INVALID_RETURN_VALUE", (e, t, r) => {
    var n;
    let i = r != null && (n = r.constructor) !== null && n !== undefined && n.name ? `instance of ${r.constructor.name}` : `type ${typeof r}`;
    return `Expected ${e} to be returned from the "${t}" function but got ${i}.`;
  }, TypeError);
  Q("ERR_MISSING_ARGS", (...e) => {
    At(e.length > 0, "At least one arg needs to be specified");
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
  Q("ERR_OUT_OF_RANGE", (e, t, r) => {
    At(t, 'Missing "range" argument');
    let n;
    return Number.isInteger(r) && Math.abs(r) > 2 ** 32 ? n = kl(String(r)) : typeof r == "bigint" ? (n = String(r), (r > 2n ** 32n || r < -(2n ** 32n)) && (n = kl(n)), n += "n") : n = Cn(r), `The value of "${e}" is out of range. It must be ${t}. Received ${n}`;
  }, RangeError);
  Q("ERR_MULTIPLE_CALLBACK", "Callback called multiple times", Error);
  Q("ERR_METHOD_NOT_IMPLEMENTED", "The %s method is not implemented", Error);
  Q("ERR_STREAM_ALREADY_FINISHED", "Cannot call %s after a stream was finished", Error);
  Q("ERR_STREAM_CANNOT_PIPE", "Cannot pipe, not readable", Error);
  Q("ERR_STREAM_DESTROYED", "Cannot call %s after a stream was destroyed", Error);
  Q("ERR_STREAM_NULL_VALUES", "May not write null values to stream", TypeError);
  Q("ERR_STREAM_PREMATURE_CLOSE", "Premature close", Error);
  Q("ERR_STREAM_PUSH_AFTER_EOF", "stream.push() after EOF", Error);
  Q("ERR_STREAM_UNSHIFT_AFTER_END_EVENT", "stream.unshift() after end event", Error);
  Q("ERR_STREAM_WRITE_AFTER_END", "write after end", Error);
  Q("ERR_UNKNOWN_ENCODING", "Unknown encoding: %s", TypeError);
  Ll.exports = { AbortError: Fo, aggregateTwoErrors: Fl(W_), hideStackFrames: Fl, codes: zn };
});
var Ur = g((NA, Cl) => {
  var { ArrayIsArray: Pl, ArrayPrototypeIncludes: Ml, ArrayPrototypeJoin: jl, ArrayPrototypeMap: Z_, NumberIsInteger: Do, NumberIsNaN: $_, NumberMAX_SAFE_INTEGER: G_, NumberMIN_SAFE_INTEGER: H_, NumberParseInt: V_, ObjectPrototypeHasOwnProperty: Y_, RegExpPrototypeExec: K_, String: X_, StringPrototypeToUpperCase: J_, StringPrototypeTrim: Q_ } = V(), { hideStackFrames: Re, codes: { ERR_SOCKET_BAD_PORT: e0, ERR_INVALID_ARG_TYPE: le, ERR_INVALID_ARG_VALUE: Wn, ERR_OUT_OF_RANGE: xt, ERR_UNKNOWN_SIGNAL: Dl } } = ne(), { normalizeEncoding: t0 } = Pe(), { isAsyncFunction: r0, isArrayBufferView: n0 } = Pe().types, Bl = {};
  function i0(e) {
    return e === (e | 0);
  }
  function o0(e) {
    return e === e >>> 0;
  }
  var a0 = /^[0-7]+$/, f0 = "must be a 32-bit unsigned integer or an octal string";
  function l0(e, t, r) {
    if (typeof e > "u" && (e = r), typeof e == "string") {
      if (K_(a0, e) === null)
        throw new Wn(t, e, f0);
      e = V_(e, 8);
    }
    return Ul(e, t), e;
  }
  var u0 = Re((e, t, r = H_, n = G_) => {
    if (typeof e != "number")
      throw new le(t, "number", e);
    if (!Do(e))
      throw new xt(t, "an integer", e);
    if (e < r || e > n)
      throw new xt(t, `>= ${r} && <= ${n}`, e);
  }), s0 = Re((e, t, r = -2147483648, n = 2147483647) => {
    if (typeof e != "number")
      throw new le(t, "number", e);
    if (!Do(e))
      throw new xt(t, "an integer", e);
    if (e < r || e > n)
      throw new xt(t, `>= ${r} && <= ${n}`, e);
  }), Ul = Re((e, t, r = false) => {
    if (typeof e != "number")
      throw new le(t, "number", e);
    if (!Do(e))
      throw new xt(t, "an integer", e);
    let n = r ? 1 : 0, i = 4294967295;
    if (e < n || e > i)
      throw new xt(t, `>= ${n} && <= ${i}`, e);
  });
  function ql(e, t) {
    if (typeof e != "string")
      throw new le(t, "string", e);
  }
  function c0(e, t, r = undefined, n) {
    if (typeof e != "number")
      throw new le(t, "number", e);
    if (r != null && e < r || n != null && e > n || (r != null || n != null) && $_(e))
      throw new xt(t, `${r != null ? `>= ${r}` : ""}${r != null && n != null ? " && " : ""}${n != null ? `<= ${n}` : ""}`, e);
  }
  var d0 = Re((e, t, r) => {
    if (!Ml(r, e)) {
      let n = jl(Z_(r, (o) => typeof o == "string" ? `'${o}'` : X_(o)), ", "), i = "must be one of: " + n;
      throw new Wn(t, e, i);
    }
  });
  function h0(e, t) {
    if (typeof e != "boolean")
      throw new le(t, "boolean", e);
  }
  function Lo(e, t, r) {
    return e == null || !Y_(e, t) ? r : e[t];
  }
  var p0 = Re((e, t, r = null) => {
    let n = Lo(r, "allowArray", false), i = Lo(r, "allowFunction", false);
    if (!Lo(r, "nullable", false) && e === null || !n && Pl(e) || typeof e != "object" && (!i || typeof e != "function"))
      throw new le(t, "Object", e);
  }), y0 = Re((e, t, r = 0) => {
    if (!Pl(e))
      throw new le(t, "Array", e);
    if (e.length < r) {
      let n = `must be longer than ${r}`;
      throw new Wn(t, e, n);
    }
  });
  function _0(e, t = "signal") {
    if (ql(e, t), Bl[e] === undefined)
      throw Bl[J_(e)] !== undefined ? new Dl(e + " (signals must use all capital letters)") : new Dl(e);
  }
  var g0 = Re((e, t = "buffer") => {
    if (!n0(e))
      throw new le(t, ["Buffer", "TypedArray", "DataView"], e);
  });
  function b0(e, t) {
    let r = t0(t), n = e.length;
    if (r === "hex" && n % 2 !== 0)
      throw new Wn("encoding", t, `is invalid for data of length ${n}`);
  }
  function w0(e, t = "Port", r = true) {
    if (typeof e != "number" && typeof e != "string" || typeof e == "string" && Q_(e).length === 0 || +e !== +e >>> 0 || e > 65535 || e === 0 && !r)
      throw new e0(t, e, r);
    return e | 0;
  }
  var E0 = Re((e, t) => {
    if (e !== undefined && (e === null || typeof e != "object" || !("aborted" in e)))
      throw new le(t, "AbortSignal", e);
  }), v0 = Re((e, t) => {
    if (typeof e != "function")
      throw new le(t, "Function", e);
  }), m0 = Re((e, t) => {
    if (typeof e != "function" || r0(e))
      throw new le(t, "Function", e);
  }), S0 = Re((e, t) => {
    if (e !== undefined)
      throw new le(t, "undefined", e);
  });
  function A0(e, t, r) {
    if (!Ml(r, e))
      throw new le(t, `('${jl(r, "|")}')`, e);
  }
  Cl.exports = { isInt32: i0, isUint32: o0, parseFileMode: l0, validateArray: y0, validateBoolean: h0, validateBuffer: g0, validateEncoding: b0, validateFunction: v0, validateInt32: s0, validateInteger: u0, validateNumber: c0, validateObject: p0, validateOneOf: d0, validatePlainFunction: m0, validatePort: w0, validateSignalName: _0, validateString: ql, validateUint32: Ul, validateUndefined: S0, validateUnion: A0, validateAbortSignal: E0 };
});
var Mo = g((kA, $l) => {
  var $ = $l.exports = {}, Me, je;
  function Bo() {
    throw new Error("setTimeout has not been defined");
  }
  function Po() {
    throw new Error("clearTimeout has not been defined");
  }
  (function() {
    try {
      typeof setTimeout == "function" ? Me = setTimeout : Me = Bo;
    } catch {
      Me = Bo;
    }
    try {
      typeof clearTimeout == "function" ? je = clearTimeout : je = Po;
    } catch {
      je = Po;
    }
  })();
  function zl(e) {
    if (Me === setTimeout)
      return setTimeout(e, 0);
    if ((Me === Bo || !Me) && setTimeout)
      return Me = setTimeout, setTimeout(e, 0);
    try {
      return Me(e, 0);
    } catch {
      try {
        return Me.call(null, e, 0);
      } catch {
        return Me.call(this, e, 0);
      }
    }
  }
  function x0(e) {
    if (je === clearTimeout)
      return clearTimeout(e);
    if ((je === Po || !je) && clearTimeout)
      return je = clearTimeout, clearTimeout(e);
    try {
      return je(e);
    } catch {
      try {
        return je.call(null, e);
      } catch {
        return je.call(this, e);
      }
    }
  }
  var Ke = [], ar = false, Rt, Zn = -1;
  function R0() {
    !ar || !Rt || (ar = false, Rt.length ? Ke = Rt.concat(Ke) : Zn = -1, Ke.length && Wl());
  }
  function Wl() {
    if (!ar) {
      var e = zl(R0);
      ar = true;
      for (var t = Ke.length;t; ) {
        for (Rt = Ke, Ke = [];++Zn < t; )
          Rt && Rt[Zn].run();
        Zn = -1, t = Ke.length;
      }
      Rt = null, ar = false, x0(e);
    }
  }
  $.nextTick = function(e) {
    var t = new Array(arguments.length - 1);
    if (arguments.length > 1)
      for (var r = 1;r < arguments.length; r++)
        t[r - 1] = arguments[r];
    Ke.push(new Zl(e, t)), Ke.length === 1 && !ar && zl(Wl);
  };
  function Zl(e, t) {
    this.fun = e, this.array = t;
  }
  Zl.prototype.run = function() {
    this.fun.apply(null, this.array);
  };
  $.title = "browser";
  $.browser = true;
  $.env = {};
  $.argv = [];
  $.version = "";
  $.versions = {};
  function Xe() {
  }
  $.on = Xe;
  $.addListener = Xe;
  $.once = Xe;
  $.off = Xe;
  $.removeListener = Xe;
  $.removeAllListeners = Xe;
  $.emit = Xe;
  $.prependListener = Xe;
  $.prependOnceListener = Xe;
  $.listeners = function(e) {
    return [];
  };
  $.binding = function(e) {
    throw new Error("process.binding is not supported");
  };
  $.cwd = function() {
    return "/";
  };
  $.chdir = function(e) {
    throw new Error("process.chdir is not supported");
  };
  $.umask = function() {
    return 0;
  };
});
var ye = {};
Bn(ye, { default: () => I0 });
var I0;
var it = wo(() => {
  X(ye, vt(Mo()));
  I0 = vt(Mo());
});
var ot = g((LA, nu) => {
  var { Symbol: $n, SymbolAsyncIterator: Gl, SymbolIterator: Hl } = V(), Vl = $n("kDestroyed"), Yl = $n("kIsErrored"), jo = $n("kIsReadable"), Kl = $n("kIsDisturbed");
  function Gn(e, t = false) {
    var r;
    return !!(e && typeof e.pipe == "function" && typeof e.on == "function" && (!t || typeof e.pause == "function" && typeof e.resume == "function") && (!e._writableState || ((r = e._readableState) === null || r === undefined ? undefined : r.readable) !== false) && (!e._writableState || e._readableState));
  }
  function Hn(e) {
    var t;
    return !!(e && typeof e.write == "function" && typeof e.on == "function" && (!e._readableState || ((t = e._writableState) === null || t === undefined ? undefined : t.writable) !== false));
  }
  function T0(e) {
    return !!(e && typeof e.pipe == "function" && e._readableState && typeof e.on == "function" && typeof e.write == "function");
  }
  function It(e) {
    return e && (e._readableState || e._writableState || typeof e.write == "function" && typeof e.on == "function" || typeof e.pipe == "function" && typeof e.on == "function");
  }
  function O0(e, t) {
    return e == null ? false : t === true ? typeof e[Gl] == "function" : t === false ? typeof e[Hl] == "function" : typeof e[Gl] == "function" || typeof e[Hl] == "function";
  }
  function Vn(e) {
    if (!It(e))
      return null;
    let { _writableState: t, _readableState: r } = e, n = t || r;
    return !!(e.destroyed || e[Vl] || n != null && n.destroyed);
  }
  function Xl(e) {
    if (!Hn(e))
      return null;
    if (e.writableEnded === true)
      return true;
    let t = e._writableState;
    return t != null && t.errored ? false : typeof t?.ended != "boolean" ? null : t.ended;
  }
  function N0(e, t) {
    if (!Hn(e))
      return null;
    if (e.writableFinished === true)
      return true;
    let r = e._writableState;
    return r != null && r.errored ? false : typeof r?.finished != "boolean" ? null : !!(r.finished || t === false && r.ended === true && r.length === 0);
  }
  function k0(e) {
    if (!Gn(e))
      return null;
    if (e.readableEnded === true)
      return true;
    let t = e._readableState;
    return !t || t.errored ? false : typeof t?.ended != "boolean" ? null : t.ended;
  }
  function Jl(e, t) {
    if (!Gn(e))
      return null;
    let r = e._readableState;
    return r != null && r.errored ? false : typeof r?.endEmitted != "boolean" ? null : !!(r.endEmitted || t === false && r.ended === true && r.length === 0);
  }
  function Ql(e) {
    return e && e[jo] != null ? e[jo] : typeof e?.readable != "boolean" ? null : Vn(e) ? false : Gn(e) && e.readable && !Jl(e);
  }
  function eu(e) {
    return typeof e?.writable != "boolean" ? null : Vn(e) ? false : Hn(e) && e.writable && !Xl(e);
  }
  function F0(e, t) {
    return It(e) ? Vn(e) ? true : !(t?.readable !== false && Ql(e) || t?.writable !== false && eu(e)) : null;
  }
  function L0(e) {
    var t, r;
    return It(e) ? e.writableErrored ? e.writableErrored : (t = (r = e._writableState) === null || r === undefined ? undefined : r.errored) !== null && t !== undefined ? t : null : null;
  }
  function D0(e) {
    var t, r;
    return It(e) ? e.readableErrored ? e.readableErrored : (t = (r = e._readableState) === null || r === undefined ? undefined : r.errored) !== null && t !== undefined ? t : null : null;
  }
  function B0(e) {
    if (!It(e))
      return null;
    if (typeof e.closed == "boolean")
      return e.closed;
    let { _writableState: t, _readableState: r } = e;
    return typeof t?.closed == "boolean" || typeof r?.closed == "boolean" ? t?.closed || r?.closed : typeof e._closed == "boolean" && tu(e) ? e._closed : null;
  }
  function tu(e) {
    return typeof e._closed == "boolean" && typeof e._defaultKeepAlive == "boolean" && typeof e._removedConnection == "boolean" && typeof e._removedContLen == "boolean";
  }
  function ru(e) {
    return typeof e._sent100 == "boolean" && tu(e);
  }
  function P0(e) {
    var t;
    return typeof e._consuming == "boolean" && typeof e._dumped == "boolean" && ((t = e.req) === null || t === undefined ? undefined : t.upgradeOrConnect) === undefined;
  }
  function M0(e) {
    if (!It(e))
      return null;
    let { _writableState: t, _readableState: r } = e, n = t || r;
    return !n && ru(e) || !!(n && n.autoDestroy && n.emitClose && n.closed === false);
  }
  function j0(e) {
    var t;
    return !!(e && ((t = e[Kl]) !== null && t !== undefined ? t : e.readableDidRead || e.readableAborted));
  }
  function U0(e) {
    var t, r, n, i, o, a, f, u, l, s;
    return !!(e && ((t = (r = (n = (i = (o = (a = e[Yl]) !== null && a !== undefined ? a : e.readableErrored) !== null && o !== undefined ? o : e.writableErrored) !== null && i !== undefined ? i : (f = e._readableState) === null || f === undefined ? undefined : f.errorEmitted) !== null && n !== undefined ? n : (u = e._writableState) === null || u === undefined ? undefined : u.errorEmitted) !== null && r !== undefined ? r : (l = e._readableState) === null || l === undefined ? undefined : l.errored) !== null && t !== undefined ? t : (s = e._writableState) === null || s === undefined ? undefined : s.errored));
  }
  nu.exports = { kDestroyed: Vl, isDisturbed: j0, kIsDisturbed: Kl, isErrored: U0, kIsErrored: Yl, isReadable: Ql, kIsReadable: jo, isClosed: B0, isDestroyed: Vn, isDuplexNodeStream: T0, isFinished: F0, isIterable: O0, isReadableNodeStream: Gn, isReadableEnded: k0, isReadableFinished: Jl, isReadableErrored: D0, isNodeStream: It, isWritable: eu, isWritableNodeStream: Hn, isWritableEnded: Xl, isWritableFinished: N0, isWritableErrored: L0, isServerRequest: P0, isServerResponse: ru, willEmitClose: M0 };
});
var at = g((DA, qo) => {
  var fr = (it(), se(ye)), { AbortError: q0, codes: C0 } = ne(), { ERR_INVALID_ARG_TYPE: z0, ERR_STREAM_PREMATURE_CLOSE: iu } = C0, { kEmptyObject: ou, once: au } = Pe(), { validateAbortSignal: W0, validateFunction: Z0, validateObject: $0 } = Ur(), { Promise: G0 } = V(), { isClosed: H0, isReadable: fu, isReadableNodeStream: Uo, isReadableFinished: lu, isReadableErrored: V0, isWritable: uu, isWritableNodeStream: su, isWritableFinished: cu, isWritableErrored: Y0, isNodeStream: K0, willEmitClose: X0 } = ot();
  function J0(e) {
    return e.setHeader && typeof e.abort == "function";
  }
  var Q0 = () => {
  };
  function du(e, t, r) {
    var n, i;
    arguments.length === 2 ? (r = t, t = ou) : t == null ? t = ou : $0(t, "options"), Z0(r, "callback"), W0(t.signal, "options.signal"), r = au(r);
    let o = (n = t.readable) !== null && n !== undefined ? n : Uo(e), a = (i = t.writable) !== null && i !== undefined ? i : su(e);
    if (!K0(e))
      throw new z0("stream", "Stream", e);
    let { _writableState: f, _readableState: u } = e, l = () => {
      e.writable || h();
    }, s = X0(e) && Uo(e) === o && su(e) === a, c = cu(e, false), h = () => {
      c = true, e.destroyed && (s = false), !(s && (!e.readable || o)) && (!o || d) && r.call(e);
    }, d = lu(e, false), y = () => {
      d = true, e.destroyed && (s = false), !(s && (!e.writable || a)) && (!a || c) && r.call(e);
    }, b = (A) => {
      r.call(e, A);
    }, R = H0(e), _ = () => {
      R = true;
      let A = Y0(e) || V0(e);
      if (A && typeof A != "boolean")
        return r.call(e, A);
      if (o && !d && Uo(e, true) && !lu(e, false))
        return r.call(e, new iu);
      if (a && !c && !cu(e, false))
        return r.call(e, new iu);
      r.call(e);
    }, E = () => {
      e.req.on("finish", h);
    };
    J0(e) ? (e.on("complete", h), s || e.on("abort", _), e.req ? E() : e.on("request", E)) : a && !f && (e.on("end", l), e.on("close", l)), !s && typeof e.aborted == "boolean" && e.on("aborted", _), e.on("end", y), e.on("finish", h), t.error !== false && e.on("error", b), e.on("close", _), R ? fr.nextTick(_) : f != null && f.errorEmitted || u != null && u.errorEmitted ? s || fr.nextTick(_) : (!o && (!s || fu(e)) && (c || uu(e) === false) || !a && (!s || uu(e)) && (d || fu(e) === false) || u && e.req && e.aborted) && fr.nextTick(_);
    let m = () => {
      r = Q0, e.removeListener("aborted", _), e.removeListener("complete", h), e.removeListener("abort", _), e.removeListener("request", E), e.req && e.req.removeListener("finish", h), e.removeListener("end", l), e.removeListener("close", l), e.removeListener("finish", h), e.removeListener("end", y), e.removeListener("error", b), e.removeListener("close", _);
    };
    if (t.signal && !R) {
      let A = () => {
        let v = r;
        m(), v.call(e, new q0(undefined, { cause: t.signal.reason }));
      };
      if (t.signal.aborted)
        fr.nextTick(A);
      else {
        let v = r;
        r = au((...T) => {
          t.signal.removeEventListener("abort", A), v.apply(e, T);
        }), t.signal.addEventListener("abort", A);
      }
    }
    return m;
  }
  function eg(e, t) {
    return new G0((r, n) => {
      du(e, t, (i) => {
        i ? n(i) : r();
      });
    });
  }
  qo.exports = du;
  qo.exports.finished = eg;
});
var Eu = g((BA, Wo) => {
  var _u = globalThis.AbortController || qn().AbortController, { codes: { ERR_INVALID_ARG_TYPE: qr, ERR_MISSING_ARGS: tg, ERR_OUT_OF_RANGE: rg }, AbortError: Ue } = ne(), { validateAbortSignal: lr, validateInteger: ng, validateObject: ur } = Ur(), ig = V().Symbol("kWeak"), { finished: og } = at(), { ArrayPrototypePush: ag, MathFloor: fg, Number: lg, NumberIsNaN: ug, Promise: hu, PromiseReject: pu, PromisePrototypeThen: sg, Symbol: gu } = V(), Yn = gu("kEmpty"), yu = gu("kEof");
  function Kn(e, t) {
    if (typeof e != "function")
      throw new qr("fn", ["Function", "AsyncFunction"], e);
    t != null && ur(t, "options"), t?.signal != null && lr(t.signal, "options.signal");
    let r = 1;
    return t?.concurrency != null && (r = fg(t.concurrency)), ng(r, "concurrency", 1), async function* () {
      var i, o;
      let a = new _u, f = this, u = [], l = a.signal, s = { signal: l }, c = () => a.abort();
      t != null && (i = t.signal) !== null && i !== undefined && i.aborted && c(), t == null || (o = t.signal) === null || o === undefined || o.addEventListener("abort", c);
      let h, d, y = false;
      function b() {
        y = true;
      }
      async function R() {
        try {
          for await (let m of f) {
            var _;
            if (y)
              return;
            if (l.aborted)
              throw new Ue;
            try {
              m = e(m, s);
            } catch (A) {
              m = pu(A);
            }
            m !== Yn && (typeof ((_ = m) === null || _ === undefined ? undefined : _.catch) == "function" && m.catch(b), u.push(m), h && (h(), h = null), !y && u.length && u.length >= r && await new hu((A) => {
              d = A;
            }));
          }
          u.push(yu);
        } catch (m) {
          let A = pu(m);
          sg(A, undefined, b), u.push(A);
        } finally {
          var E;
          y = true, h && (h(), h = null), t == null || (E = t.signal) === null || E === undefined || E.removeEventListener("abort", c);
        }
      }
      R();
      try {
        for (;; ) {
          for (;u.length > 0; ) {
            let _ = await u[0];
            if (_ === yu)
              return;
            if (l.aborted)
              throw new Ue;
            _ !== Yn && (yield _), u.shift(), d && (d(), d = null);
          }
          await new hu((_) => {
            h = _;
          });
        }
      } finally {
        a.abort(), y = true, d && (d(), d = null);
      }
    }.call(this);
  }
  function cg(e = undefined) {
    return e != null && ur(e, "options"), e?.signal != null && lr(e.signal, "options.signal"), async function* () {
      let r = 0;
      for await (let i of this) {
        var n;
        if (e != null && (n = e.signal) !== null && n !== undefined && n.aborted)
          throw new Ue({ cause: e.signal.reason });
        yield [r++, i];
      }
    }.call(this);
  }
  async function bu(e, t = undefined) {
    for await (let r of zo.call(this, e, t))
      return true;
    return false;
  }
  async function dg(e, t = undefined) {
    if (typeof e != "function")
      throw new qr("fn", ["Function", "AsyncFunction"], e);
    return !await bu.call(this, async (...r) => !await e(...r), t);
  }
  async function hg(e, t) {
    for await (let r of zo.call(this, e, t))
      return r;
  }
  async function pg(e, t) {
    if (typeof e != "function")
      throw new qr("fn", ["Function", "AsyncFunction"], e);
    async function r(n, i) {
      return await e(n, i), Yn;
    }
    for await (let n of Kn.call(this, r, t))
      ;
  }
  function zo(e, t) {
    if (typeof e != "function")
      throw new qr("fn", ["Function", "AsyncFunction"], e);
    async function r(n, i) {
      return await e(n, i) ? n : Yn;
    }
    return Kn.call(this, r, t);
  }
  var Co = class extends tg {
    constructor() {
      super("reduce"), this.message = "Reduce of an empty stream requires an initial value";
    }
  };
  async function yg(e, t, r) {
    var n;
    if (typeof e != "function")
      throw new qr("reducer", ["Function", "AsyncFunction"], e);
    r != null && ur(r, "options"), r?.signal != null && lr(r.signal, "options.signal");
    let i = arguments.length > 1;
    if (r != null && (n = r.signal) !== null && n !== undefined && n.aborted) {
      let l = new Ue(undefined, { cause: r.signal.reason });
      throw this.once("error", () => {
      }), await og(this.destroy(l)), l;
    }
    let o = new _u, a = o.signal;
    if (r != null && r.signal) {
      let l = { once: true, [ig]: this };
      r.signal.addEventListener("abort", () => o.abort(), l);
    }
    let f = false;
    try {
      for await (let l of this) {
        var u;
        if (f = true, r != null && (u = r.signal) !== null && u !== undefined && u.aborted)
          throw new Ue;
        i ? t = await e(t, l, { signal: a }) : (t = l, i = true);
      }
      if (!f && !i)
        throw new Co;
    } finally {
      o.abort();
    }
    return t;
  }
  async function _g(e) {
    e != null && ur(e, "options"), e?.signal != null && lr(e.signal, "options.signal");
    let t = [];
    for await (let n of this) {
      var r;
      if (e != null && (r = e.signal) !== null && r !== undefined && r.aborted)
        throw new Ue(undefined, { cause: e.signal.reason });
      ag(t, n);
    }
    return t;
  }
  function gg(e, t) {
    let r = Kn.call(this, e, t);
    return async function* () {
      for await (let i of r)
        yield* i;
    }.call(this);
  }
  function wu(e) {
    if (e = lg(e), ug(e))
      return 0;
    if (e < 0)
      throw new rg("number", ">= 0", e);
    return e;
  }
  function bg(e, t = undefined) {
    return t != null && ur(t, "options"), t?.signal != null && lr(t.signal, "options.signal"), e = wu(e), async function* () {
      var n;
      if (t != null && (n = t.signal) !== null && n !== undefined && n.aborted)
        throw new Ue;
      for await (let o of this) {
        var i;
        if (t != null && (i = t.signal) !== null && i !== undefined && i.aborted)
          throw new Ue;
        e-- <= 0 && (yield o);
      }
    }.call(this);
  }
  function wg(e, t = undefined) {
    return t != null && ur(t, "options"), t?.signal != null && lr(t.signal, "options.signal"), e = wu(e), async function* () {
      var n;
      if (t != null && (n = t.signal) !== null && n !== undefined && n.aborted)
        throw new Ue;
      for await (let o of this) {
        var i;
        if (t != null && (i = t.signal) !== null && i !== undefined && i.aborted)
          throw new Ue;
        if (e-- > 0)
          yield o;
        else
          return;
      }
    }.call(this);
  }
  Wo.exports.streamReturningOperators = { asIndexedPairs: cg, drop: bg, filter: zo, flatMap: gg, map: Kn, take: wg };
  Wo.exports.promiseReturningOperators = { every: dg, forEach: pg, reduce: yg, toArray: _g, some: bu, find: hg };
});
var Tt = g((PA, Tu) => {
  var ft = (it(), se(ye)), { aggregateTwoErrors: Eg, codes: { ERR_MULTIPLE_CALLBACK: vg }, AbortError: mg } = ne(), { Symbol: Su } = V(), { kDestroyed: Sg, isDestroyed: Ag, isFinished: xg, isServerRequest: Rg } = ot(), Au = Su("kDestroy"), Zo = Su("kConstruct");
  function xu(e, t, r) {
    e && (e.stack, t && !t.errored && (t.errored = e), r && !r.errored && (r.errored = e));
  }
  function Ig(e, t) {
    let r = this._readableState, n = this._writableState, i = n || r;
    return n && n.destroyed || r && r.destroyed ? (typeof t == "function" && t(), this) : (xu(e, n, r), n && (n.destroyed = true), r && (r.destroyed = true), i.constructed ? vu(this, e, t) : this.once(Au, function(o) {
      vu(this, Eg(o, e), t);
    }), this);
  }
  function vu(e, t, r) {
    let n = false;
    function i(o) {
      if (n)
        return;
      n = true;
      let { _readableState: a, _writableState: f } = e;
      xu(o, f, a), f && (f.closed = true), a && (a.closed = true), typeof r == "function" && r(o), o ? ft.nextTick(Tg, e, o) : ft.nextTick(Ru, e);
    }
    try {
      e._destroy(t || null, i);
    } catch (o) {
      i(o);
    }
  }
  function Tg(e, t) {
    $o(e, t), Ru(e);
  }
  function Ru(e) {
    let { _readableState: t, _writableState: r } = e;
    r && (r.closeEmitted = true), t && (t.closeEmitted = true), (r && r.emitClose || t && t.emitClose) && e.emit("close");
  }
  function $o(e, t) {
    let { _readableState: r, _writableState: n } = e;
    n && n.errorEmitted || r && r.errorEmitted || (n && (n.errorEmitted = true), r && (r.errorEmitted = true), e.emit("error", t));
  }
  function Og() {
    let e = this._readableState, t = this._writableState;
    e && (e.constructed = true, e.closed = false, e.closeEmitted = false, e.destroyed = false, e.errored = null, e.errorEmitted = false, e.reading = false, e.ended = e.readable === false, e.endEmitted = e.readable === false), t && (t.constructed = true, t.destroyed = false, t.closed = false, t.closeEmitted = false, t.errored = null, t.errorEmitted = false, t.finalCalled = false, t.prefinished = false, t.ended = t.writable === false, t.ending = t.writable === false, t.finished = t.writable === false);
  }
  function Go(e, t, r) {
    let { _readableState: n, _writableState: i } = e;
    if (i && i.destroyed || n && n.destroyed)
      return this;
    n && n.autoDestroy || i && i.autoDestroy ? e.destroy(t) : t && (t.stack, i && !i.errored && (i.errored = t), n && !n.errored && (n.errored = t), r ? ft.nextTick($o, e, t) : $o(e, t));
  }
  function Ng(e, t) {
    if (typeof e._construct != "function")
      return;
    let { _readableState: r, _writableState: n } = e;
    r && (r.constructed = false), n && (n.constructed = false), e.once(Zo, t), !(e.listenerCount(Zo) > 1) && ft.nextTick(kg, e);
  }
  function kg(e) {
    let t = false;
    function r(n) {
      if (t) {
        Go(e, n ?? new vg);
        return;
      }
      t = true;
      let { _readableState: i, _writableState: o } = e, a = o || i;
      i && (i.constructed = true), o && (o.constructed = true), a.destroyed ? e.emit(Au, n) : n ? Go(e, n, true) : ft.nextTick(Fg, e);
    }
    try {
      e._construct(r);
    } catch (n) {
      r(n);
    }
  }
  function Fg(e) {
    e.emit(Zo);
  }
  function mu(e) {
    return e && e.setHeader && typeof e.abort == "function";
  }
  function Iu(e) {
    e.emit("close");
  }
  function Lg(e, t) {
    e.emit("error", t), ft.nextTick(Iu, e);
  }
  function Dg(e, t) {
    !e || Ag(e) || (!t && !xg(e) && (t = new mg), Rg(e) ? (e.socket = null, e.destroy(t)) : mu(e) ? e.abort() : mu(e.req) ? e.req.abort() : typeof e.destroy == "function" ? e.destroy(t) : typeof e.close == "function" ? e.close() : t ? ft.nextTick(Lg, e, t) : ft.nextTick(Iu, e), e.destroyed || (e[Sg] = true));
  }
  Tu.exports = { construct: Ng, destroyer: Dg, destroy: Ig, undestroy: Og, errorOrDestroy: Go };
});
var Qn = g((MA, Ho) => {
  var sr = typeof Reflect == "object" ? Reflect : null, Ou = sr && typeof sr.apply == "function" ? sr.apply : function(t, r, n) {
    return Function.prototype.apply.call(t, r, n);
  }, Xn;
  sr && typeof sr.ownKeys == "function" ? Xn = sr.ownKeys : Object.getOwnPropertySymbols ? Xn = function(t) {
    return Object.getOwnPropertyNames(t).concat(Object.getOwnPropertySymbols(t));
  } : Xn = function(t) {
    return Object.getOwnPropertyNames(t);
  };
  function Bg(e) {
    console && console.warn && console.warn(e);
  }
  var ku = Number.isNaN || function(t) {
    return t !== t;
  };
  function U() {
    U.init.call(this);
  }
  Ho.exports = U;
  Ho.exports.once = Ug;
  U.EventEmitter = U;
  U.prototype._events = undefined;
  U.prototype._eventsCount = 0;
  U.prototype._maxListeners = undefined;
  var Nu = 10;
  function Jn(e) {
    if (typeof e != "function")
      throw new TypeError('The "listener" argument must be of type Function. Received type ' + typeof e);
  }
  Object.defineProperty(U, "defaultMaxListeners", { enumerable: true, get: function() {
    return Nu;
  }, set: function(e) {
    if (typeof e != "number" || e < 0 || ku(e))
      throw new RangeError('The value of "defaultMaxListeners" is out of range. It must be a non-negative number. Received ' + e + ".");
    Nu = e;
  } });
  U.init = function() {
    (this._events === undefined || this._events === Object.getPrototypeOf(this)._events) && (this._events = Object.create(null), this._eventsCount = 0), this._maxListeners = this._maxListeners || undefined;
  };
  U.prototype.setMaxListeners = function(t) {
    if (typeof t != "number" || t < 0 || ku(t))
      throw new RangeError('The value of "n" is out of range. It must be a non-negative number. Received ' + t + ".");
    return this._maxListeners = t, this;
  };
  function Fu(e) {
    return e._maxListeners === undefined ? U.defaultMaxListeners : e._maxListeners;
  }
  U.prototype.getMaxListeners = function() {
    return Fu(this);
  };
  U.prototype.emit = function(t) {
    for (var r = [], n = 1;n < arguments.length; n++)
      r.push(arguments[n]);
    var i = t === "error", o = this._events;
    if (o !== undefined)
      i = i && o.error === undefined;
    else if (!i)
      return false;
    if (i) {
      var a;
      if (r.length > 0 && (a = r[0]), a instanceof Error)
        throw a;
      var f = new Error("Unhandled error." + (a ? " (" + a.message + ")" : ""));
      throw f.context = a, f;
    }
    var u = o[t];
    if (u === undefined)
      return false;
    if (typeof u == "function")
      Ou(u, this, r);
    else
      for (var l = u.length, s = Mu(u, l), n = 0;n < l; ++n)
        Ou(s[n], this, r);
    return true;
  };
  function Lu(e, t, r, n) {
    var i, o, a;
    if (Jn(r), o = e._events, o === undefined ? (o = e._events = Object.create(null), e._eventsCount = 0) : (o.newListener !== undefined && (e.emit("newListener", t, r.listener ? r.listener : r), o = e._events), a = o[t]), a === undefined)
      a = o[t] = r, ++e._eventsCount;
    else if (typeof a == "function" ? a = o[t] = n ? [r, a] : [a, r] : n ? a.unshift(r) : a.push(r), i = Fu(e), i > 0 && a.length > i && !a.warned) {
      a.warned = true;
      var f = new Error("Possible EventEmitter memory leak detected. " + a.length + " " + String(t) + " listeners added. Use emitter.setMaxListeners() to increase limit");
      f.name = "MaxListenersExceededWarning", f.emitter = e, f.type = t, f.count = a.length, Bg(f);
    }
    return e;
  }
  U.prototype.addListener = function(t, r) {
    return Lu(this, t, r, false);
  };
  U.prototype.on = U.prototype.addListener;
  U.prototype.prependListener = function(t, r) {
    return Lu(this, t, r, true);
  };
  function Pg() {
    if (!this.fired)
      return this.target.removeListener(this.type, this.wrapFn), this.fired = true, arguments.length === 0 ? this.listener.call(this.target) : this.listener.apply(this.target, arguments);
  }
  function Du(e, t, r) {
    var n = { fired: false, wrapFn: undefined, target: e, type: t, listener: r }, i = Pg.bind(n);
    return i.listener = r, n.wrapFn = i, i;
  }
  U.prototype.once = function(t, r) {
    return Jn(r), this.on(t, Du(this, t, r)), this;
  };
  U.prototype.prependOnceListener = function(t, r) {
    return Jn(r), this.prependListener(t, Du(this, t, r)), this;
  };
  U.prototype.removeListener = function(t, r) {
    var n, i, o, a, f;
    if (Jn(r), i = this._events, i === undefined)
      return this;
    if (n = i[t], n === undefined)
      return this;
    if (n === r || n.listener === r)
      --this._eventsCount === 0 ? this._events = Object.create(null) : (delete i[t], i.removeListener && this.emit("removeListener", t, n.listener || r));
    else if (typeof n != "function") {
      for (o = -1, a = n.length - 1;a >= 0; a--)
        if (n[a] === r || n[a].listener === r) {
          f = n[a].listener, o = a;
          break;
        }
      if (o < 0)
        return this;
      o === 0 ? n.shift() : Mg(n, o), n.length === 1 && (i[t] = n[0]), i.removeListener !== undefined && this.emit("removeListener", t, f || r);
    }
    return this;
  };
  U.prototype.off = U.prototype.removeListener;
  U.prototype.removeAllListeners = function(t) {
    var r, n, i;
    if (n = this._events, n === undefined)
      return this;
    if (n.removeListener === undefined)
      return arguments.length === 0 ? (this._events = Object.create(null), this._eventsCount = 0) : n[t] !== undefined && (--this._eventsCount === 0 ? this._events = Object.create(null) : delete n[t]), this;
    if (arguments.length === 0) {
      var o = Object.keys(n), a;
      for (i = 0;i < o.length; ++i)
        a = o[i], a !== "removeListener" && this.removeAllListeners(a);
      return this.removeAllListeners("removeListener"), this._events = Object.create(null), this._eventsCount = 0, this;
    }
    if (r = n[t], typeof r == "function")
      this.removeListener(t, r);
    else if (r !== undefined)
      for (i = r.length - 1;i >= 0; i--)
        this.removeListener(t, r[i]);
    return this;
  };
  function Bu(e, t, r) {
    var n = e._events;
    if (n === undefined)
      return [];
    var i = n[t];
    return i === undefined ? [] : typeof i == "function" ? r ? [i.listener || i] : [i] : r ? jg(i) : Mu(i, i.length);
  }
  U.prototype.listeners = function(t) {
    return Bu(this, t, true);
  };
  U.prototype.rawListeners = function(t) {
    return Bu(this, t, false);
  };
  U.listenerCount = function(e, t) {
    return typeof e.listenerCount == "function" ? e.listenerCount(t) : Pu.call(e, t);
  };
  U.prototype.listenerCount = Pu;
  function Pu(e) {
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
  U.prototype.eventNames = function() {
    return this._eventsCount > 0 ? Xn(this._events) : [];
  };
  function Mu(e, t) {
    for (var r = new Array(t), n = 0;n < t; ++n)
      r[n] = e[n];
    return r;
  }
  function Mg(e, t) {
    for (;t + 1 < e.length; t++)
      e[t] = e[t + 1];
    e.pop();
  }
  function jg(e) {
    for (var t = new Array(e.length), r = 0;r < t.length; ++r)
      t[r] = e[r].listener || e[r];
    return t;
  }
  function Ug(e, t) {
    return new Promise(function(r, n) {
      function i(a) {
        e.removeListener(t, o), n(a);
      }
      function o() {
        typeof e.removeListener == "function" && e.removeListener("error", i), r([].slice.call(arguments));
      }
      ju(e, t, o, { once: true }), t !== "error" && qg(e, i, { once: true });
    });
  }
  function qg(e, t, r) {
    typeof e.on == "function" && ju(e, "error", t, r);
  }
  function ju(e, t, r, n) {
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
var ri = g((jA, qu) => {
  var { ArrayIsArray: Cg, ObjectSetPrototypeOf: Uu } = V(), { EventEmitter: ei } = Qn();
  function ti(e) {
    ei.call(this, e);
  }
  Uu(ti.prototype, ei.prototype);
  Uu(ti, ei);
  ti.prototype.pipe = function(e, t) {
    let r = this;
    function n(s) {
      e.writable && e.write(s) === false && r.pause && r.pause();
    }
    r.on("data", n);
    function i() {
      r.readable && r.resume && r.resume();
    }
    e.on("drain", i), !e._isStdio && (!t || t.end !== false) && (r.on("end", a), r.on("close", f));
    let o = false;
    function a() {
      o || (o = true, e.end());
    }
    function f() {
      o || (o = true, typeof e.destroy == "function" && e.destroy());
    }
    function u(s) {
      l(), ei.listenerCount(this, "error") === 0 && this.emit("error", s);
    }
    Vo(r, "error", u), Vo(e, "error", u);
    function l() {
      r.removeListener("data", n), e.removeListener("drain", i), r.removeListener("end", a), r.removeListener("close", f), r.removeListener("error", u), e.removeListener("error", u), r.removeListener("end", l), r.removeListener("close", l), e.removeListener("close", l);
    }
    return r.on("end", l), r.on("close", l), e.on("close", l), e.emit("pipe", r), e;
  };
  function Vo(e, t, r) {
    if (typeof e.prependListener == "function")
      return e.prependListener(t, r);
    !e._events || !e._events[t] ? e.on(t, r) : Cg(e._events[t]) ? e._events[t].unshift(r) : e._events[t] = [r, e._events[t]];
  }
  qu.exports = { Stream: ti, prependListener: Vo };
});
var ii = g((UA, ni) => {
  var { AbortError: zg, codes: Wg } = ne(), Zg = at(), { ERR_INVALID_ARG_TYPE: Cu } = Wg, $g = (e, t) => {
    if (typeof e != "object" || !("aborted" in e))
      throw new Cu(t, "AbortSignal", e);
  };
  function Gg(e) {
    return !!(e && typeof e.pipe == "function");
  }
  ni.exports.addAbortSignal = function(t, r) {
    if ($g(t, "signal"), !Gg(r))
      throw new Cu("stream", "stream.Stream", r);
    return ni.exports.addAbortSignalNoValidate(t, r);
  };
  ni.exports.addAbortSignalNoValidate = function(e, t) {
    if (typeof e != "object" || !("aborted" in e))
      return t;
    let r = () => {
      t.destroy(new zg(undefined, { cause: e.reason }));
    };
    return e.aborted ? r() : (e.addEventListener("abort", r), Zg(t, () => e.removeEventListener("abort", r))), t;
  };
});
var Zu = g((CA, Wu) => {
  var { StringPrototypeSlice: zu, SymbolIterator: Hg, TypedArrayPrototypeSet: oi, Uint8Array: Vg } = V(), { Buffer: Yo } = xe(), { inspect: Yg } = Pe();
  Wu.exports = class {
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
        return Yo.alloc(0);
      let r = Yo.allocUnsafe(t >>> 0), n = this.head, i = 0;
      for (;n; )
        oi(r, n.data, i), i += n.data.length, n = n.next;
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
    *[Hg]() {
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
          t === o.length ? (r += o, ++i, n.next ? this.head = n.next : this.head = this.tail = null) : (r += zu(o, 0, t), this.head = n, n.data = zu(o, t));
          break;
        }
        ++i;
      } while ((n = n.next) !== null);
      return this.length -= i, r;
    }
    _getBuffer(t) {
      let r = Yo.allocUnsafe(t), n = t, i = this.head, o = 0;
      do {
        let a = i.data;
        if (t > a.length)
          oi(r, a, n - t), t -= a.length;
        else {
          t === a.length ? (oi(r, a, n - t), ++o, i.next ? this.head = i.next : this.head = this.tail = null) : (oi(r, new Vg(a.buffer, a.byteOffset, t), n - t), this.head = i, i.data = a.slice(t));
          break;
        }
        ++o;
      } while ((i = i.next) !== null);
      return this.length -= o, r;
    }
    [Symbol.for("nodejs.util.inspect.custom")](t, r) {
      return Yg(this, { ...r, depth: 0, customInspect: false });
    }
  };
});
var ai = g((zA, Gu) => {
  var { MathFloor: Kg, NumberIsInteger: Xg } = V(), { ERR_INVALID_ARG_VALUE: Jg } = ne().codes;
  function Qg(e, t, r) {
    return e.highWaterMark != null ? e.highWaterMark : t ? e[r] : null;
  }
  function $u(e) {
    return e ? 16 : 16 * 1024;
  }
  function eb(e, t, r, n) {
    let i = Qg(t, n, r);
    if (i != null) {
      if (!Xg(i) || i < 0) {
        let o = n ? `options.${r}` : "options.highWaterMark";
        throw new Jg(o, i);
      }
      return Kg(i);
    }
    return $u(e.objectMode);
  }
  Gu.exports = { getHighWaterMark: eb, getDefaultHighWaterMark: $u };
});
var Yu = g((Ko, Vu) => {
  var fi = xe(), qe = fi.Buffer;
  function Hu(e, t) {
    for (var r in e)
      t[r] = e[r];
  }
  qe.from && qe.alloc && qe.allocUnsafe && qe.allocUnsafeSlow ? Vu.exports = fi : (Hu(fi, Ko), Ko.Buffer = Ot);
  function Ot(e, t, r) {
    return qe(e, t, r);
  }
  Ot.prototype = Object.create(qe.prototype);
  Hu(qe, Ot);
  Ot.from = function(e, t, r) {
    if (typeof e == "number")
      throw new TypeError("Argument must not be a number");
    return qe(e, t, r);
  };
  Ot.alloc = function(e, t, r) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    var n = qe(e);
    return t !== undefined ? typeof r == "string" ? n.fill(t, r) : n.fill(t) : n.fill(0), n;
  };
  Ot.allocUnsafe = function(e) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    return qe(e);
  };
  Ot.allocUnsafeSlow = function(e) {
    if (typeof e != "number")
      throw new TypeError("Argument must be a number");
    return fi.SlowBuffer(e);
  };
});
var Ju = g((Xu) => {
  var Jo = Yu().Buffer, Ku = Jo.isEncoding || function(e) {
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
  function tb(e) {
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
  function rb(e) {
    var t = tb(e);
    if (typeof t != "string" && (Jo.isEncoding === Ku || !Ku(e)))
      throw new Error("Unknown encoding: " + e);
    return t || e;
  }
  Xu.StringDecoder = Cr;
  function Cr(e) {
    this.encoding = rb(e);
    var t;
    switch (this.encoding) {
      case "utf16le":
        this.text = lb, this.end = ub, t = 4;
        break;
      case "utf8":
        this.fillLast = ob, t = 4;
        break;
      case "base64":
        this.text = sb, this.end = cb, t = 3;
        break;
      default:
        this.write = db, this.end = hb;
        return;
    }
    this.lastNeed = 0, this.lastTotal = 0, this.lastChar = Jo.allocUnsafe(t);
  }
  Cr.prototype.write = function(e) {
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
  Cr.prototype.end = fb;
  Cr.prototype.text = ab;
  Cr.prototype.fillLast = function(e) {
    if (this.lastNeed <= e.length)
      return e.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
    e.copy(this.lastChar, this.lastTotal - this.lastNeed, 0, e.length), this.lastNeed -= e.length;
  };
  function Xo(e) {
    return e <= 127 ? 0 : e >> 5 === 6 ? 2 : e >> 4 === 14 ? 3 : e >> 3 === 30 ? 4 : e >> 6 === 2 ? -1 : -2;
  }
  function nb(e, t, r) {
    var n = t.length - 1;
    if (n < r)
      return 0;
    var i = Xo(t[n]);
    return i >= 0 ? (i > 0 && (e.lastNeed = i - 1), i) : --n < r || i === -2 ? 0 : (i = Xo(t[n]), i >= 0 ? (i > 0 && (e.lastNeed = i - 2), i) : --n < r || i === -2 ? 0 : (i = Xo(t[n]), i >= 0 ? (i > 0 && (i === 2 ? i = 0 : e.lastNeed = i - 3), i) : 0));
  }
  function ib(e, t, r) {
    if ((t[0] & 192) !== 128)
      return e.lastNeed = 0, "�";
    if (e.lastNeed > 1 && t.length > 1) {
      if ((t[1] & 192) !== 128)
        return e.lastNeed = 1, "�";
      if (e.lastNeed > 2 && t.length > 2 && (t[2] & 192) !== 128)
        return e.lastNeed = 2, "�";
    }
  }
  function ob(e) {
    var t = this.lastTotal - this.lastNeed, r = ib(this, e, t);
    if (r !== undefined)
      return r;
    if (this.lastNeed <= e.length)
      return e.copy(this.lastChar, t, 0, this.lastNeed), this.lastChar.toString(this.encoding, 0, this.lastTotal);
    e.copy(this.lastChar, t, 0, e.length), this.lastNeed -= e.length;
  }
  function ab(e, t) {
    var r = nb(this, e, t);
    if (!this.lastNeed)
      return e.toString("utf8", t);
    this.lastTotal = r;
    var n = e.length - (r - this.lastNeed);
    return e.copy(this.lastChar, 0, n), e.toString("utf8", t, n);
  }
  function fb(e) {
    var t = e && e.length ? this.write(e) : "";
    return this.lastNeed ? t + "�" : t;
  }
  function lb(e, t) {
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
  function ub(e) {
    var t = e && e.length ? this.write(e) : "";
    if (this.lastNeed) {
      var r = this.lastTotal - this.lastNeed;
      return t + this.lastChar.toString("utf16le", 0, r);
    }
    return t;
  }
  function sb(e, t) {
    var r = (e.length - t) % 3;
    return r === 0 ? e.toString("base64", t) : (this.lastNeed = 3 - r, this.lastTotal = 3, r === 1 ? this.lastChar[0] = e[e.length - 1] : (this.lastChar[0] = e[e.length - 2], this.lastChar[1] = e[e.length - 1]), e.toString("base64", t, e.length - r));
  }
  function cb(e) {
    var t = e && e.length ? this.write(e) : "";
    return this.lastNeed ? t + this.lastChar.toString("base64", 0, 3 - this.lastNeed) : t;
  }
  function db(e) {
    return e.toString(this.encoding);
  }
  function hb(e) {
    return e && e.length ? this.write(e) : "";
  }
});
var Qo = g((ZA, rs) => {
  var Qu = (it(), se(ye)), { PromisePrototypeThen: pb, SymbolAsyncIterator: es, SymbolIterator: ts } = V(), { Buffer: yb } = xe(), { ERR_INVALID_ARG_TYPE: _b, ERR_STREAM_NULL_VALUES: gb } = ne().codes;
  function bb(e, t, r) {
    let n;
    if (typeof t == "string" || t instanceof yb)
      return new e({ objectMode: true, ...r, read() {
        this.push(t), this.push(null);
      } });
    let i;
    if (t && t[es])
      i = true, n = t[es]();
    else if (t && t[ts])
      i = false, n = t[ts]();
    else
      throw new _b("iterable", ["Iterable"], t);
    let o = new e({ objectMode: true, highWaterMark: 1, ...r }), a = false;
    o._read = function() {
      a || (a = true, u());
    }, o._destroy = function(l, s) {
      pb(f(l), () => Qu.nextTick(s, l), (c) => Qu.nextTick(s, c || l));
    };
    async function f(l) {
      let s = l != null, c = typeof n.throw == "function";
      if (s && c) {
        let { value: h, done: d } = await n.throw(l);
        if (await h, d)
          return;
      }
      if (typeof n.return == "function") {
        let { value: h } = await n.return();
        await h;
      }
    }
    async function u() {
      for (;; ) {
        try {
          let { value: l, done: s } = i ? await n.next() : n.next();
          if (s)
            o.push(null);
          else {
            let c = l && typeof l.then == "function" ? await l : l;
            if (c === null)
              throw a = false, new gb;
            if (o.push(c))
              continue;
            a = false;
          }
        } catch (l) {
          o.destroy(l);
        }
        break;
      }
    }
    return o;
  }
  rs.exports = bb;
});
var zr = g(($A, ys) => {
  var Ie = (it(), se(ye)), { ArrayPrototypeIndexOf: wb, NumberIsInteger: Eb, NumberIsNaN: vb, NumberParseInt: mb, ObjectDefineProperties: os, ObjectKeys: Sb, ObjectSetPrototypeOf: as, Promise: Ab, SafeSet: xb, SymbolAsyncIterator: Rb, Symbol: Ib } = V();
  ys.exports = F;
  F.ReadableState = oa;
  var { EventEmitter: Tb } = Qn(), { Stream: lt, prependListener: Ob } = ri(), { Buffer: ea } = xe(), { addAbortSignal: Nb } = ii(), kb = at(), B = Pe().debuglog("stream", (e) => {
    B = e;
  }), Fb = Zu(), dr = Tt(), { getHighWaterMark: Lb, getDefaultHighWaterMark: Db } = ai(), { aggregateTwoErrors: ns, codes: { ERR_INVALID_ARG_TYPE: Bb, ERR_METHOD_NOT_IMPLEMENTED: Pb, ERR_OUT_OF_RANGE: Mb, ERR_STREAM_PUSH_AFTER_EOF: jb, ERR_STREAM_UNSHIFT_AFTER_END_EVENT: Ub } } = ne(), { validateObject: qb } = Ur(), Nt = Ib("kPaused"), { StringDecoder: fs } = Ju(), Cb = Qo();
  as(F.prototype, lt.prototype);
  as(F, lt);
  var ta = () => {
  }, { errorOrDestroy: cr } = dr;
  function oa(e, t, r) {
    typeof r != "boolean" && (r = t instanceof Ce()), this.objectMode = !!(e && e.objectMode), r && (this.objectMode = this.objectMode || !!(e && e.readableObjectMode)), this.highWaterMark = e ? Lb(this, e, "readableHighWaterMark", r) : Db(false), this.buffer = new Fb, this.length = 0, this.pipes = [], this.flowing = null, this.ended = false, this.endEmitted = false, this.reading = false, this.constructed = true, this.sync = true, this.needReadable = false, this.emittedReadable = false, this.readableListening = false, this.resumeScheduled = false, this[Nt] = null, this.errorEmitted = false, this.emitClose = !e || e.emitClose !== false, this.autoDestroy = !e || e.autoDestroy !== false, this.destroyed = false, this.errored = null, this.closed = false, this.closeEmitted = false, this.defaultEncoding = e && e.defaultEncoding || "utf8", this.awaitDrainWriters = null, this.multiAwaitDrain = false, this.readingMore = false, this.dataEmitted = false, this.decoder = null, this.encoding = null, e && e.encoding && (this.decoder = new fs(e.encoding), this.encoding = e.encoding);
  }
  function F(e) {
    if (!(this instanceof F))
      return new F(e);
    let t = this instanceof Ce();
    this._readableState = new oa(e, this, t), e && (typeof e.read == "function" && (this._read = e.read), typeof e.destroy == "function" && (this._destroy = e.destroy), typeof e.construct == "function" && (this._construct = e.construct), e.signal && !t && Nb(e.signal, this)), lt.call(this, e), dr.construct(this, () => {
      this._readableState.needReadable && li(this, this._readableState);
    });
  }
  F.prototype.destroy = dr.destroy;
  F.prototype._undestroy = dr.undestroy;
  F.prototype._destroy = function(e, t) {
    t(e);
  };
  F.prototype[Tb.captureRejectionSymbol] = function(e) {
    this.destroy(e);
  };
  F.prototype.push = function(e, t) {
    return ls(this, e, t, false);
  };
  F.prototype.unshift = function(e, t) {
    return ls(this, e, t, true);
  };
  function ls(e, t, r, n) {
    B("readableAddChunk", t);
    let i = e._readableState, o;
    if (i.objectMode || (typeof t == "string" ? (r = r || i.defaultEncoding, i.encoding !== r && (n && i.encoding ? t = ea.from(t, r).toString(i.encoding) : (t = ea.from(t, r), r = ""))) : t instanceof ea ? r = "" : lt._isUint8Array(t) ? (t = lt._uint8ArrayToBuffer(t), r = "") : t != null && (o = new Bb("chunk", ["string", "Buffer", "Uint8Array"], t))), o)
      cr(e, o);
    else if (t === null)
      i.reading = false, Zb(e, i);
    else if (i.objectMode || t && t.length > 0)
      if (n)
        if (i.endEmitted)
          cr(e, new Ub);
        else {
          if (i.destroyed || i.errored)
            return false;
          ra(e, i, t, true);
        }
      else if (i.ended)
        cr(e, new jb);
      else {
        if (i.destroyed || i.errored)
          return false;
        i.reading = false, i.decoder && !r ? (t = i.decoder.write(t), i.objectMode || t.length !== 0 ? ra(e, i, t, false) : li(e, i)) : ra(e, i, t, false);
      }
    else
      n || (i.reading = false, li(e, i));
    return !i.ended && (i.length < i.highWaterMark || i.length === 0);
  }
  function ra(e, t, r, n) {
    t.flowing && t.length === 0 && !t.sync && e.listenerCount("data") > 0 ? (t.multiAwaitDrain ? t.awaitDrainWriters.clear() : t.awaitDrainWriters = null, t.dataEmitted = true, e.emit("data", r)) : (t.length += t.objectMode ? 1 : r.length, n ? t.buffer.unshift(r) : t.buffer.push(r), t.needReadable && ui(e)), li(e, t);
  }
  F.prototype.isPaused = function() {
    let e = this._readableState;
    return e[Nt] === true || e.flowing === false;
  };
  F.prototype.setEncoding = function(e) {
    let t = new fs(e);
    this._readableState.decoder = t, this._readableState.encoding = this._readableState.decoder.encoding;
    let r = this._readableState.buffer, n = "";
    for (let i of r)
      n += t.write(i);
    return r.clear(), n !== "" && r.push(n), this._readableState.length = n.length, this;
  };
  var zb = 1073741824;
  function Wb(e) {
    if (e > zb)
      throw new Mb("size", "<= 1GiB", e);
    return e--, e |= e >>> 1, e |= e >>> 2, e |= e >>> 4, e |= e >>> 8, e |= e >>> 16, e++, e;
  }
  function is(e, t) {
    return e <= 0 || t.length === 0 && t.ended ? 0 : t.objectMode ? 1 : vb(e) ? t.flowing && t.length ? t.buffer.first().length : t.length : e <= t.length ? e : t.ended ? t.length : 0;
  }
  F.prototype.read = function(e) {
    B("read", e), e === undefined ? e = NaN : Eb(e) || (e = mb(e, 10));
    let t = this._readableState, r = e;
    if (e > t.highWaterMark && (t.highWaterMark = Wb(e)), e !== 0 && (t.emittedReadable = false), e === 0 && t.needReadable && ((t.highWaterMark !== 0 ? t.length >= t.highWaterMark : t.length > 0) || t.ended))
      return B("read: emitReadable", t.length, t.ended), t.length === 0 && t.ended ? na(this) : ui(this), null;
    if (e = is(e, t), e === 0 && t.ended)
      return t.length === 0 && na(this), null;
    let n = t.needReadable;
    if (B("need readable", n), (t.length === 0 || t.length - e < t.highWaterMark) && (n = true, B("length less than watermark", n)), t.ended || t.reading || t.destroyed || t.errored || !t.constructed)
      n = false, B("reading, ended or constructing", n);
    else if (n) {
      B("do read"), t.reading = true, t.sync = true, t.length === 0 && (t.needReadable = true);
      try {
        this._read(t.highWaterMark);
      } catch (o) {
        cr(this, o);
      }
      t.sync = false, t.reading || (e = is(r, t));
    }
    let i;
    return e > 0 ? i = hs(e, t) : i = null, i === null ? (t.needReadable = t.length <= t.highWaterMark, e = 0) : (t.length -= e, t.multiAwaitDrain ? t.awaitDrainWriters.clear() : t.awaitDrainWriters = null), t.length === 0 && (t.ended || (t.needReadable = true), r !== e && t.ended && na(this)), i !== null && !t.errorEmitted && !t.closeEmitted && (t.dataEmitted = true, this.emit("data", i)), i;
  };
  function Zb(e, t) {
    if (B("onEofChunk"), !t.ended) {
      if (t.decoder) {
        let r = t.decoder.end();
        r && r.length && (t.buffer.push(r), t.length += t.objectMode ? 1 : r.length);
      }
      t.ended = true, t.sync ? ui(e) : (t.needReadable = false, t.emittedReadable = true, us(e));
    }
  }
  function ui(e) {
    let t = e._readableState;
    B("emitReadable", t.needReadable, t.emittedReadable), t.needReadable = false, t.emittedReadable || (B("emitReadable", t.flowing), t.emittedReadable = true, Ie.nextTick(us, e));
  }
  function us(e) {
    let t = e._readableState;
    B("emitReadable_", t.destroyed, t.length, t.ended), !t.destroyed && !t.errored && (t.length || t.ended) && (e.emit("readable"), t.emittedReadable = false), t.needReadable = !t.flowing && !t.ended && t.length <= t.highWaterMark, cs(e);
  }
  function li(e, t) {
    !t.readingMore && t.constructed && (t.readingMore = true, Ie.nextTick($b, e, t));
  }
  function $b(e, t) {
    for (;!t.reading && !t.ended && (t.length < t.highWaterMark || t.flowing && t.length === 0); ) {
      let r = t.length;
      if (B("maybeReadMore read 0"), e.read(0), r === t.length)
        break;
    }
    t.readingMore = false;
  }
  F.prototype._read = function(e) {
    throw new Pb("_read()");
  };
  F.prototype.pipe = function(e, t) {
    let r = this, n = this._readableState;
    n.pipes.length === 1 && (n.multiAwaitDrain || (n.multiAwaitDrain = true, n.awaitDrainWriters = new xb(n.awaitDrainWriters ? [n.awaitDrainWriters] : []))), n.pipes.push(e), B("pipe count=%d opts=%j", n.pipes.length, t);
    let o = (!t || t.end !== false) && e !== Ie.stdout && e !== Ie.stderr ? f : R;
    n.endEmitted ? Ie.nextTick(o) : r.once("end", o), e.on("unpipe", a);
    function a(_, E) {
      B("onunpipe"), _ === r && E && E.hasUnpiped === false && (E.hasUnpiped = true, s());
    }
    function f() {
      B("onend"), e.end();
    }
    let u, l = false;
    function s() {
      B("cleanup"), e.removeListener("close", y), e.removeListener("finish", b), u && e.removeListener("drain", u), e.removeListener("error", d), e.removeListener("unpipe", a), r.removeListener("end", f), r.removeListener("end", R), r.removeListener("data", h), l = true, u && n.awaitDrainWriters && (!e._writableState || e._writableState.needDrain) && u();
    }
    function c() {
      l || (n.pipes.length === 1 && n.pipes[0] === e ? (B("false write response, pause", 0), n.awaitDrainWriters = e, n.multiAwaitDrain = false) : n.pipes.length > 1 && n.pipes.includes(e) && (B("false write response, pause", n.awaitDrainWriters.size), n.awaitDrainWriters.add(e)), r.pause()), u || (u = Gb(r, e), e.on("drain", u));
    }
    r.on("data", h);
    function h(_) {
      B("ondata");
      let E = e.write(_);
      B("dest.write", E), E === false && c();
    }
    function d(_) {
      if (B("onerror", _), R(), e.removeListener("error", d), e.listenerCount("error") === 0) {
        let E = e._writableState || e._readableState;
        E && !E.errorEmitted ? cr(e, _) : e.emit("error", _);
      }
    }
    Ob(e, "error", d);
    function y() {
      e.removeListener("finish", b), R();
    }
    e.once("close", y);
    function b() {
      B("onfinish"), e.removeListener("close", y), R();
    }
    e.once("finish", b);
    function R() {
      B("unpipe"), r.unpipe(e);
    }
    return e.emit("pipe", r), e.writableNeedDrain === true ? n.flowing && c() : n.flowing || (B("pipe resume"), r.resume()), e;
  };
  function Gb(e, t) {
    return function() {
      let n = e._readableState;
      n.awaitDrainWriters === t ? (B("pipeOnDrain", 1), n.awaitDrainWriters = null) : n.multiAwaitDrain && (B("pipeOnDrain", n.awaitDrainWriters.size), n.awaitDrainWriters.delete(t)), (!n.awaitDrainWriters || n.awaitDrainWriters.size === 0) && e.listenerCount("data") && e.resume();
    };
  }
  F.prototype.unpipe = function(e) {
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
    let n = wb(t.pipes, e);
    return n === -1 ? this : (t.pipes.splice(n, 1), t.pipes.length === 0 && this.pause(), e.emit("unpipe", this, r), this);
  };
  F.prototype.on = function(e, t) {
    let r = lt.prototype.on.call(this, e, t), n = this._readableState;
    return e === "data" ? (n.readableListening = this.listenerCount("readable") > 0, n.flowing !== false && this.resume()) : e === "readable" && !n.endEmitted && !n.readableListening && (n.readableListening = n.needReadable = true, n.flowing = false, n.emittedReadable = false, B("on readable", n.length, n.reading), n.length ? ui(this) : n.reading || Ie.nextTick(Hb, this)), r;
  };
  F.prototype.addListener = F.prototype.on;
  F.prototype.removeListener = function(e, t) {
    let r = lt.prototype.removeListener.call(this, e, t);
    return e === "readable" && Ie.nextTick(ss, this), r;
  };
  F.prototype.off = F.prototype.removeListener;
  F.prototype.removeAllListeners = function(e) {
    let t = lt.prototype.removeAllListeners.apply(this, arguments);
    return (e === "readable" || e === undefined) && Ie.nextTick(ss, this), t;
  };
  function ss(e) {
    let t = e._readableState;
    t.readableListening = e.listenerCount("readable") > 0, t.resumeScheduled && t[Nt] === false ? t.flowing = true : e.listenerCount("data") > 0 ? e.resume() : t.readableListening || (t.flowing = null);
  }
  function Hb(e) {
    B("readable nexttick read 0"), e.read(0);
  }
  F.prototype.resume = function() {
    let e = this._readableState;
    return e.flowing || (B("resume"), e.flowing = !e.readableListening, Vb(this, e)), e[Nt] = false, this;
  };
  function Vb(e, t) {
    t.resumeScheduled || (t.resumeScheduled = true, Ie.nextTick(Yb, e, t));
  }
  function Yb(e, t) {
    B("resume", t.reading), t.reading || e.read(0), t.resumeScheduled = false, e.emit("resume"), cs(e), t.flowing && !t.reading && e.read(0);
  }
  F.prototype.pause = function() {
    return B("call pause flowing=%j", this._readableState.flowing), this._readableState.flowing !== false && (B("pause"), this._readableState.flowing = false, this.emit("pause")), this._readableState[Nt] = true, this;
  };
  function cs(e) {
    let t = e._readableState;
    for (B("flow", t.flowing);t.flowing && e.read() !== null; )
      ;
  }
  F.prototype.wrap = function(e) {
    let t = false;
    e.on("data", (n) => {
      !this.push(n) && e.pause && (t = true, e.pause());
    }), e.on("end", () => {
      this.push(null);
    }), e.on("error", (n) => {
      cr(this, n);
    }), e.on("close", () => {
      this.destroy();
    }), e.on("destroy", () => {
      this.destroy();
    }), this._read = () => {
      t && e.resume && (t = false, e.resume());
    };
    let r = Sb(e);
    for (let n = 1;n < r.length; n++) {
      let i = r[n];
      this[i] === undefined && typeof e[i] == "function" && (this[i] = e[i].bind(e));
    }
    return this;
  };
  F.prototype[Rb] = function() {
    return ds(this);
  };
  F.prototype.iterator = function(e) {
    return e !== undefined && qb(e, "options"), ds(this, e);
  };
  function ds(e, t) {
    typeof e.read != "function" && (e = F.wrap(e, { objectMode: true }));
    let r = Kb(e, t);
    return r.stream = e, r;
  }
  async function* Kb(e, t) {
    let r = ta;
    function n(a) {
      this === e ? (r(), r = ta) : r = a;
    }
    e.on("readable", n);
    let i, o = kb(e, { writable: false }, (a) => {
      i = a ? ns(i, a) : null, r(), r = ta;
    });
    try {
      for (;; ) {
        let a = e.destroyed ? null : e.read();
        if (a !== null)
          yield a;
        else {
          if (i)
            throw i;
          if (i === null)
            return;
          await new Ab(n);
        }
      }
    } catch (a) {
      throw i = ns(i, a), i;
    } finally {
      (i || t?.destroyOnReturn !== false) && (i === undefined || e._readableState.autoDestroy) ? dr.destroyer(e, null) : (e.off("readable", n), o());
    }
  }
  os(F.prototype, { readable: { __proto__: null, get() {
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
  os(oa.prototype, { pipesCount: { __proto__: null, get() {
    return this.pipes.length;
  } }, paused: { __proto__: null, get() {
    return this[Nt] !== false;
  }, set(e) {
    this[Nt] = !!e;
  } } });
  F._fromList = hs;
  function hs(e, t) {
    if (t.length === 0)
      return null;
    let r;
    return t.objectMode ? r = t.buffer.shift() : !e || e >= t.length ? (t.decoder ? r = t.buffer.join("") : t.buffer.length === 1 ? r = t.buffer.first() : r = t.buffer.concat(t.length), t.buffer.clear()) : r = t.buffer.consume(e, t.decoder), r;
  }
  function na(e) {
    let t = e._readableState;
    B("endReadable", t.endEmitted), t.endEmitted || (t.ended = true, Ie.nextTick(Xb, t, e));
  }
  function Xb(e, t) {
    if (B("endReadableNT", e.endEmitted, e.length), !e.errored && !e.closeEmitted && !e.endEmitted && e.length === 0) {
      if (e.endEmitted = true, t.emit("end"), t.writable && t.allowHalfOpen === false)
        Ie.nextTick(Jb, t);
      else if (e.autoDestroy) {
        let r = t._writableState;
        (!r || r.autoDestroy && (r.finished || r.writable === false)) && t.destroy();
      }
    }
  }
  function Jb(e) {
    e.writable && !e.writableEnded && !e.destroyed && e.end();
  }
  F.from = function(e, t) {
    return Cb(F, e, t);
  };
  var ia;
  function ps() {
    return ia === undefined && (ia = {}), ia;
  }
  F.fromWeb = function(e, t) {
    return ps().newStreamReadableFromReadableStream(e, t);
  };
  F.toWeb = function(e, t) {
    return ps().newReadableStreamFromStreamReadable(e, t);
  };
  F.wrap = function(e, t) {
    var r, n;
    return new F({ objectMode: (r = (n = e.readableObjectMode) !== null && n !== undefined ? n : e.objectMode) !== null && r !== undefined ? r : true, ...t, destroy(i, o) {
      dr.destroyer(e, i), o(i);
    } }).wrap(e);
  };
});
var da = g((GA, Is) => {
  var kt = (it(), se(ye)), { ArrayPrototypeSlice: bs, Error: Qb, FunctionPrototypeSymbolHasInstance: ws, ObjectDefineProperty: Es, ObjectDefineProperties: ew, ObjectSetPrototypeOf: vs, StringPrototypeToLowerCase: tw, Symbol: rw, SymbolHasInstance: nw } = V();
  Is.exports = W;
  W.WritableState = $r;
  var { EventEmitter: iw } = Qn(), Wr = ri().Stream, { Buffer: si } = xe(), hi = Tt(), { addAbortSignal: ow } = ii(), { getHighWaterMark: aw, getDefaultHighWaterMark: fw } = ai(), { ERR_INVALID_ARG_TYPE: lw, ERR_METHOD_NOT_IMPLEMENTED: uw, ERR_MULTIPLE_CALLBACK: ms, ERR_STREAM_CANNOT_PIPE: sw, ERR_STREAM_DESTROYED: Zr, ERR_STREAM_ALREADY_FINISHED: cw, ERR_STREAM_NULL_VALUES: dw, ERR_STREAM_WRITE_AFTER_END: hw, ERR_UNKNOWN_ENCODING: Ss } = ne().codes, { errorOrDestroy: hr } = hi;
  vs(W.prototype, Wr.prototype);
  vs(W, Wr);
  function la() {
  }
  var pr = rw("kOnFinished");
  function $r(e, t, r) {
    typeof r != "boolean" && (r = t instanceof Ce()), this.objectMode = !!(e && e.objectMode), r && (this.objectMode = this.objectMode || !!(e && e.writableObjectMode)), this.highWaterMark = e ? aw(this, e, "writableHighWaterMark", r) : fw(false), this.finalCalled = false, this.needDrain = false, this.ending = false, this.ended = false, this.finished = false, this.destroyed = false;
    let n = !!(e && e.decodeStrings === false);
    this.decodeStrings = !n, this.defaultEncoding = e && e.defaultEncoding || "utf8", this.length = 0, this.writing = false, this.corked = 0, this.sync = true, this.bufferProcessing = false, this.onwrite = yw.bind(undefined, t), this.writecb = null, this.writelen = 0, this.afterWriteTickInfo = null, di(this), this.pendingcb = 0, this.constructed = true, this.prefinished = false, this.errorEmitted = false, this.emitClose = !e || e.emitClose !== false, this.autoDestroy = !e || e.autoDestroy !== false, this.errored = null, this.closed = false, this.closeEmitted = false, this[pr] = [];
  }
  function di(e) {
    e.buffered = [], e.bufferedIndex = 0, e.allBuffers = true, e.allNoop = true;
  }
  $r.prototype.getBuffer = function() {
    return bs(this.buffered, this.bufferedIndex);
  };
  Es($r.prototype, "bufferedRequestCount", { __proto__: null, get() {
    return this.buffered.length - this.bufferedIndex;
  } });
  function W(e) {
    let t = this instanceof Ce();
    if (!t && !ws(W, this))
      return new W(e);
    this._writableState = new $r(e, this, t), e && (typeof e.write == "function" && (this._write = e.write), typeof e.writev == "function" && (this._writev = e.writev), typeof e.destroy == "function" && (this._destroy = e.destroy), typeof e.final == "function" && (this._final = e.final), typeof e.construct == "function" && (this._construct = e.construct), e.signal && ow(e.signal, this)), Wr.call(this, e), hi.construct(this, () => {
      let r = this._writableState;
      r.writing || sa(this, r), ca(this, r);
    });
  }
  Es(W, nw, { __proto__: null, value: function(e) {
    return ws(this, e) ? true : this !== W ? false : e && e._writableState instanceof $r;
  } });
  W.prototype.pipe = function() {
    hr(this, new sw);
  };
  function As(e, t, r, n) {
    let i = e._writableState;
    if (typeof r == "function")
      n = r, r = i.defaultEncoding;
    else {
      if (!r)
        r = i.defaultEncoding;
      else if (r !== "buffer" && !si.isEncoding(r))
        throw new Ss(r);
      typeof n != "function" && (n = la);
    }
    if (t === null)
      throw new dw;
    if (!i.objectMode)
      if (typeof t == "string")
        i.decodeStrings !== false && (t = si.from(t, r), r = "buffer");
      else if (t instanceof si)
        r = "buffer";
      else if (Wr._isUint8Array(t))
        t = Wr._uint8ArrayToBuffer(t), r = "buffer";
      else
        throw new lw("chunk", ["string", "Buffer", "Uint8Array"], t);
    let o;
    return i.ending ? o = new hw : i.destroyed && (o = new Zr("write")), o ? (kt.nextTick(n, o), hr(e, o, true), o) : (i.pendingcb++, pw(e, i, t, r, n));
  }
  W.prototype.write = function(e, t, r) {
    return As(this, e, t, r) === true;
  };
  W.prototype.cork = function() {
    this._writableState.corked++;
  };
  W.prototype.uncork = function() {
    let e = this._writableState;
    e.corked && (e.corked--, e.writing || sa(this, e));
  };
  W.prototype.setDefaultEncoding = function(t) {
    if (typeof t == "string" && (t = tw(t)), !si.isEncoding(t))
      throw new Ss(t);
    return this._writableState.defaultEncoding = t, this;
  };
  function pw(e, t, r, n, i) {
    let o = t.objectMode ? 1 : r.length;
    t.length += o;
    let a = t.length < t.highWaterMark;
    return a || (t.needDrain = true), t.writing || t.corked || t.errored || !t.constructed ? (t.buffered.push({ chunk: r, encoding: n, callback: i }), t.allBuffers && n !== "buffer" && (t.allBuffers = false), t.allNoop && i !== la && (t.allNoop = false)) : (t.writelen = o, t.writecb = i, t.writing = true, t.sync = true, e._write(r, n, t.onwrite), t.sync = false), a && !t.errored && !t.destroyed;
  }
  function _s(e, t, r, n, i, o, a) {
    t.writelen = n, t.writecb = a, t.writing = true, t.sync = true, t.destroyed ? t.onwrite(new Zr("write")) : r ? e._writev(i, t.onwrite) : e._write(i, o, t.onwrite), t.sync = false;
  }
  function gs(e, t, r, n) {
    --t.pendingcb, n(r), ua(t), hr(e, r);
  }
  function yw(e, t) {
    let r = e._writableState, n = r.sync, i = r.writecb;
    if (typeof i != "function") {
      hr(e, new ms);
      return;
    }
    r.writing = false, r.writecb = null, r.length -= r.writelen, r.writelen = 0, t ? (t.stack, r.errored || (r.errored = t), e._readableState && !e._readableState.errored && (e._readableState.errored = t), n ? kt.nextTick(gs, e, r, t, i) : gs(e, r, t, i)) : (r.buffered.length > r.bufferedIndex && sa(e, r), n ? r.afterWriteTickInfo !== null && r.afterWriteTickInfo.cb === i ? r.afterWriteTickInfo.count++ : (r.afterWriteTickInfo = { count: 1, cb: i, stream: e, state: r }, kt.nextTick(_w, r.afterWriteTickInfo)) : xs(e, r, 1, i));
  }
  function _w({ stream: e, state: t, count: r, cb: n }) {
    return t.afterWriteTickInfo = null, xs(e, t, r, n);
  }
  function xs(e, t, r, n) {
    for (!t.ending && !e.destroyed && t.length === 0 && t.needDrain && (t.needDrain = false, e.emit("drain"));r-- > 0; )
      t.pendingcb--, n();
    t.destroyed && ua(t), ca(e, t);
  }
  function ua(e) {
    if (e.writing)
      return;
    for (let i = e.bufferedIndex;i < e.buffered.length; ++i) {
      var t;
      let { chunk: o, callback: a } = e.buffered[i], f = e.objectMode ? 1 : o.length;
      e.length -= f, a((t = e.errored) !== null && t !== undefined ? t : new Zr("write"));
    }
    let r = e[pr].splice(0);
    for (let i = 0;i < r.length; i++) {
      var n;
      r[i]((n = e.errored) !== null && n !== undefined ? n : new Zr("end"));
    }
    di(e);
  }
  function sa(e, t) {
    if (t.corked || t.bufferProcessing || t.destroyed || !t.constructed)
      return;
    let { buffered: r, bufferedIndex: n, objectMode: i } = t, o = r.length - n;
    if (!o)
      return;
    let a = n;
    if (t.bufferProcessing = true, o > 1 && e._writev) {
      t.pendingcb -= o - 1;
      let f = t.allNoop ? la : (l) => {
        for (let s = a;s < r.length; ++s)
          r[s].callback(l);
      }, u = t.allNoop && a === 0 ? r : bs(r, a);
      u.allBuffers = t.allBuffers, _s(e, t, true, t.length, u, "", f), di(t);
    } else {
      do {
        let { chunk: f, encoding: u, callback: l } = r[a];
        r[a++] = null;
        let s = i ? 1 : f.length;
        _s(e, t, false, s, f, u, l);
      } while (a < r.length && !t.writing);
      a === r.length ? di(t) : a > 256 ? (r.splice(0, a), t.bufferedIndex = 0) : t.bufferedIndex = a;
    }
    t.bufferProcessing = false;
  }
  W.prototype._write = function(e, t, r) {
    if (this._writev)
      this._writev([{ chunk: e, encoding: t }], r);
    else
      throw new uw("_write()");
  };
  W.prototype._writev = null;
  W.prototype.end = function(e, t, r) {
    let n = this._writableState;
    typeof e == "function" ? (r = e, e = null, t = null) : typeof t == "function" && (r = t, t = null);
    let i;
    if (e != null) {
      let o = As(this, e, t);
      o instanceof Qb && (i = o);
    }
    return n.corked && (n.corked = 1, this.uncork()), i || (!n.errored && !n.ending ? (n.ending = true, ca(this, n, true), n.ended = true) : n.finished ? i = new cw("end") : n.destroyed && (i = new Zr("end"))), typeof r == "function" && (i || n.finished ? kt.nextTick(r, i) : n[pr].push(r)), this;
  };
  function ci(e) {
    return e.ending && !e.destroyed && e.constructed && e.length === 0 && !e.errored && e.buffered.length === 0 && !e.finished && !e.writing && !e.errorEmitted && !e.closeEmitted;
  }
  function gw(e, t) {
    let r = false;
    function n(i) {
      if (r) {
        hr(e, i ?? ms());
        return;
      }
      if (r = true, t.pendingcb--, i) {
        let o = t[pr].splice(0);
        for (let a = 0;a < o.length; a++)
          o[a](i);
        hr(e, i, t.sync);
      } else
        ci(t) && (t.prefinished = true, e.emit("prefinish"), t.pendingcb++, kt.nextTick(fa, e, t));
    }
    t.sync = true, t.pendingcb++;
    try {
      e._final(n);
    } catch (i) {
      n(i);
    }
    t.sync = false;
  }
  function bw(e, t) {
    !t.prefinished && !t.finalCalled && (typeof e._final == "function" && !t.destroyed ? (t.finalCalled = true, gw(e, t)) : (t.prefinished = true, e.emit("prefinish")));
  }
  function ca(e, t, r) {
    ci(t) && (bw(e, t), t.pendingcb === 0 && (r ? (t.pendingcb++, kt.nextTick((n, i) => {
      ci(i) ? fa(n, i) : i.pendingcb--;
    }, e, t)) : ci(t) && (t.pendingcb++, fa(e, t))));
  }
  function fa(e, t) {
    t.pendingcb--, t.finished = true;
    let r = t[pr].splice(0);
    for (let n = 0;n < r.length; n++)
      r[n]();
    if (e.emit("finish"), t.autoDestroy) {
      let n = e._readableState;
      (!n || n.autoDestroy && (n.endEmitted || n.readable === false)) && e.destroy();
    }
  }
  ew(W.prototype, { closed: { __proto__: null, get() {
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
  var ww = hi.destroy;
  W.prototype.destroy = function(e, t) {
    let r = this._writableState;
    return !r.destroyed && (r.bufferedIndex < r.buffered.length || r[pr].length) && kt.nextTick(ua, r), ww.call(this, e, t), this;
  };
  W.prototype._undestroy = hi.undestroy;
  W.prototype._destroy = function(e, t) {
    t(e);
  };
  W.prototype[iw.captureRejectionSymbol] = function(e) {
    this.destroy(e);
  };
  var aa;
  function Rs() {
    return aa === undefined && (aa = {}), aa;
  }
  W.fromWeb = function(e, t) {
    return Rs().newStreamWritableFromWritableStream(e, t);
  };
  W.toWeb = function(e) {
    return Rs().newWritableStreamFromStreamWritable(e);
  };
});
var Us = g((HA, js) => {
  var ha = (it(), se(ye)), Ew = xe(), { isReadable: vw, isWritable: mw, isIterable: Ts, isNodeStream: Sw, isReadableNodeStream: Os, isWritableNodeStream: Ns, isDuplexNodeStream: Aw } = ot(), ks = at(), { AbortError: Ms, codes: { ERR_INVALID_ARG_TYPE: xw, ERR_INVALID_RETURN_VALUE: Fs } } = ne(), { destroyer: yr } = Tt(), Rw = Ce(), Iw = zr(), { createDeferredPromise: Ls } = Pe(), Ds = Qo(), Bs = globalThis.Blob || Ew.Blob, Tw = typeof Bs < "u" ? function(t) {
    return t instanceof Bs;
  } : function(t) {
    return false;
  }, Ow = globalThis.AbortController || qn().AbortController, { FunctionPrototypeCall: Ps } = V(), Ft = class extends Rw {
    constructor(t) {
      super(t), t?.readable === false && (this._readableState.readable = false, this._readableState.ended = true, this._readableState.endEmitted = true), t?.writable === false && (this._writableState.writable = false, this._writableState.ending = true, this._writableState.ended = true, this._writableState.finished = true);
    }
  };
  js.exports = function e(t, r) {
    if (Aw(t))
      return t;
    if (Os(t))
      return pi({ readable: t });
    if (Ns(t))
      return pi({ writable: t });
    if (Sw(t))
      return pi({ writable: false, readable: false });
    if (typeof t == "function") {
      let { value: i, write: o, final: a, destroy: f } = Nw(t);
      if (Ts(i))
        return Ds(Ft, i, { objectMode: true, write: o, final: a, destroy: f });
      let u = i?.then;
      if (typeof u == "function") {
        let l, s = Ps(u, i, (c) => {
          if (c != null)
            throw new Fs("nully", "body", c);
        }, (c) => {
          yr(l, c);
        });
        return l = new Ft({ objectMode: true, readable: false, write: o, final(c) {
          a(async () => {
            try {
              await s, ha.nextTick(c, null);
            } catch (h) {
              ha.nextTick(c, h);
            }
          });
        }, destroy: f });
      }
      throw new Fs("Iterable, AsyncIterable or AsyncFunction", r, i);
    }
    if (Tw(t))
      return e(t.arrayBuffer());
    if (Ts(t))
      return Ds(Ft, t, { objectMode: true, writable: false });
    if (typeof t?.writable == "object" || typeof t?.readable == "object") {
      let i = t != null && t.readable ? Os(t?.readable) ? t?.readable : e(t.readable) : undefined, o = t != null && t.writable ? Ns(t?.writable) ? t?.writable : e(t.writable) : undefined;
      return pi({ readable: i, writable: o });
    }
    let n = t?.then;
    if (typeof n == "function") {
      let i;
      return Ps(n, t, (o) => {
        o != null && i.push(o), i.push(null);
      }, (o) => {
        yr(i, o);
      }), i = new Ft({ objectMode: true, writable: false, read() {
      } });
    }
    throw new xw(r, ["Blob", "ReadableStream", "WritableStream", "Stream", "Iterable", "AsyncIterable", "Function", "{ readable, writable } pair", "Promise"], t);
  };
  function Nw(e) {
    let { promise: t, resolve: r } = Ls(), n = new Ow, i = n.signal;
    return { value: e(async function* () {
      for (;; ) {
        let a = t;
        t = null;
        let { chunk: f, done: u, cb: l } = await a;
        if (ha.nextTick(l), u)
          return;
        if (i.aborted)
          throw new Ms(undefined, { cause: i.reason });
        ({ promise: t, resolve: r } = Ls()), yield f;
      }
    }(), { signal: i }), write(a, f, u) {
      let l = r;
      r = null, l({ chunk: a, done: false, cb: u });
    }, final(a) {
      let f = r;
      r = null, f({ done: true, cb: a });
    }, destroy(a, f) {
      n.abort(), f(a);
    } };
  }
  function pi(e) {
    let t = e.readable && typeof e.readable.read != "function" ? Iw.wrap(e.readable) : e.readable, r = e.writable, n = !!vw(t), i = !!mw(r), o, a, f, u, l;
    function s(c) {
      let h = u;
      u = null, h ? h(c) : c ? l.destroy(c) : !n && !i && l.destroy();
    }
    return l = new Ft({ readableObjectMode: !!(t != null && t.readableObjectMode), writableObjectMode: !!(r != null && r.writableObjectMode), readable: n, writable: i }), i && (ks(r, (c) => {
      i = false, c && yr(t, c), s(c);
    }), l._write = function(c, h, d) {
      r.write(c, h) ? d() : o = d;
    }, l._final = function(c) {
      r.end(), a = c;
    }, r.on("drain", function() {
      if (o) {
        let c = o;
        o = null, c();
      }
    }), r.on("finish", function() {
      if (a) {
        let c = a;
        a = null, c();
      }
    })), n && (ks(t, (c) => {
      n = false, c && yr(t, c), s(c);
    }), t.on("readable", function() {
      if (f) {
        let c = f;
        f = null, c();
      }
    }), t.on("end", function() {
      l.push(null);
    }), l._read = function() {
      for (;; ) {
        let c = t.read();
        if (c === null) {
          f = l._read;
          return;
        }
        if (!l.push(c))
          return;
      }
    }), l._destroy = function(c, h) {
      !c && u !== null && (c = new Ms), f = null, o = null, a = null, u === null ? h(c) : (u = h, yr(r, c), yr(t, c));
    }, l;
  }
});
var Ce = g((VA, zs) => {
  var { ObjectDefineProperties: kw, ObjectGetOwnPropertyDescriptor: Je, ObjectKeys: Fw, ObjectSetPrototypeOf: qs } = V();
  zs.exports = Te;
  var _a = zr(), _e = da();
  qs(Te.prototype, _a.prototype);
  qs(Te, _a);
  {
    let e = Fw(_e.prototype);
    for (let t = 0;t < e.length; t++) {
      let r = e[t];
      Te.prototype[r] || (Te.prototype[r] = _e.prototype[r]);
    }
  }
  function Te(e) {
    if (!(this instanceof Te))
      return new Te(e);
    _a.call(this, e), _e.call(this, e), e ? (this.allowHalfOpen = e.allowHalfOpen !== false, e.readable === false && (this._readableState.readable = false, this._readableState.ended = true, this._readableState.endEmitted = true), e.writable === false && (this._writableState.writable = false, this._writableState.ending = true, this._writableState.ended = true, this._writableState.finished = true)) : this.allowHalfOpen = true;
  }
  kw(Te.prototype, { writable: { __proto__: null, ...Je(_e.prototype, "writable") }, writableHighWaterMark: { __proto__: null, ...Je(_e.prototype, "writableHighWaterMark") }, writableObjectMode: { __proto__: null, ...Je(_e.prototype, "writableObjectMode") }, writableBuffer: { __proto__: null, ...Je(_e.prototype, "writableBuffer") }, writableLength: { __proto__: null, ...Je(_e.prototype, "writableLength") }, writableFinished: { __proto__: null, ...Je(_e.prototype, "writableFinished") }, writableCorked: { __proto__: null, ...Je(_e.prototype, "writableCorked") }, writableEnded: { __proto__: null, ...Je(_e.prototype, "writableEnded") }, writableNeedDrain: { __proto__: null, ...Je(_e.prototype, "writableNeedDrain") }, destroyed: { __proto__: null, get() {
    return this._readableState === undefined || this._writableState === undefined ? false : this._readableState.destroyed && this._writableState.destroyed;
  }, set(e) {
    this._readableState && this._writableState && (this._readableState.destroyed = e, this._writableState.destroyed = e);
  } } });
  var pa;
  function Cs() {
    return pa === undefined && (pa = {}), pa;
  }
  Te.fromWeb = function(e, t) {
    return Cs().newStreamDuplexFromReadableWritablePair(e, t);
  };
  Te.toWeb = function(e) {
    return Cs().newReadableWritablePairFromDuplex(e);
  };
  var ya;
  Te.from = function(e) {
    return ya || (ya = Us()), ya(e, "body");
  };
});
var wa = g((YA, Zs) => {
  var { ObjectSetPrototypeOf: Ws, Symbol: Lw } = V();
  Zs.exports = Qe;
  var { ERR_METHOD_NOT_IMPLEMENTED: Dw } = ne().codes, ba = Ce(), { getHighWaterMark: Bw } = ai();
  Ws(Qe.prototype, ba.prototype);
  Ws(Qe, ba);
  var Gr = Lw("kCallback");
  function Qe(e) {
    if (!(this instanceof Qe))
      return new Qe(e);
    let t = e ? Bw(this, e, "readableHighWaterMark", true) : null;
    t === 0 && (e = { ...e, highWaterMark: null, readableHighWaterMark: t, writableHighWaterMark: e.writableHighWaterMark || 0 }), ba.call(this, e), this._readableState.sync = false, this[Gr] = null, e && (typeof e.transform == "function" && (this._transform = e.transform), typeof e.flush == "function" && (this._flush = e.flush)), this.on("prefinish", Pw);
  }
  function ga(e) {
    typeof this._flush == "function" && !this.destroyed ? this._flush((t, r) => {
      if (t) {
        e ? e(t) : this.destroy(t);
        return;
      }
      r != null && this.push(r), this.push(null), e && e();
    }) : (this.push(null), e && e());
  }
  function Pw() {
    this._final !== ga && ga.call(this);
  }
  Qe.prototype._final = ga;
  Qe.prototype._transform = function(e, t, r) {
    throw new Dw("_transform()");
  };
  Qe.prototype._write = function(e, t, r) {
    let n = this._readableState, i = this._writableState, o = n.length;
    this._transform(e, t, (a, f) => {
      if (a) {
        r(a);
        return;
      }
      f != null && this.push(f), i.ended || o === n.length || n.length < n.highWaterMark ? r() : this[Gr] = r;
    });
  };
  Qe.prototype._read = function() {
    if (this[Gr]) {
      let e = this[Gr];
      this[Gr] = null, e();
    }
  };
});
var va = g((KA, Gs) => {
  var { ObjectSetPrototypeOf: $s } = V();
  Gs.exports = _r;
  var Ea = wa();
  $s(_r.prototype, Ea.prototype);
  $s(_r, Ea);
  function _r(e) {
    if (!(this instanceof _r))
      return new _r(e);
    Ea.call(this, e);
  }
  _r.prototype._transform = function(e, t, r) {
    r(null, e);
  };
});
var gi = g((XA, Qs) => {
  var yi = (it(), se(ye)), { ArrayIsArray: Mw, Promise: jw, SymbolAsyncIterator: Uw } = V(), _i = at(), { once: qw } = Pe(), Cw = Tt(), Hs = Ce(), { aggregateTwoErrors: zw, codes: { ERR_INVALID_ARG_TYPE: Xs, ERR_INVALID_RETURN_VALUE: ma, ERR_MISSING_ARGS: Ww, ERR_STREAM_DESTROYED: Zw, ERR_STREAM_PREMATURE_CLOSE: $w }, AbortError: Gw } = ne(), { validateFunction: Hw, validateAbortSignal: Vw } = Ur(), { isIterable: gr, isReadable: Sa, isReadableNodeStream: Ra, isNodeStream: Vs } = ot(), Yw = globalThis.AbortController || qn().AbortController, Aa, xa;
  function Ys(e, t, r) {
    let n = false;
    e.on("close", () => {
      n = true;
    });
    let i = _i(e, { readable: t, writable: r }, (o) => {
      n = !o;
    });
    return { destroy: (o) => {
      n || (n = true, Cw.destroyer(e, o || new Zw("pipe")));
    }, cleanup: i };
  }
  function Kw(e) {
    return Hw(e[e.length - 1], "streams[stream.length - 1]"), e.pop();
  }
  function Xw(e) {
    if (gr(e))
      return e;
    if (Ra(e))
      return Jw(e);
    throw new Xs("val", ["Readable", "Iterable", "AsyncIterable"], e);
  }
  async function* Jw(e) {
    xa || (xa = zr()), yield* xa.prototype[Uw].call(e);
  }
  async function Ks(e, t, r, { end: n }) {
    let i, o = null, a = (l) => {
      if (l && (i = l), o) {
        let s = o;
        o = null, s();
      }
    }, f = () => new jw((l, s) => {
      i ? s(i) : o = () => {
        i ? s(i) : l();
      };
    });
    t.on("drain", a);
    let u = _i(t, { readable: false }, a);
    try {
      t.writableNeedDrain && await f();
      for await (let l of e)
        t.write(l) || await f();
      n && t.end(), await f(), r();
    } catch (l) {
      r(i !== l ? zw(i, l) : l);
    } finally {
      u(), t.off("drain", a);
    }
  }
  function Qw(...e) {
    return Js(e, qw(Kw(e)));
  }
  function Js(e, t, r) {
    if (e.length === 1 && Mw(e[0]) && (e = e[0]), e.length < 2)
      throw new Ww("streams");
    let n = new Yw, i = n.signal, o = r?.signal, a = [];
    Vw(o, "options.signal");
    function f() {
      d(new Gw);
    }
    o?.addEventListener("abort", f);
    let u, l, s = [], c = 0;
    function h(_) {
      d(_, --c === 0);
    }
    function d(_, E) {
      if (_ && (!u || u.code === "ERR_STREAM_PREMATURE_CLOSE") && (u = _), !(!u && !E)) {
        for (;s.length; )
          s.shift()(u);
        o?.removeEventListener("abort", f), n.abort(), E && (u || a.forEach((m) => m()), yi.nextTick(t, u, l));
      }
    }
    let y;
    for (let _ = 0;_ < e.length; _++) {
      let E = e[_], m = _ < e.length - 1, A = _ > 0, v = m || r?.end !== false, T = _ === e.length - 1;
      if (Vs(E)) {
        let I = function(S) {
          S && S.name !== "AbortError" && S.code !== "ERR_STREAM_PREMATURE_CLOSE" && h(S);
        };
        var R = I;
        if (v) {
          let { destroy: S, cleanup: k } = Ys(E, m, A);
          s.push(S), Sa(E) && T && a.push(k);
        }
        E.on("error", I), Sa(E) && T && a.push(() => {
          E.removeListener("error", I);
        });
      }
      if (_ === 0)
        if (typeof E == "function") {
          if (y = E({ signal: i }), !gr(y))
            throw new ma("Iterable, AsyncIterable or Stream", "source", y);
        } else
          gr(E) || Ra(E) ? y = E : y = Hs.from(E);
      else if (typeof E == "function")
        if (y = Xw(y), y = E(y, { signal: i }), m) {
          if (!gr(y, true))
            throw new ma("AsyncIterable", `transform[${_ - 1}]`, y);
        } else {
          var b;
          Aa || (Aa = va());
          let I = new Aa({ objectMode: true }), S = (b = y) === null || b === undefined ? undefined : b.then;
          if (typeof S == "function")
            c++, S.call(y, (O) => {
              l = O, O != null && I.write(O), v && I.end(), yi.nextTick(h);
            }, (O) => {
              I.destroy(O), yi.nextTick(h, O);
            });
          else if (gr(y, true))
            c++, Ks(y, I, h, { end: v });
          else
            throw new ma("AsyncIterable or Promise", "destination", y);
          y = I;
          let { destroy: k, cleanup: z } = Ys(y, false, true);
          s.push(k), T && a.push(z);
        }
      else if (Vs(E)) {
        if (Ra(y)) {
          c += 2;
          let I = eE(y, E, h, { end: v });
          Sa(E) && T && a.push(I);
        } else if (gr(y))
          c++, Ks(y, E, h, { end: v });
        else
          throw new Xs("val", ["Readable", "Iterable", "AsyncIterable"], y);
        y = E;
      } else
        y = Hs.from(E);
    }
    return (i != null && i.aborted || o != null && o.aborted) && yi.nextTick(f), y;
  }
  function eE(e, t, r, { end: n }) {
    let i = false;
    return t.on("close", () => {
      i || r(new $w);
    }), e.pipe(t, { end: n }), n ? e.once("end", () => {
      i = true, t.end();
    }) : r(), _i(e, { readable: true, writable: false }, (o) => {
      let a = e._readableState;
      o && o.code === "ERR_STREAM_PREMATURE_CLOSE" && a && a.ended && !a.errored && !a.errorEmitted ? e.once("end", r).once("error", r) : r(o);
    }), _i(t, { readable: false, writable: true }, r);
  }
  Qs.exports = { pipelineImpl: Js, pipeline: Qw };
});
var ic = g((JA, nc) => {
  var { pipeline: tE } = gi(), bi = Ce(), { destroyer: rE } = Tt(), { isNodeStream: nE, isReadable: ec, isWritable: tc } = ot(), { AbortError: iE, codes: { ERR_INVALID_ARG_VALUE: rc, ERR_MISSING_ARGS: oE } } = ne();
  nc.exports = function(...t) {
    if (t.length === 0)
      throw new oE("streams");
    if (t.length === 1)
      return bi.from(t[0]);
    let r = [...t];
    if (typeof t[0] == "function" && (t[0] = bi.from(t[0])), typeof t[t.length - 1] == "function") {
      let d = t.length - 1;
      t[d] = bi.from(t[d]);
    }
    for (let d = 0;d < t.length; ++d)
      if (!!nE(t[d])) {
        if (d < t.length - 1 && !ec(t[d]))
          throw new rc(`streams[${d}]`, r[d], "must be readable");
        if (d > 0 && !tc(t[d]))
          throw new rc(`streams[${d}]`, r[d], "must be writable");
      }
    let n, i, o, a, f;
    function u(d) {
      let y = a;
      a = null, y ? y(d) : d ? f.destroy(d) : !h && !c && f.destroy();
    }
    let l = t[0], s = tE(t, u), c = !!tc(l), h = !!ec(s);
    return f = new bi({ writableObjectMode: !!(l != null && l.writableObjectMode), readableObjectMode: !!(s != null && s.writableObjectMode), writable: c, readable: h }), c && (f._write = function(d, y, b) {
      l.write(d, y) ? b() : n = b;
    }, f._final = function(d) {
      l.end(), i = d;
    }, l.on("drain", function() {
      if (n) {
        let d = n;
        n = null, d();
      }
    }), s.on("finish", function() {
      if (i) {
        let d = i;
        i = null, d();
      }
    })), h && (s.on("readable", function() {
      if (o) {
        let d = o;
        o = null, d();
      }
    }), s.on("end", function() {
      f.push(null);
    }), f._read = function() {
      for (;; ) {
        let d = s.read();
        if (d === null) {
          o = f._read;
          return;
        }
        if (!f.push(d))
          return;
      }
    }), f._destroy = function(d, y) {
      !d && a !== null && (d = new iE), o = null, n = null, i = null, a === null ? y(d) : (a = y, rE(s, d));
    }, f;
  };
});
var Ia = g((QA, oc) => {
  var { ArrayPrototypePop: aE, Promise: fE } = V(), { isIterable: lE, isNodeStream: uE } = ot(), { pipelineImpl: sE } = gi(), { finished: cE } = at();
  function dE(...e) {
    return new fE((t, r) => {
      let n, i, o = e[e.length - 1];
      if (o && typeof o == "object" && !uE(o) && !lE(o)) {
        let a = aE(e);
        n = a.signal, i = a.end;
      }
      sE(e, (a, f) => {
        a ? r(a) : t(f);
      }, { signal: n, end: i });
    });
  }
  oc.exports = { finished: cE, pipeline: dE };
});
var yc = g((ex, pc) => {
  var { Buffer: hE } = xe(), { ObjectDefineProperty: et, ObjectKeys: lc, ReflectApply: uc } = V(), { promisify: { custom: sc } } = Pe(), { streamReturningOperators: ac, promiseReturningOperators: fc } = Eu(), { codes: { ERR_ILLEGAL_CONSTRUCTOR: cc } } = ne(), pE = ic(), { pipeline: dc } = gi(), { destroyer: yE } = Tt(), hc = at(), Ta = Ia(), Oa = ot(), G = pc.exports = ri().Stream;
  G.isDisturbed = Oa.isDisturbed;
  G.isErrored = Oa.isErrored;
  G.isReadable = Oa.isReadable;
  G.Readable = zr();
  for (let e of lc(ac)) {
    let r = function(...n) {
      if (new.target)
        throw cc();
      return G.Readable.from(uc(t, this, n));
    };
    gE = r;
    let t = ac[e];
    et(r, "name", { __proto__: null, value: t.name }), et(r, "length", { __proto__: null, value: t.length }), et(G.Readable.prototype, e, { __proto__: null, value: r, enumerable: false, configurable: true, writable: true });
  }
  var gE;
  for (let e of lc(fc)) {
    let r = function(...i) {
      if (new.target)
        throw cc();
      return uc(t, this, i);
    };
    gE = r;
    let t = fc[e];
    et(r, "name", { __proto__: null, value: t.name }), et(r, "length", { __proto__: null, value: t.length }), et(G.Readable.prototype, e, { __proto__: null, value: r, enumerable: false, configurable: true, writable: true });
  }
  var gE;
  G.Writable = da();
  G.Duplex = Ce();
  G.Transform = wa();
  G.PassThrough = va();
  G.pipeline = dc;
  var { addAbortSignal: _E } = ii();
  G.addAbortSignal = _E;
  G.finished = hc;
  G.destroy = yE;
  G.compose = pE;
  et(G, "promises", { __proto__: null, configurable: true, enumerable: true, get() {
    return Ta;
  } });
  et(dc, sc, { __proto__: null, enumerable: true, get() {
    return Ta.pipeline;
  } });
  et(hc, sc, { __proto__: null, enumerable: true, get() {
    return Ta.finished;
  } });
  G.Stream = G;
  G._isUint8Array = function(t) {
    return t instanceof Uint8Array;
  };
  G._uint8ArrayToBuffer = function(t) {
    return hE.from(t.buffer, t.byteOffset, t.byteLength);
  };
});
var Na = g((tx, H) => {
  var Y = yc(), bE = Ia(), wE = Y.Readable.destroy;
  H.exports = Y.Readable;
  H.exports._uint8ArrayToBuffer = Y._uint8ArrayToBuffer;
  H.exports._isUint8Array = Y._isUint8Array;
  H.exports.isDisturbed = Y.isDisturbed;
  H.exports.isErrored = Y.isErrored;
  H.exports.isReadable = Y.isReadable;
  H.exports.Readable = Y.Readable;
  H.exports.Writable = Y.Writable;
  H.exports.Duplex = Y.Duplex;
  H.exports.Transform = Y.Transform;
  H.exports.PassThrough = Y.PassThrough;
  H.exports.addAbortSignal = Y.addAbortSignal;
  H.exports.finished = Y.finished;
  H.exports.destroy = Y.destroy;
  H.exports.destroy = wE;
  H.exports.pipeline = Y.pipeline;
  H.exports.compose = Y.compose;
  Object.defineProperty(Y, "promises", { configurable: true, enumerable: true, get() {
    return bE;
  } });
  H.exports.Stream = Y.Stream;
  H.exports.default = H.exports;
});
var br = {};
Bn(br, { default: () => EE });
var EE;
var _c = wo(() => {
  X(br, vt(Na()));
  EE = vt(Na());
});
var ka = g((nx, gc) => {
  gc.exports = function() {
    if (typeof Symbol != "function" || typeof Object.getOwnPropertySymbols != "function")
      return false;
    if (typeof Symbol.iterator == "symbol")
      return true;
    var t = {}, r = Symbol("test"), n = Object(r);
    if (typeof r == "string" || Object.prototype.toString.call(r) !== "[object Symbol]" || Object.prototype.toString.call(n) !== "[object Symbol]")
      return false;
    var i = 42;
    t[r] = i;
    for (r in t)
      return false;
    if (typeof Object.keys == "function" && Object.keys(t).length !== 0 || typeof Object.getOwnPropertyNames == "function" && Object.getOwnPropertyNames(t).length !== 0)
      return false;
    var o = Object.getOwnPropertySymbols(t);
    if (o.length !== 1 || o[0] !== r || !Object.prototype.propertyIsEnumerable.call(t, r))
      return false;
    if (typeof Object.getOwnPropertyDescriptor == "function") {
      var a = Object.getOwnPropertyDescriptor(t, r);
      if (a.value !== i || a.enumerable !== true)
        return false;
    }
    return true;
  };
});
var Hr = g((ix, bc) => {
  var vE = ka();
  bc.exports = function() {
    return vE() && !!Symbol.toStringTag;
  };
});
var vc = g((ox, Ec) => {
  var wc = typeof Symbol < "u" && Symbol, mE = ka();
  Ec.exports = function() {
    return typeof wc != "function" || typeof Symbol != "function" || typeof wc("foo") != "symbol" || typeof Symbol("bar") != "symbol" ? false : mE();
  };
});
var Sc = g((ax, mc) => {
  var SE = "Function.prototype.bind called on incompatible ", Fa = Array.prototype.slice, AE = Object.prototype.toString, xE = "[object Function]";
  mc.exports = function(t) {
    var r = this;
    if (typeof r != "function" || AE.call(r) !== xE)
      throw new TypeError(SE + r);
    for (var n = Fa.call(arguments, 1), i, o = function() {
      if (this instanceof i) {
        var s = r.apply(this, n.concat(Fa.call(arguments)));
        return Object(s) === s ? s : this;
      } else
        return r.apply(t, n.concat(Fa.call(arguments)));
    }, a = Math.max(0, r.length - n.length), f = [], u = 0;u < a; u++)
      f.push("$" + u);
    if (i = Function("binder", "return function (" + f.join(",") + "){ return binder.apply(this,arguments); }")(o), r.prototype) {
      var l = function() {
      };
      l.prototype = r.prototype, i.prototype = new l, l.prototype = null;
    }
    return i;
  };
});
var wi = g((fx, Ac) => {
  var RE = Sc();
  Ac.exports = Function.prototype.bind || RE;
});
var Rc = g((lx, xc) => {
  var IE = wi();
  xc.exports = IE.call(Function.call, Object.prototype.hasOwnProperty);
});
var Yr = g((ux, kc) => {
  var P, mr = SyntaxError, Nc = Function, vr = TypeError, La = function(e) {
    try {
      return Nc('"use strict"; return (' + e + ").constructor;")();
    } catch {
    }
  }, Lt = Object.getOwnPropertyDescriptor;
  if (Lt)
    try {
      Lt({}, "");
    } catch {
      Lt = null;
    }
  var Da = function() {
    throw new vr;
  }, TE = Lt ? function() {
    try {
      return arguments.callee, Da;
    } catch {
      try {
        return Lt(arguments, "callee").get;
      } catch {
        return Da;
      }
    }
  }() : Da, wr = vc()(), ze = Object.getPrototypeOf || function(e) {
    return e.__proto__;
  }, Er = {}, OE = typeof Uint8Array > "u" ? P : ze(Uint8Array), Dt = { "%AggregateError%": typeof AggregateError > "u" ? P : AggregateError, "%Array%": Array, "%ArrayBuffer%": typeof ArrayBuffer > "u" ? P : ArrayBuffer, "%ArrayIteratorPrototype%": wr ? ze([][Symbol.iterator]()) : P, "%AsyncFromSyncIteratorPrototype%": P, "%AsyncFunction%": Er, "%AsyncGenerator%": Er, "%AsyncGeneratorFunction%": Er, "%AsyncIteratorPrototype%": Er, "%Atomics%": typeof Atomics > "u" ? P : Atomics, "%BigInt%": typeof BigInt > "u" ? P : BigInt, "%BigInt64Array%": typeof BigInt64Array > "u" ? P : BigInt64Array, "%BigUint64Array%": typeof BigUint64Array > "u" ? P : BigUint64Array, "%Boolean%": Boolean, "%DataView%": typeof DataView > "u" ? P : DataView, "%Date%": Date, "%decodeURI%": decodeURI, "%decodeURIComponent%": decodeURIComponent, "%encodeURI%": encodeURI, "%encodeURIComponent%": encodeURIComponent, "%Error%": Error, "%eval%": eval, "%EvalError%": EvalError, "%Float32Array%": typeof Float32Array > "u" ? P : Float32Array, "%Float64Array%": typeof Float64Array > "u" ? P : Float64Array, "%FinalizationRegistry%": typeof FinalizationRegistry > "u" ? P : FinalizationRegistry, "%Function%": Nc, "%GeneratorFunction%": Er, "%Int8Array%": typeof Int8Array > "u" ? P : Int8Array, "%Int16Array%": typeof Int16Array > "u" ? P : Int16Array, "%Int32Array%": typeof Int32Array > "u" ? P : Int32Array, "%isFinite%": isFinite, "%isNaN%": isNaN, "%IteratorPrototype%": wr ? ze(ze([][Symbol.iterator]())) : P, "%JSON%": typeof JSON == "object" ? JSON : P, "%Map%": typeof Map > "u" ? P : Map, "%MapIteratorPrototype%": typeof Map > "u" || !wr ? P : ze(new Map()[Symbol.iterator]()), "%Math%": Math, "%Number%": Number, "%Object%": Object, "%parseFloat%": parseFloat, "%parseInt%": parseInt, "%Promise%": typeof Promise > "u" ? P : Promise, "%Proxy%": typeof Proxy > "u" ? P : Proxy, "%RangeError%": RangeError, "%ReferenceError%": ReferenceError, "%Reflect%": typeof Reflect > "u" ? P : Reflect, "%RegExp%": RegExp, "%Set%": typeof Set > "u" ? P : Set, "%SetIteratorPrototype%": typeof Set > "u" || !wr ? P : ze(new Set()[Symbol.iterator]()), "%SharedArrayBuffer%": typeof SharedArrayBuffer > "u" ? P : SharedArrayBuffer, "%String%": String, "%StringIteratorPrototype%": wr ? ze(""[Symbol.iterator]()) : P, "%Symbol%": wr ? Symbol : P, "%SyntaxError%": mr, "%ThrowTypeError%": TE, "%TypedArray%": OE, "%TypeError%": vr, "%Uint8Array%": typeof Uint8Array > "u" ? P : Uint8Array, "%Uint8ClampedArray%": typeof Uint8ClampedArray > "u" ? P : Uint8ClampedArray, "%Uint16Array%": typeof Uint16Array > "u" ? P : Uint16Array, "%Uint32Array%": typeof Uint32Array > "u" ? P : Uint32Array, "%URIError%": URIError, "%WeakMap%": typeof WeakMap > "u" ? P : WeakMap, "%WeakRef%": typeof WeakRef > "u" ? P : WeakRef, "%WeakSet%": typeof WeakSet > "u" ? P : WeakSet };
  try {
    null.error;
  } catch (e) {
    Ic = ze(ze(e)), Dt["%Error.prototype%"] = Ic;
  }
  var Ic, NE = function e(t) {
    var r;
    if (t === "%AsyncFunction%")
      r = La("async function () {}");
    else if (t === "%GeneratorFunction%")
      r = La("function* () {}");
    else if (t === "%AsyncGeneratorFunction%")
      r = La("async function* () {}");
    else if (t === "%AsyncGenerator%") {
      var n = e("%AsyncGeneratorFunction%");
      n && (r = n.prototype);
    } else if (t === "%AsyncIteratorPrototype%") {
      var i = e("%AsyncGenerator%");
      i && (r = ze(i.prototype));
    }
    return Dt[t] = r, r;
  }, Tc = { "%ArrayBufferPrototype%": ["ArrayBuffer", "prototype"], "%ArrayPrototype%": ["Array", "prototype"], "%ArrayProto_entries%": ["Array", "prototype", "entries"], "%ArrayProto_forEach%": ["Array", "prototype", "forEach"], "%ArrayProto_keys%": ["Array", "prototype", "keys"], "%ArrayProto_values%": ["Array", "prototype", "values"], "%AsyncFunctionPrototype%": ["AsyncFunction", "prototype"], "%AsyncGenerator%": ["AsyncGeneratorFunction", "prototype"], "%AsyncGeneratorPrototype%": ["AsyncGeneratorFunction", "prototype", "prototype"], "%BooleanPrototype%": ["Boolean", "prototype"], "%DataViewPrototype%": ["DataView", "prototype"], "%DatePrototype%": ["Date", "prototype"], "%ErrorPrototype%": ["Error", "prototype"], "%EvalErrorPrototype%": ["EvalError", "prototype"], "%Float32ArrayPrototype%": ["Float32Array", "prototype"], "%Float64ArrayPrototype%": ["Float64Array", "prototype"], "%FunctionPrototype%": ["Function", "prototype"], "%Generator%": ["GeneratorFunction", "prototype"], "%GeneratorPrototype%": ["GeneratorFunction", "prototype", "prototype"], "%Int8ArrayPrototype%": ["Int8Array", "prototype"], "%Int16ArrayPrototype%": ["Int16Array", "prototype"], "%Int32ArrayPrototype%": ["Int32Array", "prototype"], "%JSONParse%": ["JSON", "parse"], "%JSONStringify%": ["JSON", "stringify"], "%MapPrototype%": ["Map", "prototype"], "%NumberPrototype%": ["Number", "prototype"], "%ObjectPrototype%": ["Object", "prototype"], "%ObjProto_toString%": ["Object", "prototype", "toString"], "%ObjProto_valueOf%": ["Object", "prototype", "valueOf"], "%PromisePrototype%": ["Promise", "prototype"], "%PromiseProto_then%": ["Promise", "prototype", "then"], "%Promise_all%": ["Promise", "all"], "%Promise_reject%": ["Promise", "reject"], "%Promise_resolve%": ["Promise", "resolve"], "%RangeErrorPrototype%": ["RangeError", "prototype"], "%ReferenceErrorPrototype%": ["ReferenceError", "prototype"], "%RegExpPrototype%": ["RegExp", "prototype"], "%SetPrototype%": ["Set", "prototype"], "%SharedArrayBufferPrototype%": ["SharedArrayBuffer", "prototype"], "%StringPrototype%": ["String", "prototype"], "%SymbolPrototype%": ["Symbol", "prototype"], "%SyntaxErrorPrototype%": ["SyntaxError", "prototype"], "%TypedArrayPrototype%": ["TypedArray", "prototype"], "%TypeErrorPrototype%": ["TypeError", "prototype"], "%Uint8ArrayPrototype%": ["Uint8Array", "prototype"], "%Uint8ClampedArrayPrototype%": ["Uint8ClampedArray", "prototype"], "%Uint16ArrayPrototype%": ["Uint16Array", "prototype"], "%Uint32ArrayPrototype%": ["Uint32Array", "prototype"], "%URIErrorPrototype%": ["URIError", "prototype"], "%WeakMapPrototype%": ["WeakMap", "prototype"], "%WeakSetPrototype%": ["WeakSet", "prototype"] }, Vr = wi(), Ei = Rc(), kE = Vr.call(Function.call, Array.prototype.concat), FE = Vr.call(Function.apply, Array.prototype.splice), Oc = Vr.call(Function.call, String.prototype.replace), vi = Vr.call(Function.call, String.prototype.slice), LE = Vr.call(Function.call, RegExp.prototype.exec), DE = /[^%.[\]]+|\[(?:(-?\d+(?:\.\d+)?)|(["'])((?:(?!\2)[^\\]|\\.)*?)\2)\]|(?=(?:\.|\[\])(?:\.|\[\]|%$))/g, BE = /\\(\\)?/g, PE = function(t) {
    var r = vi(t, 0, 1), n = vi(t, -1);
    if (r === "%" && n !== "%")
      throw new mr("invalid intrinsic syntax, expected closing `%`");
    if (n === "%" && r !== "%")
      throw new mr("invalid intrinsic syntax, expected opening `%`");
    var i = [];
    return Oc(t, DE, function(o, a, f, u) {
      i[i.length] = f ? Oc(u, BE, "$1") : a || o;
    }), i;
  }, ME = function(t, r) {
    var n = t, i;
    if (Ei(Tc, n) && (i = Tc[n], n = "%" + i[0] + "%"), Ei(Dt, n)) {
      var o = Dt[n];
      if (o === Er && (o = NE(n)), typeof o > "u" && !r)
        throw new vr("intrinsic " + t + " exists, but is not available. Please file an issue!");
      return { alias: i, name: n, value: o };
    }
    throw new mr("intrinsic " + t + " does not exist!");
  };
  kc.exports = function(t, r) {
    if (typeof t != "string" || t.length === 0)
      throw new vr("intrinsic name must be a non-empty string");
    if (arguments.length > 1 && typeof r != "boolean")
      throw new vr('"allowMissing" argument must be a boolean');
    if (LE(/^%?[^%]*%?$/, t) === null)
      throw new mr("`%` may not be present anywhere but at the beginning and end of the intrinsic name");
    var n = PE(t), i = n.length > 0 ? n[0] : "", o = ME("%" + i + "%", r), a = o.name, f = o.value, u = false, l = o.alias;
    l && (i = l[0], FE(n, kE([0, 1], l)));
    for (var s = 1, c = true;s < n.length; s += 1) {
      var h = n[s], d = vi(h, 0, 1), y = vi(h, -1);
      if ((d === '"' || d === "'" || d === "`" || y === '"' || y === "'" || y === "`") && d !== y)
        throw new mr("property names with quotes must have matching quotes");
      if ((h === "constructor" || !c) && (u = true), i += "." + h, a = "%" + i + "%", Ei(Dt, a))
        f = Dt[a];
      else if (f != null) {
        if (!(h in f)) {
          if (!r)
            throw new vr("base intrinsic for " + t + " exists, but the property is not available.");
          return;
        }
        if (Lt && s + 1 >= n.length) {
          var b = Lt(f, h);
          c = !!b, c && "get" in b && !("originalValue" in b.get) ? f = b.get : f = f[h];
        } else
          c = Ei(f, h), f = f[h];
        c && !u && (Dt[a] = f);
      }
    }
    return f;
  };
});
var Si = g((sx, mi) => {
  var Ba = wi(), Sr = Yr(), Dc = Sr("%Function.prototype.apply%"), Bc = Sr("%Function.prototype.call%"), Pc = Sr("%Reflect.apply%", true) || Ba.call(Bc, Dc), Fc = Sr("%Object.getOwnPropertyDescriptor%", true), Bt = Sr("%Object.defineProperty%", true), jE = Sr("%Math.max%");
  if (Bt)
    try {
      Bt({}, "a", { value: 1 });
    } catch {
      Bt = null;
    }
  mi.exports = function(t) {
    var r = Pc(Ba, Bc, arguments);
    if (Fc && Bt) {
      var n = Fc(r, "length");
      n.configurable && Bt(r, "length", { value: 1 + jE(0, t.length - (arguments.length - 1)) });
    }
    return r;
  };
  var Lc = function() {
    return Pc(Ba, Dc, arguments);
  };
  Bt ? Bt(mi.exports, "apply", { value: Lc }) : mi.exports.apply = Lc;
});
var Ai = g((cx, Uc) => {
  var Mc = Yr(), jc = Si(), UE = jc(Mc("String.prototype.indexOf"));
  Uc.exports = function(t, r) {
    var n = Mc(t, !!r);
    return typeof n == "function" && UE(t, ".prototype.") > -1 ? jc(n) : n;
  };
});
var zc = g((dx, Cc) => {
  var qE = Hr()(), CE = Ai(), Pa = CE("Object.prototype.toString"), xi = function(t) {
    return qE && t && typeof t == "object" && Symbol.toStringTag in t ? false : Pa(t) === "[object Arguments]";
  }, qc = function(t) {
    return xi(t) ? true : t !== null && typeof t == "object" && typeof t.length == "number" && t.length >= 0 && Pa(t) !== "[object Array]" && Pa(t.callee) === "[object Function]";
  }, zE = function() {
    return xi(arguments);
  }();
  xi.isLegacyArguments = qc;
  Cc.exports = zE ? xi : qc;
});
var $c = g((hx, Zc) => {
  var WE = Object.prototype.toString, ZE = Function.prototype.toString, $E = /^\s*(?:function)?\*/, Wc = Hr()(), Ma = Object.getPrototypeOf, GE = function() {
    if (!Wc)
      return false;
    try {
      return Function("return function*() {}")();
    } catch {
    }
  }, ja;
  Zc.exports = function(t) {
    if (typeof t != "function")
      return false;
    if ($E.test(ZE.call(t)))
      return true;
    if (!Wc) {
      var r = WE.call(t);
      return r === "[object GeneratorFunction]";
    }
    if (!Ma)
      return false;
    if (typeof ja > "u") {
      var n = GE();
      ja = n ? Ma(n) : false;
    }
    return Ma(t) === ja;
  };
});
var Yc = g((px, Vc) => {
  var Hc = Function.prototype.toString, Ar = typeof Reflect == "object" && Reflect !== null && Reflect.apply, qa, Ri;
  if (typeof Ar == "function" && typeof Object.defineProperty == "function")
    try {
      qa = Object.defineProperty({}, "length", { get: function() {
        throw Ri;
      } }), Ri = {}, Ar(function() {
        throw 42;
      }, null, qa);
    } catch (e) {
      e !== Ri && (Ar = null);
    }
  else
    Ar = null;
  var HE = /^\s*class\b/, Ca = function(t) {
    try {
      var r = Hc.call(t);
      return HE.test(r);
    } catch {
      return false;
    }
  }, Ua = function(t) {
    try {
      return Ca(t) ? false : (Hc.call(t), true);
    } catch {
      return false;
    }
  }, Ii = Object.prototype.toString, VE = "[object Object]", YE = "[object Function]", KE = "[object GeneratorFunction]", XE = "[object HTMLAllCollection]", JE = "[object HTML document.all class]", QE = "[object HTMLCollection]", ev = typeof Symbol == "function" && !!Symbol.toStringTag, tv = !(0 in [,]), za = function() {
    return false;
  };
  typeof document == "object" && (Gc = document.all, Ii.call(Gc) === Ii.call(document.all) && (za = function(t) {
    if ((tv || !t) && (typeof t > "u" || typeof t == "object"))
      try {
        var r = Ii.call(t);
        return (r === XE || r === JE || r === QE || r === VE) && t("") == null;
      } catch {
      }
    return false;
  }));
  var Gc;
  Vc.exports = Ar ? function(t) {
    if (za(t))
      return true;
    if (!t || typeof t != "function" && typeof t != "object")
      return false;
    try {
      Ar(t, null, qa);
    } catch (r) {
      if (r !== Ri)
        return false;
    }
    return !Ca(t) && Ua(t);
  } : function(t) {
    if (za(t))
      return true;
    if (!t || typeof t != "function" && typeof t != "object")
      return false;
    if (ev)
      return Ua(t);
    if (Ca(t))
      return false;
    var r = Ii.call(t);
    return r !== YE && r !== KE && !/^\[object HTML/.test(r) ? false : Ua(t);
  };
});
var Wa = g((yx, Xc) => {
  var rv = Yc(), nv = Object.prototype.toString, Kc = Object.prototype.hasOwnProperty, iv = function(t, r, n) {
    for (var i = 0, o = t.length;i < o; i++)
      Kc.call(t, i) && (n == null ? r(t[i], i, t) : r.call(n, t[i], i, t));
  }, ov = function(t, r, n) {
    for (var i = 0, o = t.length;i < o; i++)
      n == null ? r(t.charAt(i), i, t) : r.call(n, t.charAt(i), i, t);
  }, av = function(t, r, n) {
    for (var i in t)
      Kc.call(t, i) && (n == null ? r(t[i], i, t) : r.call(n, t[i], i, t));
  }, fv = function(t, r, n) {
    if (!rv(r))
      throw new TypeError("iterator must be a function");
    var i;
    arguments.length >= 3 && (i = n), nv.call(t) === "[object Array]" ? iv(t, r, i) : typeof t == "string" ? ov(t, r, i) : av(t, r, i);
  };
  Xc.exports = fv;
});
var $a = g((_x, Jc) => {
  var Za = ["BigInt64Array", "BigUint64Array", "Float32Array", "Float64Array", "Int16Array", "Int32Array", "Int8Array", "Uint16Array", "Uint32Array", "Uint8Array", "Uint8ClampedArray"], lv = typeof globalThis > "u" ? global : globalThis;
  Jc.exports = function() {
    for (var t = [], r = 0;r < Za.length; r++)
      typeof lv[Za[r]] == "function" && (t[t.length] = Za[r]);
    return t;
  };
});
var Ga = g((gx, Qc) => {
  var uv = Yr(), Ti = uv("%Object.getOwnPropertyDescriptor%", true);
  if (Ti)
    try {
      Ti([], "length");
    } catch {
      Ti = null;
    }
  Qc.exports = Ti;
});
var Ya = g((bx, id) => {
  var ed = Wa(), sv = $a(), Va = Ai(), cv = Va("Object.prototype.toString"), td = Hr()(), Oi = Ga(), dv = typeof globalThis > "u" ? global : globalThis, rd = sv(), hv = Va("Array.prototype.indexOf", true) || function(t, r) {
    for (var n = 0;n < t.length; n += 1)
      if (t[n] === r)
        return n;
    return -1;
  }, pv = Va("String.prototype.slice"), nd = {}, Ha = Object.getPrototypeOf;
  td && Oi && Ha && ed(rd, function(e) {
    var t = new dv[e];
    if (Symbol.toStringTag in t) {
      var r = Ha(t), n = Oi(r, Symbol.toStringTag);
      if (!n) {
        var i = Ha(r);
        n = Oi(i, Symbol.toStringTag);
      }
      nd[e] = n.get;
    }
  });
  var yv = function(t) {
    var r = false;
    return ed(nd, function(n, i) {
      if (!r)
        try {
          r = n.call(t) === i;
        } catch {
        }
    }), r;
  };
  id.exports = function(t) {
    if (!t || typeof t != "object")
      return false;
    if (!td || !(Symbol.toStringTag in t)) {
      var r = pv(cv(t), 8, -1);
      return hv(rd, r) > -1;
    }
    return Oi ? yv(t) : false;
  };
});
var cd = g((wx, sd) => {
  var ad = Wa(), _v = $a(), fd = Ai(), Ka = Ga(), gv = fd("Object.prototype.toString"), ld = Hr()(), od = typeof globalThis > "u" ? global : globalThis, bv = _v(), wv = fd("String.prototype.slice"), ud = {}, Xa = Object.getPrototypeOf;
  ld && Ka && Xa && ad(bv, function(e) {
    if (typeof od[e] == "function") {
      var t = new od[e];
      if (Symbol.toStringTag in t) {
        var r = Xa(t), n = Ka(r, Symbol.toStringTag);
        if (!n) {
          var i = Xa(r);
          n = Ka(i, Symbol.toStringTag);
        }
        ud[e] = n.get;
      }
    }
  });
  var Ev = function(t) {
    var r = false;
    return ad(ud, function(n, i) {
      if (!r)
        try {
          var o = n.call(t);
          o === i && (r = o);
        } catch {
        }
    }), r;
  }, vv = Ya();
  sd.exports = function(t) {
    return vv(t) ? !ld || !(Symbol.toStringTag in t) ? wv(gv(t), 8, -1) : Ev(t) : false;
  };
});
var xd = g((L) => {
  var mv = zc(), Sv = $c(), Oe = cd(), dd = Ya();
  function xr(e) {
    return e.call.bind(e);
  }
  var hd = typeof BigInt < "u", pd = typeof Symbol < "u", ge = xr(Object.prototype.toString), Av = xr(Number.prototype.valueOf), xv = xr(String.prototype.valueOf), Rv = xr(Boolean.prototype.valueOf);
  hd && (yd = xr(BigInt.prototype.valueOf));
  var yd;
  pd && (_d = xr(Symbol.prototype.valueOf));
  var _d;
  function Xr(e, t) {
    if (typeof e != "object")
      return false;
    try {
      return t(e), true;
    } catch {
      return false;
    }
  }
  L.isArgumentsObject = mv;
  L.isGeneratorFunction = Sv;
  L.isTypedArray = dd;
  function Iv(e) {
    return typeof Promise < "u" && e instanceof Promise || e !== null && typeof e == "object" && typeof e.then == "function" && typeof e.catch == "function";
  }
  L.isPromise = Iv;
  function Tv(e) {
    return typeof ArrayBuffer < "u" && ArrayBuffer.isView ? ArrayBuffer.isView(e) : dd(e) || bd(e);
  }
  L.isArrayBufferView = Tv;
  function Ov(e) {
    return Oe(e) === "Uint8Array";
  }
  L.isUint8Array = Ov;
  function Nv(e) {
    return Oe(e) === "Uint8ClampedArray";
  }
  L.isUint8ClampedArray = Nv;
  function kv(e) {
    return Oe(e) === "Uint16Array";
  }
  L.isUint16Array = kv;
  function Fv(e) {
    return Oe(e) === "Uint32Array";
  }
  L.isUint32Array = Fv;
  function Lv(e) {
    return Oe(e) === "Int8Array";
  }
  L.isInt8Array = Lv;
  function Dv(e) {
    return Oe(e) === "Int16Array";
  }
  L.isInt16Array = Dv;
  function Bv(e) {
    return Oe(e) === "Int32Array";
  }
  L.isInt32Array = Bv;
  function Pv(e) {
    return Oe(e) === "Float32Array";
  }
  L.isFloat32Array = Pv;
  function Mv(e) {
    return Oe(e) === "Float64Array";
  }
  L.isFloat64Array = Mv;
  function jv(e) {
    return Oe(e) === "BigInt64Array";
  }
  L.isBigInt64Array = jv;
  function Uv(e) {
    return Oe(e) === "BigUint64Array";
  }
  L.isBigUint64Array = Uv;
  function Ni(e) {
    return ge(e) === "[object Map]";
  }
  Ni.working = typeof Map < "u" && Ni(new Map);
  function qv(e) {
    return typeof Map > "u" ? false : Ni.working ? Ni(e) : e instanceof Map;
  }
  L.isMap = qv;
  function ki(e) {
    return ge(e) === "[object Set]";
  }
  ki.working = typeof Set < "u" && ki(new Set);
  function Cv(e) {
    return typeof Set > "u" ? false : ki.working ? ki(e) : e instanceof Set;
  }
  L.isSet = Cv;
  function Fi(e) {
    return ge(e) === "[object WeakMap]";
  }
  Fi.working = typeof WeakMap < "u" && Fi(new WeakMap);
  function zv(e) {
    return typeof WeakMap > "u" ? false : Fi.working ? Fi(e) : e instanceof WeakMap;
  }
  L.isWeakMap = zv;
  function Qa(e) {
    return ge(e) === "[object WeakSet]";
  }
  Qa.working = typeof WeakSet < "u" && Qa(new WeakSet);
  function Wv(e) {
    return Qa(e);
  }
  L.isWeakSet = Wv;
  function Li(e) {
    return ge(e) === "[object ArrayBuffer]";
  }
  Li.working = typeof ArrayBuffer < "u" && Li(new ArrayBuffer);
  function gd(e) {
    return typeof ArrayBuffer > "u" ? false : Li.working ? Li(e) : e instanceof ArrayBuffer;
  }
  L.isArrayBuffer = gd;
  function Di(e) {
    return ge(e) === "[object DataView]";
  }
  Di.working = typeof ArrayBuffer < "u" && typeof DataView < "u" && Di(new DataView(new ArrayBuffer(1), 0, 1));
  function bd(e) {
    return typeof DataView > "u" ? false : Di.working ? Di(e) : e instanceof DataView;
  }
  L.isDataView = bd;
  var Ja = typeof SharedArrayBuffer < "u" ? SharedArrayBuffer : undefined;
  function Kr(e) {
    return ge(e) === "[object SharedArrayBuffer]";
  }
  function wd(e) {
    return typeof Ja > "u" ? false : (typeof Kr.working > "u" && (Kr.working = Kr(new Ja)), Kr.working ? Kr(e) : e instanceof Ja);
  }
  L.isSharedArrayBuffer = wd;
  function Zv(e) {
    return ge(e) === "[object AsyncFunction]";
  }
  L.isAsyncFunction = Zv;
  function $v(e) {
    return ge(e) === "[object Map Iterator]";
  }
  L.isMapIterator = $v;
  function Gv(e) {
    return ge(e) === "[object Set Iterator]";
  }
  L.isSetIterator = Gv;
  function Hv(e) {
    return ge(e) === "[object Generator]";
  }
  L.isGeneratorObject = Hv;
  function Vv(e) {
    return ge(e) === "[object WebAssembly.Module]";
  }
  L.isWebAssemblyCompiledModule = Vv;
  function Ed(e) {
    return Xr(e, Av);
  }
  L.isNumberObject = Ed;
  function vd(e) {
    return Xr(e, xv);
  }
  L.isStringObject = vd;
  function md(e) {
    return Xr(e, Rv);
  }
  L.isBooleanObject = md;
  function Sd(e) {
    return hd && Xr(e, yd);
  }
  L.isBigIntObject = Sd;
  function Ad(e) {
    return pd && Xr(e, _d);
  }
  L.isSymbolObject = Ad;
  function Yv(e) {
    return Ed(e) || vd(e) || md(e) || Sd(e) || Ad(e);
  }
  L.isBoxedPrimitive = Yv;
  function Kv(e) {
    return typeof Uint8Array < "u" && (gd(e) || wd(e));
  }
  L.isAnyArrayBuffer = Kv;
  ["isProxy", "isExternal", "isModuleNamespaceObject"].forEach(function(e) {
    Object.defineProperty(L, e, { enumerable: false, value: function() {
      throw new Error(e + " is not supported in userland");
    } });
  });
});
var Id = g((vx, Rd) => {
  Rd.exports = function(t) {
    return t && typeof t == "object" && typeof t.copy == "function" && typeof t.fill == "function" && typeof t.readUInt8 == "function";
  };
});
var Td = g((mx, ef) => {
  typeof Object.create == "function" ? ef.exports = function(t, r) {
    r && (t.super_ = r, t.prototype = Object.create(r.prototype, { constructor: { value: t, enumerable: false, writable: true, configurable: true } }));
  } : ef.exports = function(t, r) {
    if (r) {
      t.super_ = r;
      var n = function() {
      };
      n.prototype = r.prototype, t.prototype = new n, t.prototype.constructor = t;
    }
  };
});
var ff = g((D) => {
  var Od = Object.getOwnPropertyDescriptors || function(t) {
    for (var r = Object.keys(t), n = {}, i = 0;i < r.length; i++)
      n[r[i]] = Object.getOwnPropertyDescriptor(t, r[i]);
    return n;
  }, Xv = /%[sdj%]/g;
  D.format = function(e) {
    if (!Ci(e)) {
      for (var t = [], r = 0;r < arguments.length; r++)
        t.push(ut(arguments[r]));
      return t.join(" ");
    }
    for (var r = 1, n = arguments, i = n.length, o = String(e).replace(Xv, function(f) {
      if (f === "%%")
        return "%";
      if (r >= i)
        return f;
      switch (f) {
        case "%s":
          return String(n[r++]);
        case "%d":
          return Number(n[r++]);
        case "%j":
          try {
            return JSON.stringify(n[r++]);
          } catch {
            return "[Circular]";
          }
        default:
          return f;
      }
    }), a = n[r];r < i; a = n[++r])
      qi(a) || !Rr(a) ? o += " " + a : o += " " + ut(a);
    return o;
  };
  D.deprecate = function(e, t) {
    if (typeof process < "u" && process.noDeprecation === true)
      return e;
    if (typeof process > "u")
      return function() {
        return D.deprecate(e, t).apply(this, arguments);
      };
    var r = false;
    function n() {
      if (!r) {
        if (process.throwDeprecation)
          throw new Error(t);
        process.traceDeprecation ? console.trace(t) : console.error(t), r = true;
      }
      return e.apply(this, arguments);
    }
    return n;
  };
  var Bi = {}, Nd = /^$/;
  process.env.NODE_DEBUG && (Pi = process.env.NODE_DEBUG, Pi = Pi.replace(/[|\\{}()[\]^$+?.]/g, "\\$&").replace(/\*/g, ".*").replace(/,/g, "$|^").toUpperCase(), Nd = new RegExp("^" + Pi + "$", "i"));
  var Pi;
  D.debuglog = function(e) {
    if (e = e.toUpperCase(), !Bi[e])
      if (Nd.test(e)) {
        var t = process.pid;
        Bi[e] = function() {
          var r = D.format.apply(D, arguments);
          console.error("%s %d: %s", e, t, r);
        };
      } else
        Bi[e] = function() {
        };
    return Bi[e];
  };
  function ut(e, t) {
    var r = { seen: [], stylize: Qv };
    return arguments.length >= 3 && (r.depth = arguments[2]), arguments.length >= 4 && (r.colors = arguments[3]), of(t) ? r.showHidden = t : t && D._extend(r, t), Mt(r.showHidden) && (r.showHidden = false), Mt(r.depth) && (r.depth = 2), Mt(r.colors) && (r.colors = false), Mt(r.customInspect) && (r.customInspect = true), r.colors && (r.stylize = Jv), ji(r, e, r.depth);
  }
  D.inspect = ut;
  ut.colors = { bold: [1, 22], italic: [3, 23], underline: [4, 24], inverse: [7, 27], white: [37, 39], grey: [90, 39], black: [30, 39], blue: [34, 39], cyan: [36, 39], green: [32, 39], magenta: [35, 39], red: [31, 39], yellow: [33, 39] };
  ut.styles = { special: "cyan", number: "yellow", boolean: "yellow", undefined: "grey", null: "bold", string: "green", date: "magenta", regexp: "red" };
  function Jv(e, t) {
    var r = ut.styles[t];
    return r ? "\x1B[" + ut.colors[r][0] + "m" + e + "\x1B[" + ut.colors[r][1] + "m" : e;
  }
  function Qv(e, t) {
    return e;
  }
  function em(e) {
    var t = {};
    return e.forEach(function(r, n) {
      t[r] = true;
    }), t;
  }
  function ji(e, t, r) {
    if (e.customInspect && t && Mi(t.inspect) && t.inspect !== D.inspect && !(t.constructor && t.constructor.prototype === t)) {
      var n = t.inspect(r, e);
      return Ci(n) || (n = ji(e, n, r)), n;
    }
    var i = tm(e, t);
    if (i)
      return i;
    var o = Object.keys(t), a = em(o);
    if (e.showHidden && (o = Object.getOwnPropertyNames(t)), Qr(t) && (o.indexOf("message") >= 0 || o.indexOf("description") >= 0))
      return tf(t);
    if (o.length === 0) {
      if (Mi(t)) {
        var f = t.name ? ": " + t.name : "";
        return e.stylize("[Function" + f + "]", "special");
      }
      if (Jr(t))
        return e.stylize(RegExp.prototype.toString.call(t), "regexp");
      if (Ui(t))
        return e.stylize(Date.prototype.toString.call(t), "date");
      if (Qr(t))
        return tf(t);
    }
    var u = "", l = false, s = ["{", "}"];
    if (kd(t) && (l = true, s = ["[", "]"]), Mi(t)) {
      var c = t.name ? ": " + t.name : "";
      u = " [Function" + c + "]";
    }
    if (Jr(t) && (u = " " + RegExp.prototype.toString.call(t)), Ui(t) && (u = " " + Date.prototype.toUTCString.call(t)), Qr(t) && (u = " " + tf(t)), o.length === 0 && (!l || t.length == 0))
      return s[0] + u + s[1];
    if (r < 0)
      return Jr(t) ? e.stylize(RegExp.prototype.toString.call(t), "regexp") : e.stylize("[Object]", "special");
    e.seen.push(t);
    var h;
    return l ? h = rm(e, t, r, a, o) : h = o.map(function(d) {
      return nf(e, t, r, a, d, l);
    }), e.seen.pop(), nm(h, u, s);
  }
  function tm(e, t) {
    if (Mt(t))
      return e.stylize("undefined", "undefined");
    if (Ci(t)) {
      var r = "'" + JSON.stringify(t).replace(/^"|"$/g, "").replace(/'/g, "\\'").replace(/\\"/g, '"') + "'";
      return e.stylize(r, "string");
    }
    if (Fd(t))
      return e.stylize("" + t, "number");
    if (of(t))
      return e.stylize("" + t, "boolean");
    if (qi(t))
      return e.stylize("null", "null");
  }
  function tf(e) {
    return "[" + Error.prototype.toString.call(e) + "]";
  }
  function rm(e, t, r, n, i) {
    for (var o = [], a = 0, f = t.length;a < f; ++a)
      Ld(t, String(a)) ? o.push(nf(e, t, r, n, String(a), true)) : o.push("");
    return i.forEach(function(u) {
      u.match(/^\d+$/) || o.push(nf(e, t, r, n, u, true));
    }), o;
  }
  function nf(e, t, r, n, i, o) {
    var a, f, u;
    if (u = Object.getOwnPropertyDescriptor(t, i) || { value: t[i] }, u.get ? u.set ? f = e.stylize("[Getter/Setter]", "special") : f = e.stylize("[Getter]", "special") : u.set && (f = e.stylize("[Setter]", "special")), Ld(n, i) || (a = "[" + i + "]"), f || (e.seen.indexOf(u.value) < 0 ? (qi(r) ? f = ji(e, u.value, null) : f = ji(e, u.value, r - 1), f.indexOf(`
`) > -1 && (o ? f = f.split(`
`).map(function(l) {
      return "  " + l;
    }).join(`
`).slice(2) : f = `
` + f.split(`
`).map(function(l) {
      return "   " + l;
    }).join(`
`))) : f = e.stylize("[Circular]", "special")), Mt(a)) {
      if (o && i.match(/^\d+$/))
        return f;
      a = JSON.stringify("" + i), a.match(/^"([a-zA-Z_][a-zA-Z_0-9]*)"$/) ? (a = a.slice(1, -1), a = e.stylize(a, "name")) : (a = a.replace(/'/g, "\\'").replace(/\\"/g, '"').replace(/(^"|"$)/g, "'"), a = e.stylize(a, "string"));
    }
    return a + ": " + f;
  }
  function nm(e, t, r) {
    var n = 0, i = e.reduce(function(o, a) {
      return n++, a.indexOf(`
`) >= 0 && n++, o + a.replace(/\u001b\[\d\d?m/g, "").length + 1;
    }, 0);
    return i > 60 ? r[0] + (t === "" ? "" : t + `
 `) + " " + e.join(`,
  `) + " " + r[1] : r[0] + t + " " + e.join(", ") + " " + r[1];
  }
  D.types = xd();
  function kd(e) {
    return Array.isArray(e);
  }
  D.isArray = kd;
  function of(e) {
    return typeof e == "boolean";
  }
  D.isBoolean = of;
  function qi(e) {
    return e === null;
  }
  D.isNull = qi;
  function im(e) {
    return e == null;
  }
  D.isNullOrUndefined = im;
  function Fd(e) {
    return typeof e == "number";
  }
  D.isNumber = Fd;
  function Ci(e) {
    return typeof e == "string";
  }
  D.isString = Ci;
  function om(e) {
    return typeof e == "symbol";
  }
  D.isSymbol = om;
  function Mt(e) {
    return e === undefined;
  }
  D.isUndefined = Mt;
  function Jr(e) {
    return Rr(e) && af(e) === "[object RegExp]";
  }
  D.isRegExp = Jr;
  D.types.isRegExp = Jr;
  function Rr(e) {
    return typeof e == "object" && e !== null;
  }
  D.isObject = Rr;
  function Ui(e) {
    return Rr(e) && af(e) === "[object Date]";
  }
  D.isDate = Ui;
  D.types.isDate = Ui;
  function Qr(e) {
    return Rr(e) && (af(e) === "[object Error]" || e instanceof Error);
  }
  D.isError = Qr;
  D.types.isNativeError = Qr;
  function Mi(e) {
    return typeof e == "function";
  }
  D.isFunction = Mi;
  function am(e) {
    return e === null || typeof e == "boolean" || typeof e == "number" || typeof e == "string" || typeof e == "symbol" || typeof e > "u";
  }
  D.isPrimitive = am;
  D.isBuffer = Id();
  function af(e) {
    return Object.prototype.toString.call(e);
  }
  function rf(e) {
    return e < 10 ? "0" + e.toString(10) : e.toString(10);
  }
  var fm = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function lm() {
    var e = new Date, t = [rf(e.getHours()), rf(e.getMinutes()), rf(e.getSeconds())].join(":");
    return [e.getDate(), fm[e.getMonth()], t].join(" ");
  }
  D.log = function() {
    console.log("%s - %s", lm(), D.format.apply(D, arguments));
  };
  D.inherits = Td();
  D._extend = function(e, t) {
    if (!t || !Rr(t))
      return e;
    for (var r = Object.keys(t), n = r.length;n--; )
      e[r[n]] = t[r[n]];
    return e;
  };
  function Ld(e, t) {
    return Object.prototype.hasOwnProperty.call(e, t);
  }
  var Pt = typeof Symbol < "u" ? Symbol("util.promisify.custom") : undefined;
  D.promisify = function(t) {
    if (typeof t != "function")
      throw new TypeError('The "original" argument must be of type Function');
    if (Pt && t[Pt]) {
      var r = t[Pt];
      if (typeof r != "function")
        throw new TypeError('The "util.promisify.custom" argument must be of type Function');
      return Object.defineProperty(r, Pt, { value: r, enumerable: false, writable: false, configurable: true }), r;
    }
    function r() {
      for (var n, i, o = new Promise(function(u, l) {
        n = u, i = l;
      }), a = [], f = 0;f < arguments.length; f++)
        a.push(arguments[f]);
      a.push(function(u, l) {
        u ? i(u) : n(l);
      });
      try {
        t.apply(this, a);
      } catch (u) {
        i(u);
      }
      return o;
    }
    return Object.setPrototypeOf(r, Object.getPrototypeOf(t)), Pt && Object.defineProperty(r, Pt, { value: r, enumerable: false, writable: false, configurable: true }), Object.defineProperties(r, Od(t));
  };
  D.promisify.custom = Pt;
  function um(e, t) {
    if (!e) {
      var r = new Error("Promise was rejected with a falsy value");
      r.reason = e, e = r;
    }
    return t(e);
  }
  function sm(e) {
    if (typeof e != "function")
      throw new TypeError('The "original" argument must be of type Function');
    function t() {
      for (var r = [], n = 0;n < arguments.length; n++)
        r.push(arguments[n]);
      var i = r.pop();
      if (typeof i != "function")
        throw new TypeError("The last argument must be of type Function");
      var o = this, a = function() {
        return i.apply(o, arguments);
      };
      e.apply(this, r).then(function(f) {
        process.nextTick(a.bind(null, null, f));
      }, function(f) {
        process.nextTick(um.bind(null, f, a));
      });
    }
    return Object.setPrototypeOf(t, Object.getPrototypeOf(e)), Object.defineProperties(t, Od(e)), t;
  }
  D.callbackify = sm;
});
var be = {};
Bn(be, { TextDecoder: () => Bd, TextEncoder: () => Dd, default: () => cm });
var Dd;
var Bd;
var cm;
var Ir = wo(() => {
  X(be, vt(ff()));
  Dd = globalThis.TextEncoder, Bd = globalThis.TextDecoder, cm = { TextEncoder: Dd, TextDecoder: Bd };
});
var cf = g((xx, jd) => {
  function Tr(e) {
    return typeof Symbol == "function" && typeof Symbol.iterator == "symbol" ? Tr = function(r) {
      return typeof r;
    } : Tr = function(r) {
      return r && typeof Symbol == "function" && r.constructor === Symbol && r !== Symbol.prototype ? "symbol" : typeof r;
    }, Tr(e);
  }
  function dm(e, t) {
    if (!(e instanceof t))
      throw new TypeError("Cannot call a class as a function");
  }
  function hm(e, t) {
    return t && (Tr(t) === "object" || typeof t == "function") ? t : pm(e);
  }
  function pm(e) {
    if (e === undefined)
      throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
    return e;
  }
  function uf(e) {
    return uf = Object.setPrototypeOf ? Object.getPrototypeOf : function(r) {
      return r.__proto__ || Object.getPrototypeOf(r);
    }, uf(e);
  }
  function ym(e, t) {
    if (typeof t != "function" && t !== null)
      throw new TypeError("Super expression must either be null or a function");
    e.prototype = Object.create(t && t.prototype, { constructor: { value: e, writable: true, configurable: true } }), t && sf(e, t);
  }
  function sf(e, t) {
    return sf = Object.setPrototypeOf || function(n, i) {
      return n.__proto__ = i, n;
    }, sf(e, t);
  }
  var Md = {}, Or, lf;
  function en(e, t, r) {
    r || (r = Error);
    function n(o, a, f) {
      return typeof t == "string" ? t : t(o, a, f);
    }
    var i = function(o) {
      ym(a, o);
      function a(f, u, l) {
        var s;
        return dm(this, a), s = hm(this, uf(a).call(this, n(f, u, l))), s.code = e, s;
      }
      return a;
    }(r);
    Md[e] = i;
  }
  function Pd(e, t) {
    if (Array.isArray(e)) {
      var r = e.length;
      return e = e.map(function(n) {
        return String(n);
      }), r > 2 ? "one of ".concat(t, " ").concat(e.slice(0, r - 1).join(", "), ", or ") + e[r - 1] : r === 2 ? "one of ".concat(t, " ").concat(e[0], " or ").concat(e[1]) : "of ".concat(t, " ").concat(e[0]);
    } else
      return "of ".concat(t, " ").concat(String(e));
  }
  function _m(e, t, r) {
    return e.substr(!r || r < 0 ? 0 : +r, t.length) === t;
  }
  function gm(e, t, r) {
    return (r === undefined || r > e.length) && (r = e.length), e.substring(r - t.length, r) === t;
  }
  function bm(e, t, r) {
    return typeof r != "number" && (r = 0), r + t.length > e.length ? false : e.indexOf(t, r) !== -1;
  }
  en("ERR_AMBIGUOUS_ARGUMENT", 'The "%s" argument is ambiguous. %s', TypeError);
  en("ERR_INVALID_ARG_TYPE", function(e, t, r) {
    Or === undefined && (Or = tn()), Or(typeof e == "string", "'name' must be a string");
    var n;
    typeof t == "string" && _m(t, "not ") ? (n = "must not be", t = t.replace(/^not /, "")) : n = "must be";
    var i;
    if (gm(e, " argument"))
      i = "The ".concat(e, " ").concat(n, " ").concat(Pd(t, "type"));
    else {
      var o = bm(e, ".") ? "property" : "argument";
      i = 'The "'.concat(e, '" ').concat(o, " ").concat(n, " ").concat(Pd(t, "type"));
    }
    return i += ". Received type ".concat(Tr(r)), i;
  }, TypeError);
  en("ERR_INVALID_ARG_VALUE", function(e, t) {
    var r = arguments.length > 2 && arguments[2] !== undefined ? arguments[2] : "is invalid";
    lf === undefined && (lf = (Ir(), se(be)));
    var n = lf.inspect(t);
    return n.length > 128 && (n = "".concat(n.slice(0, 128), "...")), "The argument '".concat(e, "' ").concat(r, ". Received ").concat(n);
  }, TypeError, RangeError);
  en("ERR_INVALID_RETURN_VALUE", function(e, t, r) {
    var n;
    return r && r.constructor && r.constructor.name ? n = "instance of ".concat(r.constructor.name) : n = "type ".concat(Tr(r)), "Expected ".concat(e, ' to be returned from the "').concat(t, '"') + " function but got ".concat(n, ".");
  }, TypeError);
  en("ERR_MISSING_ARGS", function() {
    for (var e = arguments.length, t = new Array(e), r = 0;r < e; r++)
      t[r] = arguments[r];
    Or === undefined && (Or = tn()), Or(t.length > 0, "At least one arg needs to be specified");
    var n = "The ", i = t.length;
    switch (t = t.map(function(o) {
      return '"'.concat(o, '"');
    }), i) {
      case 1:
        n += "".concat(t[0], " argument");
        break;
      case 2:
        n += "".concat(t[0], " and ").concat(t[1], " arguments");
        break;
      default:
        n += t.slice(0, i - 1).join(", "), n += ", and ".concat(t[i - 1], " arguments");
        break;
    }
    return "".concat(n, " must be specified");
  }, TypeError);
  jd.exports.codes = Md;
});
var Wd = g((Rx, zd) => {
  function wm(e) {
    for (var t = 1;t < arguments.length; t++) {
      var r = arguments[t] != null ? arguments[t] : {}, n = Object.keys(r);
      typeof Object.getOwnPropertySymbols == "function" && (n = n.concat(Object.getOwnPropertySymbols(r).filter(function(i) {
        return Object.getOwnPropertyDescriptor(r, i).enumerable;
      }))), n.forEach(function(i) {
        Em(e, i, r[i]);
      });
    }
    return e;
  }
  function Em(e, t, r) {
    return t in e ? Object.defineProperty(e, t, { value: r, enumerable: true, configurable: true, writable: true }) : e[t] = r, e;
  }
  function vm(e, t) {
    if (!(e instanceof t))
      throw new TypeError("Cannot call a class as a function");
  }
  function Ud(e, t) {
    for (var r = 0;r < t.length; r++) {
      var n = t[r];
      n.enumerable = n.enumerable || false, n.configurable = true, "value" in n && (n.writable = true), Object.defineProperty(e, n.key, n);
    }
  }
  function mm(e, t, r) {
    return t && Ud(e.prototype, t), r && Ud(e, r), e;
  }
  function Nr(e, t) {
    return t && (we(t) === "object" || typeof t == "function") ? t : df(e);
  }
  function df(e) {
    if (e === undefined)
      throw new ReferenceError("this hasn't been initialised - super() hasn't been called");
    return e;
  }
  function Sm(e, t) {
    if (typeof t != "function" && t !== null)
      throw new TypeError("Super expression must either be null or a function");
    e.prototype = Object.create(t && t.prototype, { constructor: { value: e, writable: true, configurable: true } }), t && an(e, t);
  }
  function hf(e) {
    var t = typeof Map == "function" ? new Map : undefined;
    return hf = function(n) {
      if (n === null || !xm(n))
        return n;
      if (typeof n != "function")
        throw new TypeError("Super expression must either be null or a function");
      if (typeof t < "u") {
        if (t.has(n))
          return t.get(n);
        t.set(n, i);
      }
      function i() {
        return zi(n, arguments, st(this).constructor);
      }
      return i.prototype = Object.create(n.prototype, { constructor: { value: i, enumerable: false, writable: true, configurable: true } }), an(i, n);
    }, hf(e);
  }
  function Am() {
    if (typeof Reflect > "u" || !Reflect.construct || Reflect.construct.sham)
      return false;
    if (typeof Proxy == "function")
      return true;
    try {
      return Date.prototype.toString.call(Reflect.construct(Date, [], function() {
      })), true;
    } catch {
      return false;
    }
  }
  function zi(e, t, r) {
    return Am() ? zi = Reflect.construct : zi = function(i, o, a) {
      var f = [null];
      f.push.apply(f, o);
      var u = Function.bind.apply(i, f), l = new u;
      return a && an(l, a.prototype), l;
    }, zi.apply(null, arguments);
  }
  function xm(e) {
    return Function.toString.call(e).indexOf("[native code]") !== -1;
  }
  function an(e, t) {
    return an = Object.setPrototypeOf || function(n, i) {
      return n.__proto__ = i, n;
    }, an(e, t);
  }
  function st(e) {
    return st = Object.setPrototypeOf ? Object.getPrototypeOf : function(r) {
      return r.__proto__ || Object.getPrototypeOf(r);
    }, st(e);
  }
  function we(e) {
    return typeof Symbol == "function" && typeof Symbol.iterator == "symbol" ? we = function(r) {
      return typeof r;
    } : we = function(r) {
      return r && typeof Symbol == "function" && r.constructor === Symbol && r !== Symbol.prototype ? "symbol" : typeof r;
    }, we(e);
  }
  var Rm = (Ir(), se(be)), pf = Rm.inspect, Im = cf(), Tm = Im.codes.ERR_INVALID_ARG_TYPE;
  function qd(e, t, r) {
    return (r === undefined || r > e.length) && (r = e.length), e.substring(r - t.length, r) === t;
  }
  function Om(e, t) {
    if (t = Math.floor(t), e.length == 0 || t == 0)
      return "";
    var r = e.length * t;
    for (t = Math.floor(Math.log(t) / Math.log(2));t; )
      e += e, t--;
    return e += e.substring(0, r - e.length), e;
  }
  var Ne = "", rn = "", nn = "", ee = "", jt = { deepStrictEqual: "Expected values to be strictly deep-equal:", strictEqual: "Expected values to be strictly equal:", strictEqualObject: 'Expected "actual" to be reference-equal to "expected":', deepEqual: "Expected values to be loosely deep-equal:", equal: "Expected values to be loosely equal:", notDeepStrictEqual: 'Expected "actual" not to be strictly deep-equal to:', notStrictEqual: 'Expected "actual" to be strictly unequal to:', notStrictEqualObject: 'Expected "actual" not to be reference-equal to "expected":', notDeepEqual: 'Expected "actual" not to be loosely deep-equal to:', notEqual: 'Expected "actual" to be loosely unequal to:', notIdentical: "Values identical but not reference-equal:" }, Nm = 10;
  function Cd(e) {
    var t = Object.keys(e), r = Object.create(Object.getPrototypeOf(e));
    return t.forEach(function(n) {
      r[n] = e[n];
    }), Object.defineProperty(r, "message", { value: e.message }), r;
  }
  function on(e) {
    return pf(e, { compact: false, customInspect: false, depth: 1000, maxArrayLength: 1 / 0, showHidden: false, breakLength: 1 / 0, showProxy: false, sorted: true, getters: true });
  }
  function km(e, t, r) {
    var n = "", i = "", o = 0, a = "", f = false, u = on(e), l = u.split(`
`), s = on(t).split(`
`), c = 0, h = "";
    if (r === "strictEqual" && we(e) === "object" && we(t) === "object" && e !== null && t !== null && (r = "strictEqualObject"), l.length === 1 && s.length === 1 && l[0] !== s[0]) {
      var d = l[0].length + s[0].length;
      if (d <= Nm) {
        if ((we(e) !== "object" || e === null) && (we(t) !== "object" || t === null) && (e !== 0 || t !== 0))
          return "".concat(jt[r], `

`) + "".concat(l[0], " !== ").concat(s[0], `
`);
      } else if (r !== "strictEqualObject") {
        var y = process.stderr && process.stderr.isTTY ? process.stderr.columns : 80;
        if (d < y) {
          for (;l[0][c] === s[0][c]; )
            c++;
          c > 2 && (h = `
  `.concat(Om(" ", c), "^"), c = 0);
        }
      }
    }
    for (var b = l[l.length - 1], R = s[s.length - 1];b === R && (c++ < 2 ? a = `
  `.concat(b).concat(a) : n = b, l.pop(), s.pop(), !(l.length === 0 || s.length === 0)); )
      b = l[l.length - 1], R = s[s.length - 1];
    var _ = Math.max(l.length, s.length);
    if (_ === 0) {
      var E = u.split(`
`);
      if (E.length > 30)
        for (E[26] = "".concat(Ne, "...").concat(ee);E.length > 27; )
          E.pop();
      return "".concat(jt.notIdentical, `

`).concat(E.join(`
`), `
`);
    }
    c > 3 && (a = `
`.concat(Ne, "...").concat(ee).concat(a), f = true), n !== "" && (a = `
  `.concat(n).concat(a), n = "");
    var m = 0, A = jt[r] + `
`.concat(rn, "+ actual").concat(ee, " ").concat(nn, "- expected").concat(ee), v = " ".concat(Ne, "...").concat(ee, " Lines skipped");
    for (c = 0;c < _; c++) {
      var T = c - o;
      if (l.length < c + 1)
        T > 1 && c > 2 && (T > 4 ? (i += `
`.concat(Ne, "...").concat(ee), f = true) : T > 3 && (i += `
  `.concat(s[c - 2]), m++), i += `
  `.concat(s[c - 1]), m++), o = c, n += `
`.concat(nn, "-").concat(ee, " ").concat(s[c]), m++;
      else if (s.length < c + 1)
        T > 1 && c > 2 && (T > 4 ? (i += `
`.concat(Ne, "...").concat(ee), f = true) : T > 3 && (i += `
  `.concat(l[c - 2]), m++), i += `
  `.concat(l[c - 1]), m++), o = c, i += `
`.concat(rn, "+").concat(ee, " ").concat(l[c]), m++;
      else {
        var I = s[c], S = l[c], k = S !== I && (!qd(S, ",") || S.slice(0, -1) !== I);
        k && qd(I, ",") && I.slice(0, -1) === S && (k = false, S += ","), k ? (T > 1 && c > 2 && (T > 4 ? (i += `
`.concat(Ne, "...").concat(ee), f = true) : T > 3 && (i += `
  `.concat(l[c - 2]), m++), i += `
  `.concat(l[c - 1]), m++), o = c, i += `
`.concat(rn, "+").concat(ee, " ").concat(S), n += `
`.concat(nn, "-").concat(ee, " ").concat(I), m += 2) : (i += n, n = "", (T === 1 || c === 0) && (i += `
  `.concat(S), m++));
      }
      if (m > 20 && c < _ - 2)
        return "".concat(A).concat(v, `
`).concat(i, `
`).concat(Ne, "...").concat(ee).concat(n, `
`) + "".concat(Ne, "...").concat(ee);
    }
    return "".concat(A).concat(f ? v : "", `
`).concat(i).concat(n).concat(a).concat(h);
  }
  var Fm = function(e) {
    Sm(t, e);
    function t(r) {
      var n;
      if (vm(this, t), we(r) !== "object" || r === null)
        throw new Tm("options", "Object", r);
      var { message: i, operator: o, stackStartFn: a, actual: f, expected: u } = r, l = Error.stackTraceLimit;
      if (Error.stackTraceLimit = 0, i != null)
        n = Nr(this, st(t).call(this, String(i)));
      else if (process.stderr && process.stderr.isTTY && (process.stderr && process.stderr.getColorDepth && process.stderr.getColorDepth() !== 1 ? (Ne = "\x1B[34m", rn = "\x1B[32m", ee = "\x1B[39m", nn = "\x1B[31m") : (Ne = "", rn = "", ee = "", nn = "")), we(f) === "object" && f !== null && we(u) === "object" && u !== null && "stack" in f && f instanceof Error && "stack" in u && u instanceof Error && (f = Cd(f), u = Cd(u)), o === "deepStrictEqual" || o === "strictEqual")
        n = Nr(this, st(t).call(this, km(f, u, o)));
      else if (o === "notDeepStrictEqual" || o === "notStrictEqual") {
        var s = jt[o], c = on(f).split(`
`);
        if (o === "notStrictEqual" && we(f) === "object" && f !== null && (s = jt.notStrictEqualObject), c.length > 30)
          for (c[26] = "".concat(Ne, "...").concat(ee);c.length > 27; )
            c.pop();
        c.length === 1 ? n = Nr(this, st(t).call(this, "".concat(s, " ").concat(c[0]))) : n = Nr(this, st(t).call(this, "".concat(s, `

`).concat(c.join(`
`), `
`)));
      } else {
        var h = on(f), d = "", y = jt[o];
        o === "notDeepEqual" || o === "notEqual" ? (h = "".concat(jt[o], `

`).concat(h), h.length > 1024 && (h = "".concat(h.slice(0, 1021), "..."))) : (d = "".concat(on(u)), h.length > 512 && (h = "".concat(h.slice(0, 509), "...")), d.length > 512 && (d = "".concat(d.slice(0, 509), "...")), o === "deepEqual" || o === "equal" ? h = "".concat(y, `

`).concat(h, `

should equal

`) : d = " ".concat(o, " ").concat(d)), n = Nr(this, st(t).call(this, "".concat(h).concat(d)));
      }
      return Error.stackTraceLimit = l, n.generatedMessage = !i, Object.defineProperty(df(n), "name", { value: "AssertionError [ERR_ASSERTION]", enumerable: false, writable: true, configurable: true }), n.code = "ERR_ASSERTION", n.actual = f, n.expected = u, n.operator = o, Error.captureStackTrace && Error.captureStackTrace(df(n), a), n.stack, n.name = "AssertionError", Nr(n);
    }
    return mm(t, [{ key: "toString", value: function() {
      return "".concat(this.name, " [").concat(this.code, "]: ").concat(this.message);
    } }, { key: pf.custom, value: function(n, i) {
      return pf(this, wm({}, i, { customInspect: false, depth: 0 }));
    } }]), t;
  }(hf(Error));
  zd.exports = Fm;
});
var Gd = g((Ix, $d) => {
  function Zd(e, t) {
    if (e == null)
      throw new TypeError("Cannot convert first argument to object");
    for (var r = Object(e), n = 1;n < arguments.length; n++) {
      var i = arguments[n];
      if (i != null)
        for (var o = Object.keys(Object(i)), a = 0, f = o.length;a < f; a++) {
          var u = o[a], l = Object.getOwnPropertyDescriptor(i, u);
          l !== undefined && l.enumerable && (r[u] = i[u]);
        }
    }
    return r;
  }
  function Lm() {
    Object.assign || Object.defineProperty(Object, "assign", { enumerable: false, configurable: true, writable: true, value: Zd });
  }
  $d.exports = { assign: Zd, polyfill: Lm };
});
var yf = g((Tx, Vd) => {
  var Hd = Object.prototype.toString;
  Vd.exports = function(t) {
    var r = Hd.call(t), n = r === "[object Arguments]";
    return n || (n = r !== "[object Array]" && t !== null && typeof t == "object" && typeof t.length == "number" && t.length >= 0 && Hd.call(t.callee) === "[object Function]"), n;
  };
});
var nh = g((Ox, rh) => {
  var th;
  Object.keys || (fn = Object.prototype.hasOwnProperty, _f = Object.prototype.toString, Yd = yf(), gf = Object.prototype.propertyIsEnumerable, Kd = !gf.call({ toString: null }, "toString"), Xd = gf.call(function() {
  }, "prototype"), ln = ["toString", "toLocaleString", "valueOf", "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "constructor"], Wi = function(e) {
    var t = e.constructor;
    return t && t.prototype === e;
  }, Jd = { $applicationCache: true, $console: true, $external: true, $frame: true, $frameElement: true, $frames: true, $innerHeight: true, $innerWidth: true, $onmozfullscreenchange: true, $onmozfullscreenerror: true, $outerHeight: true, $outerWidth: true, $pageXOffset: true, $pageYOffset: true, $parent: true, $scrollLeft: true, $scrollTop: true, $scrollX: true, $scrollY: true, $self: true, $webkitIndexedDB: true, $webkitStorageInfo: true, $window: true }, Qd = function() {
    if (typeof window > "u")
      return false;
    for (var e in window)
      try {
        if (!Jd["$" + e] && fn.call(window, e) && window[e] !== null && typeof window[e] == "object")
          try {
            Wi(window[e]);
          } catch {
            return true;
          }
      } catch {
        return true;
      }
    return false;
  }(), eh = function(e) {
    if (typeof window > "u" || !Qd)
      return Wi(e);
    try {
      return Wi(e);
    } catch {
      return false;
    }
  }, th = function(t) {
    var r = t !== null && typeof t == "object", n = _f.call(t) === "[object Function]", i = Yd(t), o = r && _f.call(t) === "[object String]", a = [];
    if (!r && !n && !i)
      throw new TypeError("Object.keys called on a non-object");
    var f = Xd && n;
    if (o && t.length > 0 && !fn.call(t, 0))
      for (var u = 0;u < t.length; ++u)
        a.push(String(u));
    if (i && t.length > 0)
      for (var l = 0;l < t.length; ++l)
        a.push(String(l));
    else
      for (var s in t)
        !(f && s === "prototype") && fn.call(t, s) && a.push(String(s));
    if (Kd)
      for (var c = eh(t), h = 0;h < ln.length; ++h)
        !(c && ln[h] === "constructor") && fn.call(t, ln[h]) && a.push(ln[h]);
    return a;
  });
  var fn, _f, Yd, gf, Kd, Xd, ln, Wi, Jd, Qd, eh;
  rh.exports = th;
});
var fh = g((Nx, ah) => {
  var Dm = Array.prototype.slice, Bm = yf(), ih = Object.keys, Zi = ih ? function(t) {
    return ih(t);
  } : nh(), oh = Object.keys;
  Zi.shim = function() {
    if (Object.keys) {
      var t = function() {
        var r = Object.keys(arguments);
        return r && r.length === arguments.length;
      }(1, 2);
      t || (Object.keys = function(n) {
        return Bm(n) ? oh(Dm.call(n)) : oh(n);
      });
    } else
      Object.keys = Zi;
    return Object.keys || Zi;
  };
  ah.exports = Zi;
});
var uh = g((kx, lh) => {
  var Pm = Yr(), bf = Pm("%Object.defineProperty%", true), wf = function() {
    if (bf)
      try {
        return bf({}, "a", { value: 1 }), true;
      } catch {
        return false;
      }
    return false;
  };
  wf.hasArrayLengthDefineBug = function() {
    if (!wf())
      return null;
    try {
      return bf([], "length", { value: 1 }).length !== 1;
    } catch {
      return true;
    }
  };
  lh.exports = wf;
});
var un = g((Fx, hh) => {
  var Mm = fh(), jm = typeof Symbol == "function" && typeof Symbol("foo") == "symbol", Um = Object.prototype.toString, qm = Array.prototype.concat, sh = Object.defineProperty, Cm = function(e) {
    return typeof e == "function" && Um.call(e) === "[object Function]";
  }, zm = uh()(), ch = sh && zm, Wm = function(e, t, r, n) {
    if (t in e) {
      if (n === true) {
        if (e[t] === r)
          return;
      } else if (!Cm(n) || !n())
        return;
    }
    ch ? sh(e, t, { configurable: true, enumerable: false, value: r, writable: true }) : e[t] = r;
  }, dh = function(e, t) {
    var r = arguments.length > 2 ? arguments[2] : {}, n = Mm(t);
    jm && (n = qm.call(n, Object.getOwnPropertySymbols(t)));
    for (var i = 0;i < n.length; i += 1)
      Wm(e, n[i], t[n[i]], r[n[i]]);
  };
  dh.supportsDescriptors = !!ch;
  hh.exports = dh;
});
var Ef = g((Lx, yh) => {
  var ph = function(e) {
    return e !== e;
  };
  yh.exports = function(t, r) {
    return t === 0 && r === 0 ? 1 / t === 1 / r : !!(t === r || ph(t) && ph(r));
  };
});
var vf = g((Dx, _h) => {
  var Zm = Ef();
  _h.exports = function() {
    return typeof Object.is == "function" ? Object.is : Zm;
  };
});
var bh = g((Bx, gh) => {
  var $m = vf(), Gm = un();
  gh.exports = function() {
    var t = $m();
    return Gm(Object, { is: t }, { is: function() {
      return Object.is !== t;
    } }), t;
  };
});
var mf = g((Px, vh) => {
  var Hm = un(), Vm = Si(), Ym = Ef(), wh = vf(), Km = bh(), Eh = Vm(wh(), Object);
  Hm(Eh, { getPolyfill: wh, implementation: Ym, shim: Km });
  vh.exports = Eh;
});
var Sf = g((Mx, mh) => {
  mh.exports = function(t) {
    return t !== t;
  };
});
var Af = g((jx, Sh) => {
  var Xm = Sf();
  Sh.exports = function() {
    return Number.isNaN && Number.isNaN(NaN) && !Number.isNaN("a") ? Number.isNaN : Xm;
  };
});
var xh = g((Ux, Ah) => {
  var Jm = un(), Qm = Af();
  Ah.exports = function() {
    var t = Qm();
    return Jm(Number, { isNaN: t }, { isNaN: function() {
      return Number.isNaN !== t;
    } }), t;
  };
});
var Oh = g((qx, Th) => {
  var eS = Si(), tS = un(), rS = Sf(), Rh = Af(), nS = xh(), Ih = eS(Rh(), Number);
  tS(Ih, { getPolyfill: Rh, implementation: rS, shim: nS });
  Th.exports = Ih;
});
var Vh = g((Cx, Hh) => {
  function Nh(e, t) {
    return aS(e) || oS(e, t) || iS();
  }
  function iS() {
    throw new TypeError("Invalid attempt to destructure non-iterable instance");
  }
  function oS(e, t) {
    var r = [], n = true, i = false, o = undefined;
    try {
      for (var a = e[Symbol.iterator](), f;!(n = (f = a.next()).done) && (r.push(f.value), !(t && r.length === t)); n = true)
        ;
    } catch (u) {
      i = true, o = u;
    } finally {
      try {
        !n && a.return != null && a.return();
      } finally {
        if (i)
          throw o;
      }
    }
    return r;
  }
  function aS(e) {
    if (Array.isArray(e))
      return e;
  }
  function ce(e) {
    return typeof Symbol == "function" && typeof Symbol.iterator == "symbol" ? ce = function(r) {
      return typeof r;
    } : ce = function(r) {
      return r && typeof Symbol == "function" && r.constructor === Symbol && r !== Symbol.prototype ? "symbol" : typeof r;
    }, ce(e);
  }
  var fS = /a/g.flags !== undefined, Xi = function(t) {
    var r = [];
    return t.forEach(function(n) {
      return r.push(n);
    }), r;
  }, kh = function(t) {
    var r = [];
    return t.forEach(function(n, i) {
      return r.push([i, n]);
    }), r;
  }, zh = Object.is ? Object.is : mf(), Yi = Object.getOwnPropertySymbols ? Object.getOwnPropertySymbols : function() {
    return [];
  }, xf = Number.isNaN ? Number.isNaN : Oh();
  function If(e) {
    return e.call.bind(e);
  }
  var cn = If(Object.prototype.hasOwnProperty), Ki = If(Object.prototype.propertyIsEnumerable), Fh = If(Object.prototype.toString), ie = (Ir(), se(be)).types, lS = ie.isAnyArrayBuffer, uS = ie.isArrayBufferView, Lh = ie.isDate, $i = ie.isMap, Dh = ie.isRegExp, Gi = ie.isSet, sS = ie.isNativeError, cS = ie.isBoxedPrimitive, Bh = ie.isNumberObject, Ph = ie.isStringObject, Mh = ie.isBooleanObject, jh = ie.isBigIntObject, dS = ie.isSymbolObject, hS = ie.isFloat32Array, pS = ie.isFloat64Array;
  function yS(e) {
    if (e.length === 0 || e.length > 10)
      return true;
    for (var t = 0;t < e.length; t++) {
      var r = e.charCodeAt(t);
      if (r < 48 || r > 57)
        return true;
    }
    return e.length === 10 && e >= Math.pow(2, 32);
  }
  function Hi(e) {
    return Object.keys(e).filter(yS).concat(Yi(e).filter(Object.prototype.propertyIsEnumerable.bind(e)));
  }
  function Wh(e, t) {
    if (e === t)
      return 0;
    for (var r = e.length, n = t.length, i = 0, o = Math.min(r, n);i < o; ++i)
      if (e[i] !== t[i]) {
        r = e[i], n = t[i];
        break;
      }
    return r < n ? -1 : n < r ? 1 : 0;
  }
  var Vi = undefined, _S = true, gS = false, Rf = 0, Tf = 1, Zh = 2, $h = 3;
  function bS(e, t) {
    return fS ? e.source === t.source && e.flags === t.flags : RegExp.prototype.toString.call(e) === RegExp.prototype.toString.call(t);
  }
  function wS(e, t) {
    if (e.byteLength !== t.byteLength)
      return false;
    for (var r = 0;r < e.byteLength; r++)
      if (e[r] !== t[r])
        return false;
    return true;
  }
  function ES(e, t) {
    return e.byteLength !== t.byteLength ? false : Wh(new Uint8Array(e.buffer, e.byteOffset, e.byteLength), new Uint8Array(t.buffer, t.byteOffset, t.byteLength)) === 0;
  }
  function vS(e, t) {
    return e.byteLength === t.byteLength && Wh(new Uint8Array(e), new Uint8Array(t)) === 0;
  }
  function mS(e, t) {
    return Bh(e) ? Bh(t) && zh(Number.prototype.valueOf.call(e), Number.prototype.valueOf.call(t)) : Ph(e) ? Ph(t) && String.prototype.valueOf.call(e) === String.prototype.valueOf.call(t) : Mh(e) ? Mh(t) && Boolean.prototype.valueOf.call(e) === Boolean.prototype.valueOf.call(t) : jh(e) ? jh(t) && BigInt.prototype.valueOf.call(e) === BigInt.prototype.valueOf.call(t) : dS(t) && Symbol.prototype.valueOf.call(e) === Symbol.prototype.valueOf.call(t);
  }
  function Ee(e, t, r, n) {
    if (e === t)
      return e !== 0 ? true : r ? zh(e, t) : true;
    if (r) {
      if (ce(e) !== "object")
        return typeof e == "number" && xf(e) && xf(t);
      if (ce(t) !== "object" || e === null || t === null || Object.getPrototypeOf(e) !== Object.getPrototypeOf(t))
        return false;
    } else {
      if (e === null || ce(e) !== "object")
        return t === null || ce(t) !== "object" ? e == t : false;
      if (t === null || ce(t) !== "object")
        return false;
    }
    var i = Fh(e), o = Fh(t);
    if (i !== o)
      return false;
    if (Array.isArray(e)) {
      if (e.length !== t.length)
        return false;
      var a = Hi(e, Vi), f = Hi(t, Vi);
      return a.length !== f.length ? false : sn(e, t, r, n, Tf, a);
    }
    if (i === "[object Object]" && (!$i(e) && $i(t) || !Gi(e) && Gi(t)))
      return false;
    if (Lh(e)) {
      if (!Lh(t) || Date.prototype.getTime.call(e) !== Date.prototype.getTime.call(t))
        return false;
    } else if (Dh(e)) {
      if (!Dh(t) || !bS(e, t))
        return false;
    } else if (sS(e) || e instanceof Error) {
      if (e.message !== t.message || e.name !== t.name)
        return false;
    } else if (uS(e)) {
      if (!r && (hS(e) || pS(e))) {
        if (!wS(e, t))
          return false;
      } else if (!ES(e, t))
        return false;
      var u = Hi(e, Vi), l = Hi(t, Vi);
      return u.length !== l.length ? false : sn(e, t, r, n, Rf, u);
    } else {
      if (Gi(e))
        return !Gi(t) || e.size !== t.size ? false : sn(e, t, r, n, Zh);
      if ($i(e))
        return !$i(t) || e.size !== t.size ? false : sn(e, t, r, n, $h);
      if (lS(e)) {
        if (!vS(e, t))
          return false;
      } else if (cS(e) && !mS(e, t))
        return false;
    }
    return sn(e, t, r, n, Rf);
  }
  function Uh(e, t) {
    return t.filter(function(r) {
      return Ki(e, r);
    });
  }
  function sn(e, t, r, n, i, o) {
    if (arguments.length === 5) {
      o = Object.keys(e);
      var a = Object.keys(t);
      if (o.length !== a.length)
        return false;
    }
    for (var f = 0;f < o.length; f++)
      if (!cn(t, o[f]))
        return false;
    if (r && arguments.length === 5) {
      var u = Yi(e);
      if (u.length !== 0) {
        var l = 0;
        for (f = 0;f < u.length; f++) {
          var s = u[f];
          if (Ki(e, s)) {
            if (!Ki(t, s))
              return false;
            o.push(s), l++;
          } else if (Ki(t, s))
            return false;
        }
        var c = Yi(t);
        if (u.length !== c.length && Uh(t, c).length !== l)
          return false;
      } else {
        var h = Yi(t);
        if (h.length !== 0 && Uh(t, h).length !== 0)
          return false;
      }
    }
    if (o.length === 0 && (i === Rf || i === Tf && e.length === 0 || e.size === 0))
      return true;
    if (n === undefined)
      n = { val1: new Map, val2: new Map, position: 0 };
    else {
      var d = n.val1.get(e);
      if (d !== undefined) {
        var y = n.val2.get(t);
        if (y !== undefined)
          return d === y;
      }
      n.position++;
    }
    n.val1.set(e, n.position), n.val2.set(t, n.position);
    var b = IS(e, t, r, o, n, i);
    return n.val1.delete(e), n.val2.delete(t), b;
  }
  function qh(e, t, r, n) {
    for (var i = Xi(e), o = 0;o < i.length; o++) {
      var a = i[o];
      if (Ee(t, a, r, n))
        return e.delete(a), true;
    }
    return false;
  }
  function Gh(e) {
    switch (ce(e)) {
      case "undefined":
        return null;
      case "object":
        return;
      case "symbol":
        return false;
      case "string":
        e = +e;
      case "number":
        if (xf(e))
          return false;
    }
    return true;
  }
  function SS(e, t, r) {
    var n = Gh(r);
    return n ?? (t.has(n) && !e.has(n));
  }
  function AS(e, t, r, n, i) {
    var o = Gh(r);
    if (o != null)
      return o;
    var a = t.get(o);
    return a === undefined && !t.has(o) || !Ee(n, a, false, i) ? false : !e.has(o) && Ee(n, a, false, i);
  }
  function xS(e, t, r, n) {
    for (var i = null, o = Xi(e), a = 0;a < o.length; a++) {
      var f = o[a];
      if (ce(f) === "object" && f !== null)
        i === null && (i = new Set), i.add(f);
      else if (!t.has(f)) {
        if (r || !SS(e, t, f))
          return false;
        i === null && (i = new Set), i.add(f);
      }
    }
    if (i !== null) {
      for (var u = Xi(t), l = 0;l < u.length; l++) {
        var s = u[l];
        if (ce(s) === "object" && s !== null) {
          if (!qh(i, s, r, n))
            return false;
        } else if (!r && !e.has(s) && !qh(i, s, r, n))
          return false;
      }
      return i.size === 0;
    }
    return true;
  }
  function Ch(e, t, r, n, i, o) {
    for (var a = Xi(e), f = 0;f < a.length; f++) {
      var u = a[f];
      if (Ee(r, u, i, o) && Ee(n, t.get(u), i, o))
        return e.delete(u), true;
    }
    return false;
  }
  function RS(e, t, r, n) {
    for (var i = null, o = kh(e), a = 0;a < o.length; a++) {
      var f = Nh(o[a], 2), u = f[0], l = f[1];
      if (ce(u) === "object" && u !== null)
        i === null && (i = new Set), i.add(u);
      else {
        var s = t.get(u);
        if (s === undefined && !t.has(u) || !Ee(l, s, r, n)) {
          if (r || !AS(e, t, u, l, n))
            return false;
          i === null && (i = new Set), i.add(u);
        }
      }
    }
    if (i !== null) {
      for (var c = kh(t), h = 0;h < c.length; h++) {
        var d = Nh(c[h], 2), u = d[0], y = d[1];
        if (ce(u) === "object" && u !== null) {
          if (!Ch(i, e, u, y, r, n))
            return false;
        } else if (!r && (!e.has(u) || !Ee(e.get(u), y, false, n)) && !Ch(i, e, u, y, false, n))
          return false;
      }
      return i.size === 0;
    }
    return true;
  }
  function IS(e, t, r, n, i, o) {
    var a = 0;
    if (o === Zh) {
      if (!xS(e, t, r, i))
        return false;
    } else if (o === $h) {
      if (!RS(e, t, r, i))
        return false;
    } else if (o === Tf)
      for (;a < e.length; a++)
        if (cn(e, a)) {
          if (!cn(t, a) || !Ee(e[a], t[a], r, i))
            return false;
        } else {
          if (cn(t, a))
            return false;
          for (var f = Object.keys(e);a < f.length; a++) {
            var u = f[a];
            if (!cn(t, u) || !Ee(e[u], t[u], r, i))
              return false;
          }
          return f.length === Object.keys(t).length;
        }
    for (a = 0;a < n.length; a++) {
      var l = n[a];
      if (!Ee(e[l], t[l], r, i))
        return false;
    }
    return true;
  }
  function TS(e, t) {
    return Ee(e, t, gS);
  }
  function OS(e, t) {
    return Ee(e, t, _S);
  }
  Hh.exports = { isDeepEqual: TS, isDeepStrictEqual: OS };
});
var tn = g((zx, sp) => {
  function ct(e) {
    return typeof Symbol == "function" && typeof Symbol.iterator == "symbol" ? ct = function(r) {
      return typeof r;
    } : ct = function(r) {
      return r && typeof Symbol == "function" && r.constructor === Symbol && r !== Symbol.prototype ? "symbol" : typeof r;
    }, ct(e);
  }
  function NS(e, t) {
    if (!(e instanceof t))
      throw new TypeError("Cannot call a class as a function");
  }
  var kS = cf(), hn = kS.codes, Yh = hn.ERR_AMBIGUOUS_ARGUMENT, dn = hn.ERR_INVALID_ARG_TYPE, FS = hn.ERR_INVALID_ARG_VALUE, LS = hn.ERR_INVALID_RETURN_VALUE, ht = hn.ERR_MISSING_ARGS, Ut = Wd(), DS = (Ir(), se(be)), BS = DS.inspect, Qh = (Ir(), se(be)).types, PS = Qh.isPromise, Of = Qh.isRegExp, MS = Object.assign ? Object.assign : Gd().assign, ep = Object.is ? Object.is : mf(), dt, Ji;
  function pn() {
    var e = Vh();
    dt = e.isDeepEqual, Ji = e.isDeepStrictEqual;
  }
  var Kh = false, q = sp.exports = Nf, Qi = {};
  function ke(e) {
    throw e.message instanceof Error ? e.message : new Ut(e);
  }
  function tp(e, t, r, n, i) {
    var o = arguments.length, a;
    if (o === 0)
      a = "Failed";
    else if (o === 1)
      r = e, e = undefined;
    else {
      if (Kh === false) {
        Kh = true;
        var f = process.emitWarning ? process.emitWarning : console.warn.bind(console);
        f("assert.fail() with more than one argument is deprecated. Please use assert.strictEqual() instead or only pass a message.", "DeprecationWarning", "DEP0094");
      }
      o === 2 && (n = "!=");
    }
    if (r instanceof Error)
      throw r;
    var u = { actual: e, expected: t, operator: n === undefined ? "fail" : n, stackStartFn: i || tp };
    r !== undefined && (u.message = r);
    var l = new Ut(u);
    throw a && (l.message = a, l.generatedMessage = true), l;
  }
  q.fail = tp;
  q.AssertionError = Ut;
  function rp(e, t, r, n) {
    if (!r) {
      var i = false;
      if (t === 0)
        i = true, n = "No value argument passed to `assert.ok()`";
      else if (n instanceof Error)
        throw n;
      var o = new Ut({ actual: r, expected: true, message: n, operator: "==", stackStartFn: e });
      throw o.generatedMessage = i, o;
    }
  }
  function Nf() {
    for (var e = arguments.length, t = new Array(e), r = 0;r < e; r++)
      t[r] = arguments[r];
    rp.apply(undefined, [Nf, t.length].concat(t));
  }
  q.ok = Nf;
  q.equal = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    t != r && ke({ actual: t, expected: r, message: n, operator: "==", stackStartFn: e });
  };
  q.notEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    t == r && ke({ actual: t, expected: r, message: n, operator: "!=", stackStartFn: e });
  };
  q.deepEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    dt === undefined && pn(), dt(t, r) || ke({ actual: t, expected: r, message: n, operator: "deepEqual", stackStartFn: e });
  };
  q.notDeepEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    dt === undefined && pn(), dt(t, r) && ke({ actual: t, expected: r, message: n, operator: "notDeepEqual", stackStartFn: e });
  };
  q.deepStrictEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    dt === undefined && pn(), Ji(t, r) || ke({ actual: t, expected: r, message: n, operator: "deepStrictEqual", stackStartFn: e });
  };
  q.notDeepStrictEqual = np;
  function np(e, t, r) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    dt === undefined && pn(), Ji(e, t) && ke({ actual: e, expected: t, message: r, operator: "notDeepStrictEqual", stackStartFn: np });
  }
  q.strictEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    ep(t, r) || ke({ actual: t, expected: r, message: n, operator: "strictEqual", stackStartFn: e });
  };
  q.notStrictEqual = function e(t, r, n) {
    if (arguments.length < 2)
      throw new ht("actual", "expected");
    ep(t, r) && ke({ actual: t, expected: r, message: n, operator: "notStrictEqual", stackStartFn: e });
  };
  var Xh = function e(t, r, n) {
    var i = this;
    NS(this, e), r.forEach(function(o) {
      o in t && (n !== undefined && typeof n[o] == "string" && Of(t[o]) && t[o].test(n[o]) ? i[o] = n[o] : i[o] = t[o]);
    });
  };
  function jS(e, t, r, n, i, o) {
    if (!(r in e) || !Ji(e[r], t[r])) {
      if (!n) {
        var a = new Xh(e, i), f = new Xh(t, i, e), u = new Ut({ actual: a, expected: f, operator: "deepStrictEqual", stackStartFn: o });
        throw u.actual = e, u.expected = t, u.operator = o.name, u;
      }
      ke({ actual: e, expected: t, message: n, operator: o.name, stackStartFn: o });
    }
  }
  function ip(e, t, r, n) {
    if (typeof t != "function") {
      if (Of(t))
        return t.test(e);
      if (arguments.length === 2)
        throw new dn("expected", ["Function", "RegExp"], t);
      if (ct(e) !== "object" || e === null) {
        var i = new Ut({ actual: e, expected: t, message: r, operator: "deepStrictEqual", stackStartFn: n });
        throw i.operator = n.name, i;
      }
      var o = Object.keys(t);
      if (t instanceof Error)
        o.push("name", "message");
      else if (o.length === 0)
        throw new FS("error", t, "may not be an empty object");
      return dt === undefined && pn(), o.forEach(function(a) {
        typeof e[a] == "string" && Of(t[a]) && t[a].test(e[a]) || jS(e, t, a, r, o, n);
      }), true;
    }
    return t.prototype !== undefined && e instanceof t ? true : Error.isPrototypeOf(t) ? false : t.call({}, e) === true;
  }
  function op(e) {
    if (typeof e != "function")
      throw new dn("fn", "Function", e);
    try {
      e();
    } catch (t) {
      return t;
    }
    return Qi;
  }
  function Jh(e) {
    return PS(e) || e !== null && ct(e) === "object" && typeof e.then == "function" && typeof e.catch == "function";
  }
  function ap(e) {
    return Promise.resolve().then(function() {
      var t;
      if (typeof e == "function") {
        if (t = e(), !Jh(t))
          throw new LS("instance of Promise", "promiseFn", t);
      } else if (Jh(e))
        t = e;
      else
        throw new dn("promiseFn", ["Function", "Promise"], e);
      return Promise.resolve().then(function() {
        return t;
      }).then(function() {
        return Qi;
      }).catch(function(r) {
        return r;
      });
    });
  }
  function fp(e, t, r, n) {
    if (typeof r == "string") {
      if (arguments.length === 4)
        throw new dn("error", ["Object", "Error", "Function", "RegExp"], r);
      if (ct(t) === "object" && t !== null) {
        if (t.message === r)
          throw new Yh("error/message", 'The error message "'.concat(t.message, '" is identical to the message.'));
      } else if (t === r)
        throw new Yh("error/message", 'The error "'.concat(t, '" is identical to the message.'));
      n = r, r = undefined;
    } else if (r != null && ct(r) !== "object" && typeof r != "function")
      throw new dn("error", ["Object", "Error", "Function", "RegExp"], r);
    if (t === Qi) {
      var i = "";
      r && r.name && (i += " (".concat(r.name, ")")), i += n ? ": ".concat(n) : ".";
      var o = e.name === "rejects" ? "rejection" : "exception";
      ke({ actual: undefined, expected: r, operator: e.name, message: "Missing expected ".concat(o).concat(i), stackStartFn: e });
    }
    if (r && !ip(t, r, n, e))
      throw t;
  }
  function lp(e, t, r, n) {
    if (t !== Qi) {
      if (typeof r == "string" && (n = r, r = undefined), !r || ip(t, r)) {
        var i = n ? ": ".concat(n) : ".", o = e.name === "doesNotReject" ? "rejection" : "exception";
        ke({ actual: t, expected: r, operator: e.name, message: "Got unwanted ".concat(o).concat(i, `
`) + 'Actual message: "'.concat(t && t.message, '"'), stackStartFn: e });
      }
      throw t;
    }
  }
  q.throws = function e(t) {
    for (var r = arguments.length, n = new Array(r > 1 ? r - 1 : 0), i = 1;i < r; i++)
      n[i - 1] = arguments[i];
    fp.apply(undefined, [e, op(t)].concat(n));
  };
  q.rejects = function e(t) {
    for (var r = arguments.length, n = new Array(r > 1 ? r - 1 : 0), i = 1;i < r; i++)
      n[i - 1] = arguments[i];
    return ap(t).then(function(o) {
      return fp.apply(undefined, [e, o].concat(n));
    });
  };
  q.doesNotThrow = function e(t) {
    for (var r = arguments.length, n = new Array(r > 1 ? r - 1 : 0), i = 1;i < r; i++)
      n[i - 1] = arguments[i];
    lp.apply(undefined, [e, op(t)].concat(n));
  };
  q.doesNotReject = function e(t) {
    for (var r = arguments.length, n = new Array(r > 1 ? r - 1 : 0), i = 1;i < r; i++)
      n[i - 1] = arguments[i];
    return ap(t).then(function(o) {
      return lp.apply(undefined, [e, o].concat(n));
    });
  };
  q.ifError = function e(t) {
    if (t != null) {
      var r = "ifError got unwanted exception: ";
      ct(t) === "object" && typeof t.message == "string" ? t.message.length === 0 && t.constructor ? r += t.constructor.name : r += t.message : r += BS(t);
      var n = new Ut({ actual: t, expected: null, operator: "ifError", message: r, stackStartFn: e }), i = t.stack;
      if (typeof i == "string") {
        var o = i.split(`
`);
        o.shift();
        for (var a = n.stack.split(`
`), f = 0;f < o.length; f++) {
          var u = a.indexOf(o[f]);
          if (u !== -1) {
            a = a.slice(0, u);
            break;
          }
        }
        n.stack = "".concat(a.join(`
`), `
`).concat(o.join(`
`));
      }
      throw n;
    }
  };
  function up() {
    for (var e = arguments.length, t = new Array(e), r = 0;r < e; r++)
      t[r] = arguments[r];
    rp.apply(undefined, [up, t.length].concat(t));
  }
  q.strict = MS(up, q, { equal: q.strictEqual, deepEqual: q.deepStrictEqual, notEqual: q.notStrictEqual, notDeepEqual: q.notDeepStrictEqual });
  q.strict.strict = q.strict;
});
var dp = g((Wx, cp) => {
  function US() {
    this.input = null, this.next_in = 0, this.avail_in = 0, this.total_in = 0, this.output = null, this.next_out = 0, this.avail_out = 0, this.total_out = 0, this.msg = "", this.state = null, this.data_type = 2, this.adler = 0;
  }
  cp.exports = US;
});
var yn = g((oe) => {
  var qS = typeof Uint8Array < "u" && typeof Uint16Array < "u" && typeof Int32Array < "u";
  function CS(e, t) {
    return Object.prototype.hasOwnProperty.call(e, t);
  }
  oe.assign = function(e) {
    for (var t = Array.prototype.slice.call(arguments, 1);t.length; ) {
      var r = t.shift();
      if (!!r) {
        if (typeof r != "object")
          throw new TypeError(r + "must be non-object");
        for (var n in r)
          CS(r, n) && (e[n] = r[n]);
      }
    }
    return e;
  };
  oe.shrinkBuf = function(e, t) {
    return e.length === t ? e : e.subarray ? e.subarray(0, t) : (e.length = t, e);
  };
  var zS = { arraySet: function(e, t, r, n, i) {
    if (t.subarray && e.subarray) {
      e.set(t.subarray(r, r + n), i);
      return;
    }
    for (var o = 0;o < n; o++)
      e[i + o] = t[r + o];
  }, flattenChunks: function(e) {
    var t, r, n, i, o, a;
    for (n = 0, t = 0, r = e.length;t < r; t++)
      n += e[t].length;
    for (a = new Uint8Array(n), i = 0, t = 0, r = e.length;t < r; t++)
      o = e[t], a.set(o, i), i += o.length;
    return a;
  } }, WS = { arraySet: function(e, t, r, n, i) {
    for (var o = 0;o < n; o++)
      e[i + o] = t[r + o];
  }, flattenChunks: function(e) {
    return [].concat.apply([], e);
  } };
  oe.setTyped = function(e) {
    e ? (oe.Buf8 = Uint8Array, oe.Buf16 = Uint16Array, oe.Buf32 = Int32Array, oe.assign(oe, zS)) : (oe.Buf8 = Array, oe.Buf16 = Array, oe.Buf32 = Array, oe.assign(oe, WS));
  };
  oe.setTyped(qS);
});
var Bp = g((Lr) => {
  var ZS = yn(), $S = 4, hp = 0, pp = 1, GS = 2;
  function Fr(e) {
    for (var t = e.length;--t >= 0; )
      e[t] = 0;
  }
  var HS = 0, Ep = 1, VS = 2, YS = 3, KS = 258, Mf = 29, vn = 256, gn = vn + 1 + Mf, kr = 30, jf = 19, vp = 2 * gn + 1, qt = 15, kf = 16, XS = 7, Uf = 256, mp = 16, Sp = 17, Ap = 18, Bf = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0], eo = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13], JS = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7], xp = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15], QS = 512, tt = new Array((gn + 2) * 2);
  Fr(tt);
  var _n = new Array(kr * 2);
  Fr(_n);
  var bn = new Array(QS);
  Fr(bn);
  var wn = new Array(KS - YS + 1);
  Fr(wn);
  var qf = new Array(Mf);
  Fr(qf);
  var to = new Array(kr);
  Fr(to);
  function Ff(e, t, r, n, i) {
    this.static_tree = e, this.extra_bits = t, this.extra_base = r, this.elems = n, this.max_length = i, this.has_stree = e && e.length;
  }
  var Rp, Ip, Tp;
  function Lf(e, t) {
    this.dyn_tree = e, this.max_code = 0, this.stat_desc = t;
  }
  function Op(e) {
    return e < 256 ? bn[e] : bn[256 + (e >>> 7)];
  }
  function En(e, t) {
    e.pending_buf[e.pending++] = t & 255, e.pending_buf[e.pending++] = t >>> 8 & 255;
  }
  function ue(e, t, r) {
    e.bi_valid > kf - r ? (e.bi_buf |= t << e.bi_valid & 65535, En(e, e.bi_buf), e.bi_buf = t >> kf - e.bi_valid, e.bi_valid += r - kf) : (e.bi_buf |= t << e.bi_valid & 65535, e.bi_valid += r);
  }
  function We(e, t, r) {
    ue(e, r[t * 2], r[t * 2 + 1]);
  }
  function Np(e, t) {
    var r = 0;
    do
      r |= e & 1, e >>>= 1, r <<= 1;
    while (--t > 0);
    return r >>> 1;
  }
  function e1(e) {
    e.bi_valid === 16 ? (En(e, e.bi_buf), e.bi_buf = 0, e.bi_valid = 0) : e.bi_valid >= 8 && (e.pending_buf[e.pending++] = e.bi_buf & 255, e.bi_buf >>= 8, e.bi_valid -= 8);
  }
  function t1(e, t) {
    var { dyn_tree: r, max_code: n } = t, i = t.stat_desc.static_tree, o = t.stat_desc.has_stree, a = t.stat_desc.extra_bits, f = t.stat_desc.extra_base, u = t.stat_desc.max_length, l, s, c, h, d, y, b = 0;
    for (h = 0;h <= qt; h++)
      e.bl_count[h] = 0;
    for (r[e.heap[e.heap_max] * 2 + 1] = 0, l = e.heap_max + 1;l < vp; l++)
      s = e.heap[l], h = r[r[s * 2 + 1] * 2 + 1] + 1, h > u && (h = u, b++), r[s * 2 + 1] = h, !(s > n) && (e.bl_count[h]++, d = 0, s >= f && (d = a[s - f]), y = r[s * 2], e.opt_len += y * (h + d), o && (e.static_len += y * (i[s * 2 + 1] + d)));
    if (b !== 0) {
      do {
        for (h = u - 1;e.bl_count[h] === 0; )
          h--;
        e.bl_count[h]--, e.bl_count[h + 1] += 2, e.bl_count[u]--, b -= 2;
      } while (b > 0);
      for (h = u;h !== 0; h--)
        for (s = e.bl_count[h];s !== 0; )
          c = e.heap[--l], !(c > n) && (r[c * 2 + 1] !== h && (e.opt_len += (h - r[c * 2 + 1]) * r[c * 2], r[c * 2 + 1] = h), s--);
    }
  }
  function kp(e, t, r) {
    var n = new Array(qt + 1), i = 0, o, a;
    for (o = 1;o <= qt; o++)
      n[o] = i = i + r[o - 1] << 1;
    for (a = 0;a <= t; a++) {
      var f = e[a * 2 + 1];
      f !== 0 && (e[a * 2] = Np(n[f]++, f));
    }
  }
  function r1() {
    var e, t, r, n, i, o = new Array(qt + 1);
    for (r = 0, n = 0;n < Mf - 1; n++)
      for (qf[n] = r, e = 0;e < 1 << Bf[n]; e++)
        wn[r++] = n;
    for (wn[r - 1] = n, i = 0, n = 0;n < 16; n++)
      for (to[n] = i, e = 0;e < 1 << eo[n]; e++)
        bn[i++] = n;
    for (i >>= 7;n < kr; n++)
      for (to[n] = i << 7, e = 0;e < 1 << eo[n] - 7; e++)
        bn[256 + i++] = n;
    for (t = 0;t <= qt; t++)
      o[t] = 0;
    for (e = 0;e <= 143; )
      tt[e * 2 + 1] = 8, e++, o[8]++;
    for (;e <= 255; )
      tt[e * 2 + 1] = 9, e++, o[9]++;
    for (;e <= 279; )
      tt[e * 2 + 1] = 7, e++, o[7]++;
    for (;e <= 287; )
      tt[e * 2 + 1] = 8, e++, o[8]++;
    for (kp(tt, gn + 1, o), e = 0;e < kr; e++)
      _n[e * 2 + 1] = 5, _n[e * 2] = Np(e, 5);
    Rp = new Ff(tt, Bf, vn + 1, gn, qt), Ip = new Ff(_n, eo, 0, kr, qt), Tp = new Ff(new Array(0), JS, 0, jf, XS);
  }
  function Fp(e) {
    var t;
    for (t = 0;t < gn; t++)
      e.dyn_ltree[t * 2] = 0;
    for (t = 0;t < kr; t++)
      e.dyn_dtree[t * 2] = 0;
    for (t = 0;t < jf; t++)
      e.bl_tree[t * 2] = 0;
    e.dyn_ltree[Uf * 2] = 1, e.opt_len = e.static_len = 0, e.last_lit = e.matches = 0;
  }
  function Lp(e) {
    e.bi_valid > 8 ? En(e, e.bi_buf) : e.bi_valid > 0 && (e.pending_buf[e.pending++] = e.bi_buf), e.bi_buf = 0, e.bi_valid = 0;
  }
  function n1(e, t, r, n) {
    Lp(e), n && (En(e, r), En(e, ~r)), ZS.arraySet(e.pending_buf, e.window, t, r, e.pending), e.pending += r;
  }
  function yp(e, t, r, n) {
    var i = t * 2, o = r * 2;
    return e[i] < e[o] || e[i] === e[o] && n[t] <= n[r];
  }
  function Df(e, t, r) {
    for (var n = e.heap[r], i = r << 1;i <= e.heap_len && (i < e.heap_len && yp(t, e.heap[i + 1], e.heap[i], e.depth) && i++, !yp(t, n, e.heap[i], e.depth)); )
      e.heap[r] = e.heap[i], r = i, i <<= 1;
    e.heap[r] = n;
  }
  function _p(e, t, r) {
    var n, i, o = 0, a, f;
    if (e.last_lit !== 0)
      do
        n = e.pending_buf[e.d_buf + o * 2] << 8 | e.pending_buf[e.d_buf + o * 2 + 1], i = e.pending_buf[e.l_buf + o], o++, n === 0 ? We(e, i, t) : (a = wn[i], We(e, a + vn + 1, t), f = Bf[a], f !== 0 && (i -= qf[a], ue(e, i, f)), n--, a = Op(n), We(e, a, r), f = eo[a], f !== 0 && (n -= to[a], ue(e, n, f)));
      while (o < e.last_lit);
    We(e, Uf, t);
  }
  function Pf(e, t) {
    var r = t.dyn_tree, n = t.stat_desc.static_tree, i = t.stat_desc.has_stree, o = t.stat_desc.elems, a, f, u = -1, l;
    for (e.heap_len = 0, e.heap_max = vp, a = 0;a < o; a++)
      r[a * 2] !== 0 ? (e.heap[++e.heap_len] = u = a, e.depth[a] = 0) : r[a * 2 + 1] = 0;
    for (;e.heap_len < 2; )
      l = e.heap[++e.heap_len] = u < 2 ? ++u : 0, r[l * 2] = 1, e.depth[l] = 0, e.opt_len--, i && (e.static_len -= n[l * 2 + 1]);
    for (t.max_code = u, a = e.heap_len >> 1;a >= 1; a--)
      Df(e, r, a);
    l = o;
    do
      a = e.heap[1], e.heap[1] = e.heap[e.heap_len--], Df(e, r, 1), f = e.heap[1], e.heap[--e.heap_max] = a, e.heap[--e.heap_max] = f, r[l * 2] = r[a * 2] + r[f * 2], e.depth[l] = (e.depth[a] >= e.depth[f] ? e.depth[a] : e.depth[f]) + 1, r[a * 2 + 1] = r[f * 2 + 1] = l, e.heap[1] = l++, Df(e, r, 1);
    while (e.heap_len >= 2);
    e.heap[--e.heap_max] = e.heap[1], t1(e, t), kp(r, u, e.bl_count);
  }
  function gp(e, t, r) {
    var n, i = -1, o, a = t[0 * 2 + 1], f = 0, u = 7, l = 4;
    for (a === 0 && (u = 138, l = 3), t[(r + 1) * 2 + 1] = 65535, n = 0;n <= r; n++)
      o = a, a = t[(n + 1) * 2 + 1], !(++f < u && o === a) && (f < l ? e.bl_tree[o * 2] += f : o !== 0 ? (o !== i && e.bl_tree[o * 2]++, e.bl_tree[mp * 2]++) : f <= 10 ? e.bl_tree[Sp * 2]++ : e.bl_tree[Ap * 2]++, f = 0, i = o, a === 0 ? (u = 138, l = 3) : o === a ? (u = 6, l = 3) : (u = 7, l = 4));
  }
  function bp(e, t, r) {
    var n, i = -1, o, a = t[0 * 2 + 1], f = 0, u = 7, l = 4;
    for (a === 0 && (u = 138, l = 3), n = 0;n <= r; n++)
      if (o = a, a = t[(n + 1) * 2 + 1], !(++f < u && o === a)) {
        if (f < l)
          do
            We(e, o, e.bl_tree);
          while (--f !== 0);
        else
          o !== 0 ? (o !== i && (We(e, o, e.bl_tree), f--), We(e, mp, e.bl_tree), ue(e, f - 3, 2)) : f <= 10 ? (We(e, Sp, e.bl_tree), ue(e, f - 3, 3)) : (We(e, Ap, e.bl_tree), ue(e, f - 11, 7));
        f = 0, i = o, a === 0 ? (u = 138, l = 3) : o === a ? (u = 6, l = 3) : (u = 7, l = 4);
      }
  }
  function i1(e) {
    var t;
    for (gp(e, e.dyn_ltree, e.l_desc.max_code), gp(e, e.dyn_dtree, e.d_desc.max_code), Pf(e, e.bl_desc), t = jf - 1;t >= 3 && e.bl_tree[xp[t] * 2 + 1] === 0; t--)
      ;
    return e.opt_len += 3 * (t + 1) + 5 + 5 + 4, t;
  }
  function o1(e, t, r, n) {
    var i;
    for (ue(e, t - 257, 5), ue(e, r - 1, 5), ue(e, n - 4, 4), i = 0;i < n; i++)
      ue(e, e.bl_tree[xp[i] * 2 + 1], 3);
    bp(e, e.dyn_ltree, t - 1), bp(e, e.dyn_dtree, r - 1);
  }
  function a1(e) {
    var t = 4093624447, r;
    for (r = 0;r <= 31; r++, t >>>= 1)
      if (t & 1 && e.dyn_ltree[r * 2] !== 0)
        return hp;
    if (e.dyn_ltree[9 * 2] !== 0 || e.dyn_ltree[10 * 2] !== 0 || e.dyn_ltree[13 * 2] !== 0)
      return pp;
    for (r = 32;r < vn; r++)
      if (e.dyn_ltree[r * 2] !== 0)
        return pp;
    return hp;
  }
  var wp = false;
  function f1(e) {
    wp || (r1(), wp = true), e.l_desc = new Lf(e.dyn_ltree, Rp), e.d_desc = new Lf(e.dyn_dtree, Ip), e.bl_desc = new Lf(e.bl_tree, Tp), e.bi_buf = 0, e.bi_valid = 0, Fp(e);
  }
  function Dp(e, t, r, n) {
    ue(e, (HS << 1) + (n ? 1 : 0), 3), n1(e, t, r, true);
  }
  function l1(e) {
    ue(e, Ep << 1, 3), We(e, Uf, tt), e1(e);
  }
  function u1(e, t, r, n) {
    var i, o, a = 0;
    e.level > 0 ? (e.strm.data_type === GS && (e.strm.data_type = a1(e)), Pf(e, e.l_desc), Pf(e, e.d_desc), a = i1(e), i = e.opt_len + 3 + 7 >>> 3, o = e.static_len + 3 + 7 >>> 3, o <= i && (i = o)) : i = o = r + 5, r + 4 <= i && t !== -1 ? Dp(e, t, r, n) : e.strategy === $S || o === i ? (ue(e, (Ep << 1) + (n ? 1 : 0), 3), _p(e, tt, _n)) : (ue(e, (VS << 1) + (n ? 1 : 0), 3), o1(e, e.l_desc.max_code + 1, e.d_desc.max_code + 1, a + 1), _p(e, e.dyn_ltree, e.dyn_dtree)), Fp(e), n && Lp(e);
  }
  function s1(e, t, r) {
    return e.pending_buf[e.d_buf + e.last_lit * 2] = t >>> 8 & 255, e.pending_buf[e.d_buf + e.last_lit * 2 + 1] = t & 255, e.pending_buf[e.l_buf + e.last_lit] = r & 255, e.last_lit++, t === 0 ? e.dyn_ltree[r * 2]++ : (e.matches++, t--, e.dyn_ltree[(wn[r] + vn + 1) * 2]++, e.dyn_dtree[Op(t) * 2]++), e.last_lit === e.lit_bufsize - 1;
  }
  Lr._tr_init = f1;
  Lr._tr_stored_block = Dp;
  Lr._tr_flush_block = u1;
  Lr._tr_tally = s1;
  Lr._tr_align = l1;
});
var Cf = g((Gx, Pp) => {
  function c1(e, t, r, n) {
    for (var i = e & 65535 | 0, o = e >>> 16 & 65535 | 0, a = 0;r !== 0; ) {
      a = r > 2000 ? 2000 : r, r -= a;
      do
        i = i + t[n++] | 0, o = o + i | 0;
      while (--a);
      i %= 65521, o %= 65521;
    }
    return i | o << 16 | 0;
  }
  Pp.exports = c1;
});
var zf = g((Hx, Mp) => {
  function d1() {
    for (var e, t = [], r = 0;r < 256; r++) {
      e = r;
      for (var n = 0;n < 8; n++)
        e = e & 1 ? 3988292384 ^ e >>> 1 : e >>> 1;
      t[r] = e;
    }
    return t;
  }
  var h1 = d1();
  function p1(e, t, r, n) {
    var i = h1, o = n + r;
    e ^= -1;
    for (var a = n;a < o; a++)
      e = e >>> 8 ^ i[(e ^ t[a]) & 255];
    return e ^ -1;
  }
  Mp.exports = p1;
});
var Up = g((Vx, jp) => {
  jp.exports = { 2: "need dictionary", 1: "stream end", 0: "", "-1": "file error", "-2": "stream error", "-3": "data error", "-4": "insufficient memory", "-5": "buffer error", "-6": "incompatible version" };
});
var Vp = g((Ge) => {
  var ae = yn(), ve = Bp(), Wp = Cf(), pt = zf(), y1 = Up(), Zt = 0, _1 = 1, g1 = 3, wt = 4, qp = 5, $e = 0, Cp = 1, me = -2, b1 = -3, Wf = -5, w1 = -1, E1 = 1, ro = 2, v1 = 3, m1 = 4, S1 = 0, A1 = 2, ao = 8, x1 = 9, R1 = 15, I1 = 8, T1 = 29, O1 = 256, $f = O1 + 1 + T1, N1 = 30, k1 = 19, F1 = 2 * $f + 1, L1 = 15, M = 3, gt = 258, Fe = gt + M + 1, D1 = 32, fo = 42, Gf = 69, no = 73, io = 91, oo = 103, Ct = 113, Sn = 666, K = 1, An = 2, zt = 3, Pr = 4, B1 = 3;
  function bt(e, t) {
    return e.msg = y1[t], t;
  }
  function zp(e) {
    return (e << 1) - (e > 4 ? 9 : 0);
  }
  function _t(e) {
    for (var t = e.length;--t >= 0; )
      e[t] = 0;
  }
  function yt(e) {
    var t = e.state, r = t.pending;
    r > e.avail_out && (r = e.avail_out), r !== 0 && (ae.arraySet(e.output, t.pending_buf, t.pending_out, r, e.next_out), e.next_out += r, t.pending_out += r, e.total_out += r, e.avail_out -= r, t.pending -= r, t.pending === 0 && (t.pending_out = 0));
  }
  function te(e, t) {
    ve._tr_flush_block(e, e.block_start >= 0 ? e.block_start : -1, e.strstart - e.block_start, t), e.block_start = e.strstart, yt(e.strm);
  }
  function j(e, t) {
    e.pending_buf[e.pending++] = t;
  }
  function mn(e, t) {
    e.pending_buf[e.pending++] = t >>> 8 & 255, e.pending_buf[e.pending++] = t & 255;
  }
  function P1(e, t, r, n) {
    var i = e.avail_in;
    return i > n && (i = n), i === 0 ? 0 : (e.avail_in -= i, ae.arraySet(t, e.input, e.next_in, i, r), e.state.wrap === 1 ? e.adler = Wp(e.adler, t, i, r) : e.state.wrap === 2 && (e.adler = pt(e.adler, t, i, r)), e.next_in += i, e.total_in += i, i);
  }
  function Zp(e, t) {
    var { max_chain_length: r, strstart: n } = e, i, o, a = e.prev_length, f = e.nice_match, u = e.strstart > e.w_size - Fe ? e.strstart - (e.w_size - Fe) : 0, l = e.window, s = e.w_mask, c = e.prev, h = e.strstart + gt, d = l[n + a - 1], y = l[n + a];
    e.prev_length >= e.good_match && (r >>= 2), f > e.lookahead && (f = e.lookahead);
    do
      if (i = t, !(l[i + a] !== y || l[i + a - 1] !== d || l[i] !== l[n] || l[++i] !== l[n + 1])) {
        n += 2, i++;
        do
          ;
        while (l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && l[++n] === l[++i] && n < h);
        if (o = gt - (h - n), n = h - gt, o > a) {
          if (e.match_start = t, a = o, o >= f)
            break;
          d = l[n + a - 1], y = l[n + a];
        }
      }
    while ((t = c[t & s]) > u && --r !== 0);
    return a <= e.lookahead ? a : e.lookahead;
  }
  function Wt(e) {
    var t = e.w_size, r, n, i, o, a;
    do {
      if (o = e.window_size - e.lookahead - e.strstart, e.strstart >= t + (t - Fe)) {
        ae.arraySet(e.window, e.window, t, t, 0), e.match_start -= t, e.strstart -= t, e.block_start -= t, n = e.hash_size, r = n;
        do
          i = e.head[--r], e.head[r] = i >= t ? i - t : 0;
        while (--n);
        n = t, r = n;
        do
          i = e.prev[--r], e.prev[r] = i >= t ? i - t : 0;
        while (--n);
        o += t;
      }
      if (e.strm.avail_in === 0)
        break;
      if (n = P1(e.strm, e.window, e.strstart + e.lookahead, o), e.lookahead += n, e.lookahead + e.insert >= M)
        for (a = e.strstart - e.insert, e.ins_h = e.window[a], e.ins_h = (e.ins_h << e.hash_shift ^ e.window[a + 1]) & e.hash_mask;e.insert && (e.ins_h = (e.ins_h << e.hash_shift ^ e.window[a + M - 1]) & e.hash_mask, e.prev[a & e.w_mask] = e.head[e.ins_h], e.head[e.ins_h] = a, a++, e.insert--, !(e.lookahead + e.insert < M)); )
          ;
    } while (e.lookahead < Fe && e.strm.avail_in !== 0);
  }
  function M1(e, t) {
    var r = 65535;
    for (r > e.pending_buf_size - 5 && (r = e.pending_buf_size - 5);; ) {
      if (e.lookahead <= 1) {
        if (Wt(e), e.lookahead === 0 && t === Zt)
          return K;
        if (e.lookahead === 0)
          break;
      }
      e.strstart += e.lookahead, e.lookahead = 0;
      var n = e.block_start + r;
      if ((e.strstart === 0 || e.strstart >= n) && (e.lookahead = e.strstart - n, e.strstart = n, te(e, false), e.strm.avail_out === 0) || e.strstart - e.block_start >= e.w_size - Fe && (te(e, false), e.strm.avail_out === 0))
        return K;
    }
    return e.insert = 0, t === wt ? (te(e, true), e.strm.avail_out === 0 ? zt : Pr) : (e.strstart > e.block_start && (te(e, false), e.strm.avail_out === 0), K);
  }
  function Zf(e, t) {
    for (var r, n;; ) {
      if (e.lookahead < Fe) {
        if (Wt(e), e.lookahead < Fe && t === Zt)
          return K;
        if (e.lookahead === 0)
          break;
      }
      if (r = 0, e.lookahead >= M && (e.ins_h = (e.ins_h << e.hash_shift ^ e.window[e.strstart + M - 1]) & e.hash_mask, r = e.prev[e.strstart & e.w_mask] = e.head[e.ins_h], e.head[e.ins_h] = e.strstart), r !== 0 && e.strstart - r <= e.w_size - Fe && (e.match_length = Zp(e, r)), e.match_length >= M)
        if (n = ve._tr_tally(e, e.strstart - e.match_start, e.match_length - M), e.lookahead -= e.match_length, e.match_length <= e.max_lazy_match && e.lookahead >= M) {
          e.match_length--;
          do
            e.strstart++, e.ins_h = (e.ins_h << e.hash_shift ^ e.window[e.strstart + M - 1]) & e.hash_mask, r = e.prev[e.strstart & e.w_mask] = e.head[e.ins_h], e.head[e.ins_h] = e.strstart;
          while (--e.match_length !== 0);
          e.strstart++;
        } else
          e.strstart += e.match_length, e.match_length = 0, e.ins_h = e.window[e.strstart], e.ins_h = (e.ins_h << e.hash_shift ^ e.window[e.strstart + 1]) & e.hash_mask;
      else
        n = ve._tr_tally(e, 0, e.window[e.strstart]), e.lookahead--, e.strstart++;
      if (n && (te(e, false), e.strm.avail_out === 0))
        return K;
    }
    return e.insert = e.strstart < M - 1 ? e.strstart : M - 1, t === wt ? (te(e, true), e.strm.avail_out === 0 ? zt : Pr) : e.last_lit && (te(e, false), e.strm.avail_out === 0) ? K : An;
  }
  function Dr(e, t) {
    for (var r, n, i;; ) {
      if (e.lookahead < Fe) {
        if (Wt(e), e.lookahead < Fe && t === Zt)
          return K;
        if (e.lookahead === 0)
          break;
      }
      if (r = 0, e.lookahead >= M && (e.ins_h = (e.ins_h << e.hash_shift ^ e.window[e.strstart + M - 1]) & e.hash_mask, r = e.prev[e.strstart & e.w_mask] = e.head[e.ins_h], e.head[e.ins_h] = e.strstart), e.prev_length = e.match_length, e.prev_match = e.match_start, e.match_length = M - 1, r !== 0 && e.prev_length < e.max_lazy_match && e.strstart - r <= e.w_size - Fe && (e.match_length = Zp(e, r), e.match_length <= 5 && (e.strategy === E1 || e.match_length === M && e.strstart - e.match_start > 4096) && (e.match_length = M - 1)), e.prev_length >= M && e.match_length <= e.prev_length) {
        i = e.strstart + e.lookahead - M, n = ve._tr_tally(e, e.strstart - 1 - e.prev_match, e.prev_length - M), e.lookahead -= e.prev_length - 1, e.prev_length -= 2;
        do
          ++e.strstart <= i && (e.ins_h = (e.ins_h << e.hash_shift ^ e.window[e.strstart + M - 1]) & e.hash_mask, r = e.prev[e.strstart & e.w_mask] = e.head[e.ins_h], e.head[e.ins_h] = e.strstart);
        while (--e.prev_length !== 0);
        if (e.match_available = 0, e.match_length = M - 1, e.strstart++, n && (te(e, false), e.strm.avail_out === 0))
          return K;
      } else if (e.match_available) {
        if (n = ve._tr_tally(e, 0, e.window[e.strstart - 1]), n && te(e, false), e.strstart++, e.lookahead--, e.strm.avail_out === 0)
          return K;
      } else
        e.match_available = 1, e.strstart++, e.lookahead--;
    }
    return e.match_available && (n = ve._tr_tally(e, 0, e.window[e.strstart - 1]), e.match_available = 0), e.insert = e.strstart < M - 1 ? e.strstart : M - 1, t === wt ? (te(e, true), e.strm.avail_out === 0 ? zt : Pr) : e.last_lit && (te(e, false), e.strm.avail_out === 0) ? K : An;
  }
  function j1(e, t) {
    for (var r, n, i, o, a = e.window;; ) {
      if (e.lookahead <= gt) {
        if (Wt(e), e.lookahead <= gt && t === Zt)
          return K;
        if (e.lookahead === 0)
          break;
      }
      if (e.match_length = 0, e.lookahead >= M && e.strstart > 0 && (i = e.strstart - 1, n = a[i], n === a[++i] && n === a[++i] && n === a[++i])) {
        o = e.strstart + gt;
        do
          ;
        while (n === a[++i] && n === a[++i] && n === a[++i] && n === a[++i] && n === a[++i] && n === a[++i] && n === a[++i] && n === a[++i] && i < o);
        e.match_length = gt - (o - i), e.match_length > e.lookahead && (e.match_length = e.lookahead);
      }
      if (e.match_length >= M ? (r = ve._tr_tally(e, 1, e.match_length - M), e.lookahead -= e.match_length, e.strstart += e.match_length, e.match_length = 0) : (r = ve._tr_tally(e, 0, e.window[e.strstart]), e.lookahead--, e.strstart++), r && (te(e, false), e.strm.avail_out === 0))
        return K;
    }
    return e.insert = 0, t === wt ? (te(e, true), e.strm.avail_out === 0 ? zt : Pr) : e.last_lit && (te(e, false), e.strm.avail_out === 0) ? K : An;
  }
  function U1(e, t) {
    for (var r;; ) {
      if (e.lookahead === 0 && (Wt(e), e.lookahead === 0)) {
        if (t === Zt)
          return K;
        break;
      }
      if (e.match_length = 0, r = ve._tr_tally(e, 0, e.window[e.strstart]), e.lookahead--, e.strstart++, r && (te(e, false), e.strm.avail_out === 0))
        return K;
    }
    return e.insert = 0, t === wt ? (te(e, true), e.strm.avail_out === 0 ? zt : Pr) : e.last_lit && (te(e, false), e.strm.avail_out === 0) ? K : An;
  }
  function Ze(e, t, r, n, i) {
    this.good_length = e, this.max_lazy = t, this.nice_length = r, this.max_chain = n, this.func = i;
  }
  var Br;
  Br = [new Ze(0, 0, 0, 0, M1), new Ze(4, 4, 8, 4, Zf), new Ze(4, 5, 16, 8, Zf), new Ze(4, 6, 32, 32, Zf), new Ze(4, 4, 16, 16, Dr), new Ze(8, 16, 32, 32, Dr), new Ze(8, 16, 128, 128, Dr), new Ze(8, 32, 128, 256, Dr), new Ze(32, 128, 258, 1024, Dr), new Ze(32, 258, 258, 4096, Dr)];
  function q1(e) {
    e.window_size = 2 * e.w_size, _t(e.head), e.max_lazy_match = Br[e.level].max_lazy, e.good_match = Br[e.level].good_length, e.nice_match = Br[e.level].nice_length, e.max_chain_length = Br[e.level].max_chain, e.strstart = 0, e.block_start = 0, e.lookahead = 0, e.insert = 0, e.match_length = e.prev_length = M - 1, e.match_available = 0, e.ins_h = 0;
  }
  function C1() {
    this.strm = null, this.status = 0, this.pending_buf = null, this.pending_buf_size = 0, this.pending_out = 0, this.pending = 0, this.wrap = 0, this.gzhead = null, this.gzindex = 0, this.method = ao, this.last_flush = -1, this.w_size = 0, this.w_bits = 0, this.w_mask = 0, this.window = null, this.window_size = 0, this.prev = null, this.head = null, this.ins_h = 0, this.hash_size = 0, this.hash_bits = 0, this.hash_mask = 0, this.hash_shift = 0, this.block_start = 0, this.match_length = 0, this.prev_match = 0, this.match_available = 0, this.strstart = 0, this.match_start = 0, this.lookahead = 0, this.prev_length = 0, this.max_chain_length = 0, this.max_lazy_match = 0, this.level = 0, this.strategy = 0, this.good_match = 0, this.nice_match = 0, this.dyn_ltree = new ae.Buf16(F1 * 2), this.dyn_dtree = new ae.Buf16((2 * N1 + 1) * 2), this.bl_tree = new ae.Buf16((2 * k1 + 1) * 2), _t(this.dyn_ltree), _t(this.dyn_dtree), _t(this.bl_tree), this.l_desc = null, this.d_desc = null, this.bl_desc = null, this.bl_count = new ae.Buf16(L1 + 1), this.heap = new ae.Buf16(2 * $f + 1), _t(this.heap), this.heap_len = 0, this.heap_max = 0, this.depth = new ae.Buf16(2 * $f + 1), _t(this.depth), this.l_buf = 0, this.lit_bufsize = 0, this.last_lit = 0, this.d_buf = 0, this.opt_len = 0, this.static_len = 0, this.matches = 0, this.insert = 0, this.bi_buf = 0, this.bi_valid = 0;
  }
  function $p(e) {
    var t;
    return !e || !e.state ? bt(e, me) : (e.total_in = e.total_out = 0, e.data_type = A1, t = e.state, t.pending = 0, t.pending_out = 0, t.wrap < 0 && (t.wrap = -t.wrap), t.status = t.wrap ? fo : Ct, e.adler = t.wrap === 2 ? 0 : 1, t.last_flush = Zt, ve._tr_init(t), $e);
  }
  function Gp(e) {
    var t = $p(e);
    return t === $e && q1(e.state), t;
  }
  function z1(e, t) {
    return !e || !e.state || e.state.wrap !== 2 ? me : (e.state.gzhead = t, $e);
  }
  function Hp(e, t, r, n, i, o) {
    if (!e)
      return me;
    var a = 1;
    if (t === w1 && (t = 6), n < 0 ? (a = 0, n = -n) : n > 15 && (a = 2, n -= 16), i < 1 || i > x1 || r !== ao || n < 8 || n > 15 || t < 0 || t > 9 || o < 0 || o > m1)
      return bt(e, me);
    n === 8 && (n = 9);
    var f = new C1;
    return e.state = f, f.strm = e, f.wrap = a, f.gzhead = null, f.w_bits = n, f.w_size = 1 << f.w_bits, f.w_mask = f.w_size - 1, f.hash_bits = i + 7, f.hash_size = 1 << f.hash_bits, f.hash_mask = f.hash_size - 1, f.hash_shift = ~~((f.hash_bits + M - 1) / M), f.window = new ae.Buf8(f.w_size * 2), f.head = new ae.Buf16(f.hash_size), f.prev = new ae.Buf16(f.w_size), f.lit_bufsize = 1 << i + 6, f.pending_buf_size = f.lit_bufsize * 4, f.pending_buf = new ae.Buf8(f.pending_buf_size), f.d_buf = 1 * f.lit_bufsize, f.l_buf = (1 + 2) * f.lit_bufsize, f.level = t, f.strategy = o, f.method = r, Gp(e);
  }
  function W1(e, t) {
    return Hp(e, t, ao, R1, I1, S1);
  }
  function Z1(e, t) {
    var r, n, i, o;
    if (!e || !e.state || t > qp || t < 0)
      return e ? bt(e, me) : me;
    if (n = e.state, !e.output || !e.input && e.avail_in !== 0 || n.status === Sn && t !== wt)
      return bt(e, e.avail_out === 0 ? Wf : me);
    if (n.strm = e, r = n.last_flush, n.last_flush = t, n.status === fo)
      if (n.wrap === 2)
        e.adler = 0, j(n, 31), j(n, 139), j(n, 8), n.gzhead ? (j(n, (n.gzhead.text ? 1 : 0) + (n.gzhead.hcrc ? 2 : 0) + (n.gzhead.extra ? 4 : 0) + (n.gzhead.name ? 8 : 0) + (n.gzhead.comment ? 16 : 0)), j(n, n.gzhead.time & 255), j(n, n.gzhead.time >> 8 & 255), j(n, n.gzhead.time >> 16 & 255), j(n, n.gzhead.time >> 24 & 255), j(n, n.level === 9 ? 2 : n.strategy >= ro || n.level < 2 ? 4 : 0), j(n, n.gzhead.os & 255), n.gzhead.extra && n.gzhead.extra.length && (j(n, n.gzhead.extra.length & 255), j(n, n.gzhead.extra.length >> 8 & 255)), n.gzhead.hcrc && (e.adler = pt(e.adler, n.pending_buf, n.pending, 0)), n.gzindex = 0, n.status = Gf) : (j(n, 0), j(n, 0), j(n, 0), j(n, 0), j(n, 0), j(n, n.level === 9 ? 2 : n.strategy >= ro || n.level < 2 ? 4 : 0), j(n, B1), n.status = Ct);
      else {
        var a = ao + (n.w_bits - 8 << 4) << 8, f = -1;
        n.strategy >= ro || n.level < 2 ? f = 0 : n.level < 6 ? f = 1 : n.level === 6 ? f = 2 : f = 3, a |= f << 6, n.strstart !== 0 && (a |= D1), a += 31 - a % 31, n.status = Ct, mn(n, a), n.strstart !== 0 && (mn(n, e.adler >>> 16), mn(n, e.adler & 65535)), e.adler = 1;
      }
    if (n.status === Gf)
      if (n.gzhead.extra) {
        for (i = n.pending;n.gzindex < (n.gzhead.extra.length & 65535) && !(n.pending === n.pending_buf_size && (n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), yt(e), i = n.pending, n.pending === n.pending_buf_size)); )
          j(n, n.gzhead.extra[n.gzindex] & 255), n.gzindex++;
        n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), n.gzindex === n.gzhead.extra.length && (n.gzindex = 0, n.status = no);
      } else
        n.status = no;
    if (n.status === no)
      if (n.gzhead.name) {
        i = n.pending;
        do {
          if (n.pending === n.pending_buf_size && (n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), yt(e), i = n.pending, n.pending === n.pending_buf_size)) {
            o = 1;
            break;
          }
          n.gzindex < n.gzhead.name.length ? o = n.gzhead.name.charCodeAt(n.gzindex++) & 255 : o = 0, j(n, o);
        } while (o !== 0);
        n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), o === 0 && (n.gzindex = 0, n.status = io);
      } else
        n.status = io;
    if (n.status === io)
      if (n.gzhead.comment) {
        i = n.pending;
        do {
          if (n.pending === n.pending_buf_size && (n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), yt(e), i = n.pending, n.pending === n.pending_buf_size)) {
            o = 1;
            break;
          }
          n.gzindex < n.gzhead.comment.length ? o = n.gzhead.comment.charCodeAt(n.gzindex++) & 255 : o = 0, j(n, o);
        } while (o !== 0);
        n.gzhead.hcrc && n.pending > i && (e.adler = pt(e.adler, n.pending_buf, n.pending - i, i)), o === 0 && (n.status = oo);
      } else
        n.status = oo;
    if (n.status === oo && (n.gzhead.hcrc ? (n.pending + 2 > n.pending_buf_size && yt(e), n.pending + 2 <= n.pending_buf_size && (j(n, e.adler & 255), j(n, e.adler >> 8 & 255), e.adler = 0, n.status = Ct)) : n.status = Ct), n.pending !== 0) {
      if (yt(e), e.avail_out === 0)
        return n.last_flush = -1, $e;
    } else if (e.avail_in === 0 && zp(t) <= zp(r) && t !== wt)
      return bt(e, Wf);
    if (n.status === Sn && e.avail_in !== 0)
      return bt(e, Wf);
    if (e.avail_in !== 0 || n.lookahead !== 0 || t !== Zt && n.status !== Sn) {
      var u = n.strategy === ro ? U1(n, t) : n.strategy === v1 ? j1(n, t) : Br[n.level].func(n, t);
      if ((u === zt || u === Pr) && (n.status = Sn), u === K || u === zt)
        return e.avail_out === 0 && (n.last_flush = -1), $e;
      if (u === An && (t === _1 ? ve._tr_align(n) : t !== qp && (ve._tr_stored_block(n, 0, 0, false), t === g1 && (_t(n.head), n.lookahead === 0 && (n.strstart = 0, n.block_start = 0, n.insert = 0))), yt(e), e.avail_out === 0))
        return n.last_flush = -1, $e;
    }
    return t !== wt ? $e : n.wrap <= 0 ? Cp : (n.wrap === 2 ? (j(n, e.adler & 255), j(n, e.adler >> 8 & 255), j(n, e.adler >> 16 & 255), j(n, e.adler >> 24 & 255), j(n, e.total_in & 255), j(n, e.total_in >> 8 & 255), j(n, e.total_in >> 16 & 255), j(n, e.total_in >> 24 & 255)) : (mn(n, e.adler >>> 16), mn(n, e.adler & 65535)), yt(e), n.wrap > 0 && (n.wrap = -n.wrap), n.pending !== 0 ? $e : Cp);
  }
  function $1(e) {
    var t;
    return !e || !e.state ? me : (t = e.state.status, t !== fo && t !== Gf && t !== no && t !== io && t !== oo && t !== Ct && t !== Sn ? bt(e, me) : (e.state = null, t === Ct ? bt(e, b1) : $e));
  }
  function G1(e, t) {
    var r = t.length, n, i, o, a, f, u, l, s;
    if (!e || !e.state || (n = e.state, a = n.wrap, a === 2 || a === 1 && n.status !== fo || n.lookahead))
      return me;
    for (a === 1 && (e.adler = Wp(e.adler, t, r, 0)), n.wrap = 0, r >= n.w_size && (a === 0 && (_t(n.head), n.strstart = 0, n.block_start = 0, n.insert = 0), s = new ae.Buf8(n.w_size), ae.arraySet(s, t, r - n.w_size, n.w_size, 0), t = s, r = n.w_size), f = e.avail_in, u = e.next_in, l = e.input, e.avail_in = r, e.next_in = 0, e.input = t, Wt(n);n.lookahead >= M; ) {
      i = n.strstart, o = n.lookahead - (M - 1);
      do
        n.ins_h = (n.ins_h << n.hash_shift ^ n.window[i + M - 1]) & n.hash_mask, n.prev[i & n.w_mask] = n.head[n.ins_h], n.head[n.ins_h] = i, i++;
      while (--o);
      n.strstart = i, n.lookahead = M - 1, Wt(n);
    }
    return n.strstart += n.lookahead, n.block_start = n.strstart, n.insert = n.lookahead, n.lookahead = 0, n.match_length = n.prev_length = M - 1, n.match_available = 0, e.next_in = u, e.input = l, e.avail_in = f, n.wrap = a, $e;
  }
  Ge.deflateInit = W1;
  Ge.deflateInit2 = Hp;
  Ge.deflateReset = Gp;
  Ge.deflateResetKeep = $p;
  Ge.deflateSetHeader = z1;
  Ge.deflate = Z1;
  Ge.deflateEnd = $1;
  Ge.deflateSetDictionary = G1;
  Ge.deflateInfo = "pako deflate (from Nodeca project)";
});
var Kp = g((Kx, Yp) => {
  var lo = 30, H1 = 12;
  Yp.exports = function(t, r) {
    var n, i, o, a, f, u, l, s, c, h, d, y, b, R, _, E, m, A, v, T, I, S, k, z, O;
    n = t.state, i = t.next_in, z = t.input, o = i + (t.avail_in - 5), a = t.next_out, O = t.output, f = a - (r - t.avail_out), u = a + (t.avail_out - 257), l = n.dmax, s = n.wsize, c = n.whave, h = n.wnext, d = n.window, y = n.hold, b = n.bits, R = n.lencode, _ = n.distcode, E = (1 << n.lenbits) - 1, m = (1 << n.distbits) - 1;
    e:
      do {
        b < 15 && (y += z[i++] << b, b += 8, y += z[i++] << b, b += 8), A = R[y & E];
        t:
          for (;; ) {
            if (v = A >>> 24, y >>>= v, b -= v, v = A >>> 16 & 255, v === 0)
              O[a++] = A & 65535;
            else if (v & 16) {
              T = A & 65535, v &= 15, v && (b < v && (y += z[i++] << b, b += 8), T += y & (1 << v) - 1, y >>>= v, b -= v), b < 15 && (y += z[i++] << b, b += 8, y += z[i++] << b, b += 8), A = _[y & m];
              r:
                for (;; ) {
                  if (v = A >>> 24, y >>>= v, b -= v, v = A >>> 16 & 255, v & 16) {
                    if (I = A & 65535, v &= 15, b < v && (y += z[i++] << b, b += 8, b < v && (y += z[i++] << b, b += 8)), I += y & (1 << v) - 1, I > l) {
                      t.msg = "invalid distance too far back", n.mode = lo;
                      break e;
                    }
                    if (y >>>= v, b -= v, v = a - f, I > v) {
                      if (v = I - v, v > c && n.sane) {
                        t.msg = "invalid distance too far back", n.mode = lo;
                        break e;
                      }
                      if (S = 0, k = d, h === 0) {
                        if (S += s - v, v < T) {
                          T -= v;
                          do
                            O[a++] = d[S++];
                          while (--v);
                          S = a - I, k = O;
                        }
                      } else if (h < v) {
                        if (S += s + h - v, v -= h, v < T) {
                          T -= v;
                          do
                            O[a++] = d[S++];
                          while (--v);
                          if (S = 0, h < T) {
                            v = h, T -= v;
                            do
                              O[a++] = d[S++];
                            while (--v);
                            S = a - I, k = O;
                          }
                        }
                      } else if (S += h - v, v < T) {
                        T -= v;
                        do
                          O[a++] = d[S++];
                        while (--v);
                        S = a - I, k = O;
                      }
                      for (;T > 2; )
                        O[a++] = k[S++], O[a++] = k[S++], O[a++] = k[S++], T -= 3;
                      T && (O[a++] = k[S++], T > 1 && (O[a++] = k[S++]));
                    } else {
                      S = a - I;
                      do
                        O[a++] = O[S++], O[a++] = O[S++], O[a++] = O[S++], T -= 3;
                      while (T > 2);
                      T && (O[a++] = O[S++], T > 1 && (O[a++] = O[S++]));
                    }
                  } else if ((v & 64) === 0) {
                    A = _[(A & 65535) + (y & (1 << v) - 1)];
                    continue r;
                  } else {
                    t.msg = "invalid distance code", n.mode = lo;
                    break e;
                  }
                  break;
                }
            } else if ((v & 64) === 0) {
              A = R[(A & 65535) + (y & (1 << v) - 1)];
              continue t;
            } else if (v & 32) {
              n.mode = H1;
              break e;
            } else {
              t.msg = "invalid literal/length code", n.mode = lo;
              break e;
            }
            break;
          }
      } while (i < o && a < u);
    T = b >> 3, i -= T, b -= T << 3, y &= (1 << b) - 1, t.next_in = i, t.next_out = a, t.avail_in = i < o ? 5 + (o - i) : 5 - (i - o), t.avail_out = a < u ? 257 + (u - a) : 257 - (a - u), n.hold = y, n.bits = b;
  };
});
var ny = g((Xx, ry) => {
  var Xp = yn(), Mr = 15, Jp = 852, Qp = 592, ey = 0, Hf = 1, ty = 2, V1 = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 0, 0], Y1 = [16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20, 21, 21, 21, 21, 16, 72, 78], K1 = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0], X1 = [16, 16, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 26, 26, 27, 27, 28, 28, 29, 29, 64, 64];
  ry.exports = function(t, r, n, i, o, a, f, u) {
    var l = u.bits, s = 0, c = 0, h = 0, d = 0, y = 0, b = 0, R = 0, _ = 0, E = 0, m = 0, A, v, T, I, S, k = null, z = 0, O, Ae = new Xp.Buf16(Mr + 1), On = new Xp.Buf16(Mr + 1), Nn = null, al = 0, fl, kn, Fn;
    for (s = 0;s <= Mr; s++)
      Ae[s] = 0;
    for (c = 0;c < i; c++)
      Ae[r[n + c]]++;
    for (y = l, d = Mr;d >= 1 && Ae[d] === 0; d--)
      ;
    if (y > d && (y = d), d === 0)
      return o[a++] = 1 << 24 | 64 << 16 | 0, o[a++] = 1 << 24 | 64 << 16 | 0, u.bits = 1, 0;
    for (h = 1;h < d && Ae[h] === 0; h++)
      ;
    for (y < h && (y = h), _ = 1, s = 1;s <= Mr; s++)
      if (_ <<= 1, _ -= Ae[s], _ < 0)
        return -1;
    if (_ > 0 && (t === ey || d !== 1))
      return -1;
    for (On[1] = 0, s = 1;s < Mr; s++)
      On[s + 1] = On[s] + Ae[s];
    for (c = 0;c < i; c++)
      r[n + c] !== 0 && (f[On[r[n + c]]++] = c);
    if (t === ey ? (k = Nn = f, O = 19) : t === Hf ? (k = V1, z -= 257, Nn = Y1, al -= 257, O = 256) : (k = K1, Nn = X1, O = -1), m = 0, c = 0, s = h, S = a, b = y, R = 0, T = -1, E = 1 << y, I = E - 1, t === Hf && E > Jp || t === ty && E > Qp)
      return 1;
    for (;; ) {
      fl = s - R, f[c] < O ? (kn = 0, Fn = f[c]) : f[c] > O ? (kn = Nn[al + f[c]], Fn = k[z + f[c]]) : (kn = 32 + 64, Fn = 0), A = 1 << s - R, v = 1 << b, h = v;
      do
        v -= A, o[S + (m >> R) + v] = fl << 24 | kn << 16 | Fn | 0;
      while (v !== 0);
      for (A = 1 << s - 1;m & A; )
        A >>= 1;
      if (A !== 0 ? (m &= A - 1, m += A) : m = 0, c++, --Ae[s] === 0) {
        if (s === d)
          break;
        s = r[n + f[c]];
      }
      if (s > y && (m & I) !== T) {
        for (R === 0 && (R = y), S += h, b = s - R, _ = 1 << b;b + R < d && (_ -= Ae[b + R], !(_ <= 0)); )
          b++, _ <<= 1;
        if (E += 1 << b, t === Hf && E > Jp || t === ty && E > Qp)
          return 1;
        T = m & I, o[T] = y << 24 | b << 16 | S - a | 0;
      }
    }
    return m !== 0 && (o[S + m] = s - R << 24 | 64 << 16 | 0), u.bits = y, 0;
  };
});
var jy = g((Le) => {
  var de = yn(), Qf = Cf(), He = zf(), J1 = Kp(), xn = ny(), Q1 = 0, Oy = 1, Ny = 2, iy = 4, eA = 5, uo = 6, $t = 0, tA = 1, rA = 2, Se = -2, ky = -3, el = -4, nA = -5, oy = 8, Fy = 1, ay = 2, fy = 3, ly = 4, uy = 5, sy = 6, cy = 7, dy = 8, hy = 9, py = 10, ho = 11, rt = 12, Vf = 13, yy = 14, Yf = 15, _y = 16, gy = 17, by = 18, wy = 19, so = 20, co = 21, Ey = 22, vy = 23, my = 24, Sy = 25, Ay = 26, Kf = 27, xy = 28, Ry = 29, C = 30, tl = 31, iA = 32, oA = 852, aA = 592, fA = 15, lA = fA;
  function Iy(e) {
    return (e >>> 24 & 255) + (e >>> 8 & 65280) + ((e & 65280) << 8) + ((e & 255) << 24);
  }
  function uA() {
    this.mode = 0, this.last = false, this.wrap = 0, this.havedict = false, this.flags = 0, this.dmax = 0, this.check = 0, this.total = 0, this.head = null, this.wbits = 0, this.wsize = 0, this.whave = 0, this.wnext = 0, this.window = null, this.hold = 0, this.bits = 0, this.length = 0, this.offset = 0, this.extra = 0, this.lencode = null, this.distcode = null, this.lenbits = 0, this.distbits = 0, this.ncode = 0, this.nlen = 0, this.ndist = 0, this.have = 0, this.next = null, this.lens = new de.Buf16(320), this.work = new de.Buf16(288), this.lendyn = null, this.distdyn = null, this.sane = 0, this.back = 0, this.was = 0;
  }
  function Ly(e) {
    var t;
    return !e || !e.state ? Se : (t = e.state, e.total_in = e.total_out = t.total = 0, e.msg = "", t.wrap && (e.adler = t.wrap & 1), t.mode = Fy, t.last = 0, t.havedict = 0, t.dmax = 32768, t.head = null, t.hold = 0, t.bits = 0, t.lencode = t.lendyn = new de.Buf32(oA), t.distcode = t.distdyn = new de.Buf32(aA), t.sane = 1, t.back = -1, $t);
  }
  function Dy(e) {
    var t;
    return !e || !e.state ? Se : (t = e.state, t.wsize = 0, t.whave = 0, t.wnext = 0, Ly(e));
  }
  function By(e, t) {
    var r, n;
    return !e || !e.state || (n = e.state, t < 0 ? (r = 0, t = -t) : (r = (t >> 4) + 1, t < 48 && (t &= 15)), t && (t < 8 || t > 15)) ? Se : (n.window !== null && n.wbits !== t && (n.window = null), n.wrap = r, n.wbits = t, Dy(e));
  }
  function Py(e, t) {
    var r, n;
    return e ? (n = new uA, e.state = n, n.window = null, r = By(e, t), r !== $t && (e.state = null), r) : Se;
  }
  function sA(e) {
    return Py(e, lA);
  }
  var Ty = true, Xf, Jf;
  function cA(e) {
    if (Ty) {
      var t;
      for (Xf = new de.Buf32(512), Jf = new de.Buf32(32), t = 0;t < 144; )
        e.lens[t++] = 8;
      for (;t < 256; )
        e.lens[t++] = 9;
      for (;t < 280; )
        e.lens[t++] = 7;
      for (;t < 288; )
        e.lens[t++] = 8;
      for (xn(Oy, e.lens, 0, 288, Xf, 0, e.work, { bits: 9 }), t = 0;t < 32; )
        e.lens[t++] = 5;
      xn(Ny, e.lens, 0, 32, Jf, 0, e.work, { bits: 5 }), Ty = false;
    }
    e.lencode = Xf, e.lenbits = 9, e.distcode = Jf, e.distbits = 5;
  }
  function My(e, t, r, n) {
    var i, o = e.state;
    return o.window === null && (o.wsize = 1 << o.wbits, o.wnext = 0, o.whave = 0, o.window = new de.Buf8(o.wsize)), n >= o.wsize ? (de.arraySet(o.window, t, r - o.wsize, o.wsize, 0), o.wnext = 0, o.whave = o.wsize) : (i = o.wsize - o.wnext, i > n && (i = n), de.arraySet(o.window, t, r - n, i, o.wnext), n -= i, n ? (de.arraySet(o.window, t, r - n, n, 0), o.wnext = n, o.whave = o.wsize) : (o.wnext += i, o.wnext === o.wsize && (o.wnext = 0), o.whave < o.wsize && (o.whave += i))), 0;
  }
  function dA(e, t) {
    var r, n, i, o, a, f, u, l, s, c, h, d, y, b, R = 0, _, E, m, A, v, T, I, S, k = new de.Buf8(4), z, O, Ae = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    if (!e || !e.state || !e.output || !e.input && e.avail_in !== 0)
      return Se;
    r = e.state, r.mode === rt && (r.mode = Vf), a = e.next_out, i = e.output, u = e.avail_out, o = e.next_in, n = e.input, f = e.avail_in, l = r.hold, s = r.bits, c = f, h = u, S = $t;
    e:
      for (;; )
        switch (r.mode) {
          case Fy:
            if (r.wrap === 0) {
              r.mode = Vf;
              break;
            }
            for (;s < 16; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if (r.wrap & 2 && l === 35615) {
              r.check = 0, k[0] = l & 255, k[1] = l >>> 8 & 255, r.check = He(r.check, k, 2, 0), l = 0, s = 0, r.mode = ay;
              break;
            }
            if (r.flags = 0, r.head && (r.head.done = false), !(r.wrap & 1) || (((l & 255) << 8) + (l >> 8)) % 31) {
              e.msg = "incorrect header check", r.mode = C;
              break;
            }
            if ((l & 15) !== oy) {
              e.msg = "unknown compression method", r.mode = C;
              break;
            }
            if (l >>>= 4, s -= 4, I = (l & 15) + 8, r.wbits === 0)
              r.wbits = I;
            else if (I > r.wbits) {
              e.msg = "invalid window size", r.mode = C;
              break;
            }
            r.dmax = 1 << I, e.adler = r.check = 1, r.mode = l & 512 ? py : rt, l = 0, s = 0;
            break;
          case ay:
            for (;s < 16; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if (r.flags = l, (r.flags & 255) !== oy) {
              e.msg = "unknown compression method", r.mode = C;
              break;
            }
            if (r.flags & 57344) {
              e.msg = "unknown header flags set", r.mode = C;
              break;
            }
            r.head && (r.head.text = l >> 8 & 1), r.flags & 512 && (k[0] = l & 255, k[1] = l >>> 8 & 255, r.check = He(r.check, k, 2, 0)), l = 0, s = 0, r.mode = fy;
          case fy:
            for (;s < 32; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            r.head && (r.head.time = l), r.flags & 512 && (k[0] = l & 255, k[1] = l >>> 8 & 255, k[2] = l >>> 16 & 255, k[3] = l >>> 24 & 255, r.check = He(r.check, k, 4, 0)), l = 0, s = 0, r.mode = ly;
          case ly:
            for (;s < 16; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            r.head && (r.head.xflags = l & 255, r.head.os = l >> 8), r.flags & 512 && (k[0] = l & 255, k[1] = l >>> 8 & 255, r.check = He(r.check, k, 2, 0)), l = 0, s = 0, r.mode = uy;
          case uy:
            if (r.flags & 1024) {
              for (;s < 16; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              r.length = l, r.head && (r.head.extra_len = l), r.flags & 512 && (k[0] = l & 255, k[1] = l >>> 8 & 255, r.check = He(r.check, k, 2, 0)), l = 0, s = 0;
            } else
              r.head && (r.head.extra = null);
            r.mode = sy;
          case sy:
            if (r.flags & 1024 && (d = r.length, d > f && (d = f), d && (r.head && (I = r.head.extra_len - r.length, r.head.extra || (r.head.extra = new Array(r.head.extra_len)), de.arraySet(r.head.extra, n, o, d, I)), r.flags & 512 && (r.check = He(r.check, n, d, o)), f -= d, o += d, r.length -= d), r.length))
              break e;
            r.length = 0, r.mode = cy;
          case cy:
            if (r.flags & 2048) {
              if (f === 0)
                break e;
              d = 0;
              do
                I = n[o + d++], r.head && I && r.length < 65536 && (r.head.name += String.fromCharCode(I));
              while (I && d < f);
              if (r.flags & 512 && (r.check = He(r.check, n, d, o)), f -= d, o += d, I)
                break e;
            } else
              r.head && (r.head.name = null);
            r.length = 0, r.mode = dy;
          case dy:
            if (r.flags & 4096) {
              if (f === 0)
                break e;
              d = 0;
              do
                I = n[o + d++], r.head && I && r.length < 65536 && (r.head.comment += String.fromCharCode(I));
              while (I && d < f);
              if (r.flags & 512 && (r.check = He(r.check, n, d, o)), f -= d, o += d, I)
                break e;
            } else
              r.head && (r.head.comment = null);
            r.mode = hy;
          case hy:
            if (r.flags & 512) {
              for (;s < 16; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              if (l !== (r.check & 65535)) {
                e.msg = "header crc mismatch", r.mode = C;
                break;
              }
              l = 0, s = 0;
            }
            r.head && (r.head.hcrc = r.flags >> 9 & 1, r.head.done = true), e.adler = r.check = 0, r.mode = rt;
            break;
          case py:
            for (;s < 32; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            e.adler = r.check = Iy(l), l = 0, s = 0, r.mode = ho;
          case ho:
            if (r.havedict === 0)
              return e.next_out = a, e.avail_out = u, e.next_in = o, e.avail_in = f, r.hold = l, r.bits = s, rA;
            e.adler = r.check = 1, r.mode = rt;
          case rt:
            if (t === eA || t === uo)
              break e;
          case Vf:
            if (r.last) {
              l >>>= s & 7, s -= s & 7, r.mode = Kf;
              break;
            }
            for (;s < 3; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            switch (r.last = l & 1, l >>>= 1, s -= 1, l & 3) {
              case 0:
                r.mode = yy;
                break;
              case 1:
                if (cA(r), r.mode = so, t === uo) {
                  l >>>= 2, s -= 2;
                  break e;
                }
                break;
              case 2:
                r.mode = gy;
                break;
              case 3:
                e.msg = "invalid block type", r.mode = C;
            }
            l >>>= 2, s -= 2;
            break;
          case yy:
            for (l >>>= s & 7, s -= s & 7;s < 32; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if ((l & 65535) !== (l >>> 16 ^ 65535)) {
              e.msg = "invalid stored block lengths", r.mode = C;
              break;
            }
            if (r.length = l & 65535, l = 0, s = 0, r.mode = Yf, t === uo)
              break e;
          case Yf:
            r.mode = _y;
          case _y:
            if (d = r.length, d) {
              if (d > f && (d = f), d > u && (d = u), d === 0)
                break e;
              de.arraySet(i, n, o, d, a), f -= d, o += d, u -= d, a += d, r.length -= d;
              break;
            }
            r.mode = rt;
            break;
          case gy:
            for (;s < 14; ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if (r.nlen = (l & 31) + 257, l >>>= 5, s -= 5, r.ndist = (l & 31) + 1, l >>>= 5, s -= 5, r.ncode = (l & 15) + 4, l >>>= 4, s -= 4, r.nlen > 286 || r.ndist > 30) {
              e.msg = "too many length or distance symbols", r.mode = C;
              break;
            }
            r.have = 0, r.mode = by;
          case by:
            for (;r.have < r.ncode; ) {
              for (;s < 3; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              r.lens[Ae[r.have++]] = l & 7, l >>>= 3, s -= 3;
            }
            for (;r.have < 19; )
              r.lens[Ae[r.have++]] = 0;
            if (r.lencode = r.lendyn, r.lenbits = 7, z = { bits: r.lenbits }, S = xn(Q1, r.lens, 0, 19, r.lencode, 0, r.work, z), r.lenbits = z.bits, S) {
              e.msg = "invalid code lengths set", r.mode = C;
              break;
            }
            r.have = 0, r.mode = wy;
          case wy:
            for (;r.have < r.nlen + r.ndist; ) {
              for (;R = r.lencode[l & (1 << r.lenbits) - 1], _ = R >>> 24, E = R >>> 16 & 255, m = R & 65535, !(_ <= s); ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              if (m < 16)
                l >>>= _, s -= _, r.lens[r.have++] = m;
              else {
                if (m === 16) {
                  for (O = _ + 2;s < O; ) {
                    if (f === 0)
                      break e;
                    f--, l += n[o++] << s, s += 8;
                  }
                  if (l >>>= _, s -= _, r.have === 0) {
                    e.msg = "invalid bit length repeat", r.mode = C;
                    break;
                  }
                  I = r.lens[r.have - 1], d = 3 + (l & 3), l >>>= 2, s -= 2;
                } else if (m === 17) {
                  for (O = _ + 3;s < O; ) {
                    if (f === 0)
                      break e;
                    f--, l += n[o++] << s, s += 8;
                  }
                  l >>>= _, s -= _, I = 0, d = 3 + (l & 7), l >>>= 3, s -= 3;
                } else {
                  for (O = _ + 7;s < O; ) {
                    if (f === 0)
                      break e;
                    f--, l += n[o++] << s, s += 8;
                  }
                  l >>>= _, s -= _, I = 0, d = 11 + (l & 127), l >>>= 7, s -= 7;
                }
                if (r.have + d > r.nlen + r.ndist) {
                  e.msg = "invalid bit length repeat", r.mode = C;
                  break;
                }
                for (;d--; )
                  r.lens[r.have++] = I;
              }
            }
            if (r.mode === C)
              break;
            if (r.lens[256] === 0) {
              e.msg = "invalid code -- missing end-of-block", r.mode = C;
              break;
            }
            if (r.lenbits = 9, z = { bits: r.lenbits }, S = xn(Oy, r.lens, 0, r.nlen, r.lencode, 0, r.work, z), r.lenbits = z.bits, S) {
              e.msg = "invalid literal/lengths set", r.mode = C;
              break;
            }
            if (r.distbits = 6, r.distcode = r.distdyn, z = { bits: r.distbits }, S = xn(Ny, r.lens, r.nlen, r.ndist, r.distcode, 0, r.work, z), r.distbits = z.bits, S) {
              e.msg = "invalid distances set", r.mode = C;
              break;
            }
            if (r.mode = so, t === uo)
              break e;
          case so:
            r.mode = co;
          case co:
            if (f >= 6 && u >= 258) {
              e.next_out = a, e.avail_out = u, e.next_in = o, e.avail_in = f, r.hold = l, r.bits = s, J1(e, h), a = e.next_out, i = e.output, u = e.avail_out, o = e.next_in, n = e.input, f = e.avail_in, l = r.hold, s = r.bits, r.mode === rt && (r.back = -1);
              break;
            }
            for (r.back = 0;R = r.lencode[l & (1 << r.lenbits) - 1], _ = R >>> 24, E = R >>> 16 & 255, m = R & 65535, !(_ <= s); ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if (E && (E & 240) === 0) {
              for (A = _, v = E, T = m;R = r.lencode[T + ((l & (1 << A + v) - 1) >> A)], _ = R >>> 24, E = R >>> 16 & 255, m = R & 65535, !(A + _ <= s); ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              l >>>= A, s -= A, r.back += A;
            }
            if (l >>>= _, s -= _, r.back += _, r.length = m, E === 0) {
              r.mode = Ay;
              break;
            }
            if (E & 32) {
              r.back = -1, r.mode = rt;
              break;
            }
            if (E & 64) {
              e.msg = "invalid literal/length code", r.mode = C;
              break;
            }
            r.extra = E & 15, r.mode = Ey;
          case Ey:
            if (r.extra) {
              for (O = r.extra;s < O; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              r.length += l & (1 << r.extra) - 1, l >>>= r.extra, s -= r.extra, r.back += r.extra;
            }
            r.was = r.length, r.mode = vy;
          case vy:
            for (;R = r.distcode[l & (1 << r.distbits) - 1], _ = R >>> 24, E = R >>> 16 & 255, m = R & 65535, !(_ <= s); ) {
              if (f === 0)
                break e;
              f--, l += n[o++] << s, s += 8;
            }
            if ((E & 240) === 0) {
              for (A = _, v = E, T = m;R = r.distcode[T + ((l & (1 << A + v) - 1) >> A)], _ = R >>> 24, E = R >>> 16 & 255, m = R & 65535, !(A + _ <= s); ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              l >>>= A, s -= A, r.back += A;
            }
            if (l >>>= _, s -= _, r.back += _, E & 64) {
              e.msg = "invalid distance code", r.mode = C;
              break;
            }
            r.offset = m, r.extra = E & 15, r.mode = my;
          case my:
            if (r.extra) {
              for (O = r.extra;s < O; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              r.offset += l & (1 << r.extra) - 1, l >>>= r.extra, s -= r.extra, r.back += r.extra;
            }
            if (r.offset > r.dmax) {
              e.msg = "invalid distance too far back", r.mode = C;
              break;
            }
            r.mode = Sy;
          case Sy:
            if (u === 0)
              break e;
            if (d = h - u, r.offset > d) {
              if (d = r.offset - d, d > r.whave && r.sane) {
                e.msg = "invalid distance too far back", r.mode = C;
                break;
              }
              d > r.wnext ? (d -= r.wnext, y = r.wsize - d) : y = r.wnext - d, d > r.length && (d = r.length), b = r.window;
            } else
              b = i, y = a - r.offset, d = r.length;
            d > u && (d = u), u -= d, r.length -= d;
            do
              i[a++] = b[y++];
            while (--d);
            r.length === 0 && (r.mode = co);
            break;
          case Ay:
            if (u === 0)
              break e;
            i[a++] = r.length, u--, r.mode = co;
            break;
          case Kf:
            if (r.wrap) {
              for (;s < 32; ) {
                if (f === 0)
                  break e;
                f--, l |= n[o++] << s, s += 8;
              }
              if (h -= u, e.total_out += h, r.total += h, h && (e.adler = r.check = r.flags ? He(r.check, i, h, a - h) : Qf(r.check, i, h, a - h)), h = u, (r.flags ? l : Iy(l)) !== r.check) {
                e.msg = "incorrect data check", r.mode = C;
                break;
              }
              l = 0, s = 0;
            }
            r.mode = xy;
          case xy:
            if (r.wrap && r.flags) {
              for (;s < 32; ) {
                if (f === 0)
                  break e;
                f--, l += n[o++] << s, s += 8;
              }
              if (l !== (r.total & 4294967295)) {
                e.msg = "incorrect length check", r.mode = C;
                break;
              }
              l = 0, s = 0;
            }
            r.mode = Ry;
          case Ry:
            S = tA;
            break e;
          case C:
            S = ky;
            break e;
          case tl:
            return el;
          case iA:
          default:
            return Se;
        }
    return e.next_out = a, e.avail_out = u, e.next_in = o, e.avail_in = f, r.hold = l, r.bits = s, (r.wsize || h !== e.avail_out && r.mode < C && (r.mode < Kf || t !== iy)) && My(e, e.output, e.next_out, h - e.avail_out) ? (r.mode = tl, el) : (c -= e.avail_in, h -= e.avail_out, e.total_in += c, e.total_out += h, r.total += h, r.wrap && h && (e.adler = r.check = r.flags ? He(r.check, i, h, e.next_out - h) : Qf(r.check, i, h, e.next_out - h)), e.data_type = r.bits + (r.last ? 64 : 0) + (r.mode === rt ? 128 : 0) + (r.mode === so || r.mode === Yf ? 256 : 0), (c === 0 && h === 0 || t === iy) && S === $t && (S = nA), S);
  }
  function hA(e) {
    if (!e || !e.state)
      return Se;
    var t = e.state;
    return t.window && (t.window = null), e.state = null, $t;
  }
  function pA(e, t) {
    var r;
    return !e || !e.state || (r = e.state, (r.wrap & 2) === 0) ? Se : (r.head = t, t.done = false, $t);
  }
  function yA(e, t) {
    var r = t.length, n, i, o;
    return !e || !e.state || (n = e.state, n.wrap !== 0 && n.mode !== ho) ? Se : n.mode === ho && (i = 1, i = Qf(i, t, r, 0), i !== n.check) ? ky : (o = My(e, t, r, r), o ? (n.mode = tl, el) : (n.havedict = 1, $t));
  }
  Le.inflateReset = Dy;
  Le.inflateReset2 = By;
  Le.inflateResetKeep = Ly;
  Le.inflateInit = sA;
  Le.inflateInit2 = Py;
  Le.inflate = dA;
  Le.inflateEnd = hA;
  Le.inflateGetHeader = pA;
  Le.inflateSetDictionary = yA;
  Le.inflateInfo = "pako inflate (from Nodeca project)";
});
var qy = g((Qx, Uy) => {
  Uy.exports = { Z_NO_FLUSH: 0, Z_PARTIAL_FLUSH: 1, Z_SYNC_FLUSH: 2, Z_FULL_FLUSH: 3, Z_FINISH: 4, Z_BLOCK: 5, Z_TREES: 6, Z_OK: 0, Z_STREAM_END: 1, Z_NEED_DICT: 2, Z_ERRNO: -1, Z_STREAM_ERROR: -2, Z_DATA_ERROR: -3, Z_BUF_ERROR: -5, Z_NO_COMPRESSION: 0, Z_BEST_SPEED: 1, Z_BEST_COMPRESSION: 9, Z_DEFAULT_COMPRESSION: -1, Z_FILTERED: 1, Z_HUFFMAN_ONLY: 2, Z_RLE: 3, Z_FIXED: 4, Z_DEFAULT_STRATEGY: 0, Z_BINARY: 0, Z_TEXT: 1, Z_UNKNOWN: 2, Z_DEFLATED: 8 };
});
var zy = g((w) => {
  var he = tn(), _A = dp(), Rn = Vp(), Gt = jy(), Cy = qy();
  for (rl in Cy)
    w[rl] = Cy[rl];
  var rl;
  w.NONE = 0;
  w.DEFLATE = 1;
  w.INFLATE = 2;
  w.GZIP = 3;
  w.GUNZIP = 4;
  w.DEFLATERAW = 5;
  w.INFLATERAW = 6;
  w.UNZIP = 7;
  var gA = 31, bA = 139;
  function re(e) {
    if (typeof e != "number" || e < w.DEFLATE || e > w.UNZIP)
      throw new TypeError("Bad argument");
    this.dictionary = null, this.err = 0, this.flush = 0, this.init_done = false, this.level = 0, this.memLevel = 0, this.mode = e, this.strategy = 0, this.windowBits = 0, this.write_in_progress = false, this.pending_close = false, this.gzip_id_bytes_read = 0;
  }
  re.prototype.close = function() {
    if (this.write_in_progress) {
      this.pending_close = true;
      return;
    }
    this.pending_close = false, he(this.init_done, "close before init"), he(this.mode <= w.UNZIP), this.mode === w.DEFLATE || this.mode === w.GZIP || this.mode === w.DEFLATERAW ? Rn.deflateEnd(this.strm) : (this.mode === w.INFLATE || this.mode === w.GUNZIP || this.mode === w.INFLATERAW || this.mode === w.UNZIP) && Gt.inflateEnd(this.strm), this.mode = w.NONE, this.dictionary = null;
  };
  re.prototype.write = function(e, t, r, n, i, o, a) {
    return this._write(true, e, t, r, n, i, o, a);
  };
  re.prototype.writeSync = function(e, t, r, n, i, o, a) {
    return this._write(false, e, t, r, n, i, o, a);
  };
  re.prototype._write = function(e, t, r, n, i, o, a, f) {
    if (he.equal(arguments.length, 8), he(this.init_done, "write before init"), he(this.mode !== w.NONE, "already finalized"), he.equal(false, this.write_in_progress, "write already in progress"), he.equal(false, this.pending_close, "close is pending"), this.write_in_progress = true, he.equal(false, t === undefined, "must provide flush value"), this.write_in_progress = true, t !== w.Z_NO_FLUSH && t !== w.Z_PARTIAL_FLUSH && t !== w.Z_SYNC_FLUSH && t !== w.Z_FULL_FLUSH && t !== w.Z_FINISH && t !== w.Z_BLOCK)
      throw new Error("Invalid flush value");
    if (r == null && (r = Buffer.alloc(0), i = 0, n = 0), this.strm.avail_in = i, this.strm.input = r, this.strm.next_in = n, this.strm.avail_out = f, this.strm.output = o, this.strm.next_out = a, this.flush = t, !e)
      return this._process(), this._checkError() ? this._afterSync() : undefined;
    var u = this;
    return process.nextTick(function() {
      u._process(), u._after();
    }), this;
  };
  re.prototype._afterSync = function() {
    var e = this.strm.avail_out, t = this.strm.avail_in;
    return this.write_in_progress = false, [t, e];
  };
  re.prototype._process = function() {
    var e = null;
    switch (this.mode) {
      case w.DEFLATE:
      case w.GZIP:
      case w.DEFLATERAW:
        this.err = Rn.deflate(this.strm, this.flush);
        break;
      case w.UNZIP:
        switch (this.strm.avail_in > 0 && (e = this.strm.next_in), this.gzip_id_bytes_read) {
          case 0:
            if (e === null)
              break;
            if (this.strm.input[e] === gA) {
              if (this.gzip_id_bytes_read = 1, e++, this.strm.avail_in === 1)
                break;
            } else {
              this.mode = w.INFLATE;
              break;
            }
          case 1:
            if (e === null)
              break;
            this.strm.input[e] === bA ? (this.gzip_id_bytes_read = 2, this.mode = w.GUNZIP) : this.mode = w.INFLATE;
            break;
          default:
            throw new Error("invalid number of gzip magic number bytes read");
        }
      case w.INFLATE:
      case w.GUNZIP:
      case w.INFLATERAW:
        for (this.err = Gt.inflate(this.strm, this.flush), this.err === w.Z_NEED_DICT && this.dictionary && (this.err = Gt.inflateSetDictionary(this.strm, this.dictionary), this.err === w.Z_OK ? this.err = Gt.inflate(this.strm, this.flush) : this.err === w.Z_DATA_ERROR && (this.err = w.Z_NEED_DICT));this.strm.avail_in > 0 && this.mode === w.GUNZIP && this.err === w.Z_STREAM_END && this.strm.next_in[0] !== 0; )
          this.reset(), this.err = Gt.inflate(this.strm, this.flush);
        break;
      default:
        throw new Error("Unknown mode " + this.mode);
    }
  };
  re.prototype._checkError = function() {
    switch (this.err) {
      case w.Z_OK:
      case w.Z_BUF_ERROR:
        if (this.strm.avail_out !== 0 && this.flush === w.Z_FINISH)
          return this._error("unexpected end of file"), false;
        break;
      case w.Z_STREAM_END:
        break;
      case w.Z_NEED_DICT:
        return this.dictionary == null ? this._error("Missing dictionary") : this._error("Bad dictionary"), false;
      default:
        return this._error("Zlib error"), false;
    }
    return true;
  };
  re.prototype._after = function() {
    if (!!this._checkError()) {
      var e = this.strm.avail_out, t = this.strm.avail_in;
      this.write_in_progress = false, this.callback(t, e), this.pending_close && this.close();
    }
  };
  re.prototype._error = function(e) {
    this.strm.msg && (e = this.strm.msg), this.onerror(e, this.err), this.write_in_progress = false, this.pending_close && this.close();
  };
  re.prototype.init = function(e, t, r, n, i) {
    he(arguments.length === 4 || arguments.length === 5, "init(windowBits, level, memLevel, strategy, [dictionary])"), he(e >= 8 && e <= 15, "invalid windowBits"), he(t >= -1 && t <= 9, "invalid compression level"), he(r >= 1 && r <= 9, "invalid memlevel"), he(n === w.Z_FILTERED || n === w.Z_HUFFMAN_ONLY || n === w.Z_RLE || n === w.Z_FIXED || n === w.Z_DEFAULT_STRATEGY, "invalid strategy"), this._init(t, e, r, n, i), this._setDictionary();
  };
  re.prototype.params = function() {
    throw new Error("deflateParams Not supported");
  };
  re.prototype.reset = function() {
    this._reset(), this._setDictionary();
  };
  re.prototype._init = function(e, t, r, n, i) {
    switch (this.level = e, this.windowBits = t, this.memLevel = r, this.strategy = n, this.flush = w.Z_NO_FLUSH, this.err = w.Z_OK, (this.mode === w.GZIP || this.mode === w.GUNZIP) && (this.windowBits += 16), this.mode === w.UNZIP && (this.windowBits += 32), (this.mode === w.DEFLATERAW || this.mode === w.INFLATERAW) && (this.windowBits = -1 * this.windowBits), this.strm = new _A, this.mode) {
      case w.DEFLATE:
      case w.GZIP:
      case w.DEFLATERAW:
        this.err = Rn.deflateInit2(this.strm, this.level, w.Z_DEFLATED, this.windowBits, this.memLevel, this.strategy);
        break;
      case w.INFLATE:
      case w.GUNZIP:
      case w.INFLATERAW:
      case w.UNZIP:
        this.err = Gt.inflateInit2(this.strm, this.windowBits);
        break;
      default:
        throw new Error("Unknown mode " + this.mode);
    }
    this.err !== w.Z_OK && this._error("Init error"), this.dictionary = i, this.write_in_progress = false, this.init_done = true;
  };
  re.prototype._setDictionary = function() {
    if (this.dictionary != null) {
      switch (this.err = w.Z_OK, this.mode) {
        case w.DEFLATE:
        case w.DEFLATERAW:
          this.err = Rn.deflateSetDictionary(this.strm, this.dictionary);
          break;
        default:
          break;
      }
      this.err !== w.Z_OK && this._error("Failed to set dictionary");
    }
  };
  re.prototype._reset = function() {
    switch (this.err = w.Z_OK, this.mode) {
      case w.DEFLATE:
      case w.DEFLATERAW:
      case w.GZIP:
        this.err = Rn.deflateReset(this.strm);
        break;
      case w.INFLATE:
      case w.INFLATERAW:
      case w.GUNZIP:
        this.err = Gt.inflateReset(this.strm);
        break;
      default:
        break;
    }
    this.err !== w.Z_OK && this._error("Failed to reset stream");
  };
  w.Zlib = re;
});
var ol = g((x) => {
  var Ve = xe().Buffer, Gy = (_c(), se(br)).Transform, N = zy(), Et = ff(), In = tn().ok, il = xe().kMaxLength, Hy = "Cannot create final Buffer. It would be larger than 0x" + il.toString(16) + " bytes";
  N.Z_MIN_WINDOWBITS = 8;
  N.Z_MAX_WINDOWBITS = 15;
  N.Z_DEFAULT_WINDOWBITS = 15;
  N.Z_MIN_CHUNK = 64;
  N.Z_MAX_CHUNK = 1 / 0;
  N.Z_DEFAULT_CHUNK = 16 * 1024;
  N.Z_MIN_MEMLEVEL = 1;
  N.Z_MAX_MEMLEVEL = 9;
  N.Z_DEFAULT_MEMLEVEL = 8;
  N.Z_MIN_LEVEL = -1;
  N.Z_MAX_LEVEL = 9;
  N.Z_DEFAULT_LEVEL = N.Z_DEFAULT_COMPRESSION;
  var Wy = Object.keys(N);
  for (po = 0;po < Wy.length; po++)
    yo = Wy[po], yo.match(/^Z/) && Object.defineProperty(x, yo, { enumerable: true, value: N[yo], writable: false });
  var yo, po, go = { Z_OK: N.Z_OK, Z_STREAM_END: N.Z_STREAM_END, Z_NEED_DICT: N.Z_NEED_DICT, Z_ERRNO: N.Z_ERRNO, Z_STREAM_ERROR: N.Z_STREAM_ERROR, Z_DATA_ERROR: N.Z_DATA_ERROR, Z_MEM_ERROR: N.Z_MEM_ERROR, Z_BUF_ERROR: N.Z_BUF_ERROR, Z_VERSION_ERROR: N.Z_VERSION_ERROR }, Zy = Object.keys(go);
  for (_o = 0;_o < Zy.length; _o++)
    nl = Zy[_o], go[go[nl]] = nl;
  var nl, _o;
  Object.defineProperty(x, "codes", { enumerable: true, value: Object.freeze(go), writable: false });
  x.Deflate = Ht;
  x.Inflate = Vt;
  x.Gzip = Yt;
  x.Gunzip = Kt;
  x.DeflateRaw = Xt;
  x.InflateRaw = Jt;
  x.Unzip = Qt;
  x.createDeflate = function(e) {
    return new Ht(e);
  };
  x.createInflate = function(e) {
    return new Vt(e);
  };
  x.createDeflateRaw = function(e) {
    return new Xt(e);
  };
  x.createInflateRaw = function(e) {
    return new Jt(e);
  };
  x.createGzip = function(e) {
    return new Yt(e);
  };
  x.createGunzip = function(e) {
    return new Kt(e);
  };
  x.createUnzip = function(e) {
    return new Qt(e);
  };
  x.deflate = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Ht(t), e, r);
  };
  x.deflateSync = function(e, t) {
    return tr(new Ht(t), e);
  };
  x.gzip = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Yt(t), e, r);
  };
  x.gzipSync = function(e, t) {
    return tr(new Yt(t), e);
  };
  x.deflateRaw = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Xt(t), e, r);
  };
  x.deflateRawSync = function(e, t) {
    return tr(new Xt(t), e);
  };
  x.unzip = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Qt(t), e, r);
  };
  x.unzipSync = function(e, t) {
    return tr(new Qt(t), e);
  };
  x.inflate = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Vt(t), e, r);
  };
  x.inflateSync = function(e, t) {
    return tr(new Vt(t), e);
  };
  x.gunzip = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Kt(t), e, r);
  };
  x.gunzipSync = function(e, t) {
    return tr(new Kt(t), e);
  };
  x.inflateRaw = function(e, t, r) {
    return typeof t == "function" && (r = t, t = {}), er(new Jt(t), e, r);
  };
  x.inflateRawSync = function(e, t) {
    return tr(new Jt(t), e);
  };
  function er(e, t, r) {
    var n = [], i = 0;
    e.on("error", a), e.on("end", f), e.end(t), o();
    function o() {
      for (var u;(u = e.read()) !== null; )
        n.push(u), i += u.length;
      e.once("readable", o);
    }
    function a(u) {
      e.removeListener("end", f), e.removeListener("readable", o), r(u);
    }
    function f() {
      var u, l = null;
      i >= il ? l = new RangeError(Hy) : u = Ve.concat(n, i), n = [], e.close(), r(l, u);
    }
  }
  function tr(e, t) {
    if (typeof t == "string" && (t = Ve.from(t)), !Ve.isBuffer(t))
      throw new TypeError("Not a string or buffer");
    var r = e._finishFlushFlag;
    return e._processChunk(t, r);
  }
  function Ht(e) {
    if (!(this instanceof Ht))
      return new Ht(e);
    Z.call(this, e, N.DEFLATE);
  }
  function Vt(e) {
    if (!(this instanceof Vt))
      return new Vt(e);
    Z.call(this, e, N.INFLATE);
  }
  function Yt(e) {
    if (!(this instanceof Yt))
      return new Yt(e);
    Z.call(this, e, N.GZIP);
  }
  function Kt(e) {
    if (!(this instanceof Kt))
      return new Kt(e);
    Z.call(this, e, N.GUNZIP);
  }
  function Xt(e) {
    if (!(this instanceof Xt))
      return new Xt(e);
    Z.call(this, e, N.DEFLATERAW);
  }
  function Jt(e) {
    if (!(this instanceof Jt))
      return new Jt(e);
    Z.call(this, e, N.INFLATERAW);
  }
  function Qt(e) {
    if (!(this instanceof Qt))
      return new Qt(e);
    Z.call(this, e, N.UNZIP);
  }
  function $y(e) {
    return e === N.Z_NO_FLUSH || e === N.Z_PARTIAL_FLUSH || e === N.Z_SYNC_FLUSH || e === N.Z_FULL_FLUSH || e === N.Z_FINISH || e === N.Z_BLOCK;
  }
  function Z(e, t) {
    var r = this;
    if (this._opts = e = e || {}, this._chunkSize = e.chunkSize || x.Z_DEFAULT_CHUNK, Gy.call(this, e), e.flush && !$y(e.flush))
      throw new Error("Invalid flush flag: " + e.flush);
    if (e.finishFlush && !$y(e.finishFlush))
      throw new Error("Invalid flush flag: " + e.finishFlush);
    if (this._flushFlag = e.flush || N.Z_NO_FLUSH, this._finishFlushFlag = typeof e.finishFlush < "u" ? e.finishFlush : N.Z_FINISH, e.chunkSize && (e.chunkSize < x.Z_MIN_CHUNK || e.chunkSize > x.Z_MAX_CHUNK))
      throw new Error("Invalid chunk size: " + e.chunkSize);
    if (e.windowBits && (e.windowBits < x.Z_MIN_WINDOWBITS || e.windowBits > x.Z_MAX_WINDOWBITS))
      throw new Error("Invalid windowBits: " + e.windowBits);
    if (e.level && (e.level < x.Z_MIN_LEVEL || e.level > x.Z_MAX_LEVEL))
      throw new Error("Invalid compression level: " + e.level);
    if (e.memLevel && (e.memLevel < x.Z_MIN_MEMLEVEL || e.memLevel > x.Z_MAX_MEMLEVEL))
      throw new Error("Invalid memLevel: " + e.memLevel);
    if (e.strategy && e.strategy != x.Z_FILTERED && e.strategy != x.Z_HUFFMAN_ONLY && e.strategy != x.Z_RLE && e.strategy != x.Z_FIXED && e.strategy != x.Z_DEFAULT_STRATEGY)
      throw new Error("Invalid strategy: " + e.strategy);
    if (e.dictionary && !Ve.isBuffer(e.dictionary))
      throw new Error("Invalid dictionary: it should be a Buffer instance");
    this._handle = new N.Zlib(t);
    var n = this;
    this._hadError = false, this._handle.onerror = function(a, f) {
      bo(n), n._hadError = true;
      var u = new Error(a);
      u.errno = f, u.code = x.codes[f], n.emit("error", u);
    };
    var i = x.Z_DEFAULT_COMPRESSION;
    typeof e.level == "number" && (i = e.level);
    var o = x.Z_DEFAULT_STRATEGY;
    typeof e.strategy == "number" && (o = e.strategy), this._handle.init(e.windowBits || x.Z_DEFAULT_WINDOWBITS, i, e.memLevel || x.Z_DEFAULT_MEMLEVEL, o, e.dictionary), this._buffer = Ve.allocUnsafe(this._chunkSize), this._offset = 0, this._level = i, this._strategy = o, this.once("end", this.close), Object.defineProperty(this, "_closed", { get: function() {
      return !r._handle;
    }, configurable: true, enumerable: true });
  }
  Et.inherits(Z, Gy);
  Z.prototype.params = function(e, t, r) {
    if (e < x.Z_MIN_LEVEL || e > x.Z_MAX_LEVEL)
      throw new RangeError("Invalid compression level: " + e);
    if (t != x.Z_FILTERED && t != x.Z_HUFFMAN_ONLY && t != x.Z_RLE && t != x.Z_FIXED && t != x.Z_DEFAULT_STRATEGY)
      throw new TypeError("Invalid strategy: " + t);
    if (this._level !== e || this._strategy !== t) {
      var n = this;
      this.flush(N.Z_SYNC_FLUSH, function() {
        In(n._handle, "zlib binding closed"), n._handle.params(e, t), n._hadError || (n._level = e, n._strategy = t, r && r());
      });
    } else
      process.nextTick(r);
  };
  Z.prototype.reset = function() {
    return In(this._handle, "zlib binding closed"), this._handle.reset();
  };
  Z.prototype._flush = function(e) {
    this._transform(Ve.alloc(0), "", e);
  };
  Z.prototype.flush = function(e, t) {
    var r = this, n = this._writableState;
    (typeof e == "function" || e === undefined && !t) && (t = e, e = N.Z_FULL_FLUSH), n.ended ? t && process.nextTick(t) : n.ending ? t && this.once("end", t) : n.needDrain ? t && this.once("drain", function() {
      return r.flush(e, t);
    }) : (this._flushFlag = e, this.write(Ve.alloc(0), "", t));
  };
  Z.prototype.close = function(e) {
    bo(this, e), process.nextTick(wA, this);
  };
  function bo(e, t) {
    t && process.nextTick(t), e._handle && (e._handle.close(), e._handle = null);
  }
  function wA(e) {
    e.emit("close");
  }
  Z.prototype._transform = function(e, t, r) {
    var n, i = this._writableState, o = i.ending || i.ended, a = o && (!e || i.length === e.length);
    if (e !== null && !Ve.isBuffer(e))
      return r(new Error("invalid input"));
    if (!this._handle)
      return r(new Error("zlib binding closed"));
    a ? n = this._finishFlushFlag : (n = this._flushFlag, e.length >= i.length && (this._flushFlag = this._opts.flush || N.Z_NO_FLUSH)), this._processChunk(e, n, r);
  };
  Z.prototype._processChunk = function(e, t, r) {
    var n = e && e.length, i = this._chunkSize - this._offset, o = 0, a = this, f = typeof r == "function";
    if (!f) {
      var u = [], l = 0, s;
      this.on("error", function(b) {
        s = b;
      }), In(this._handle, "zlib binding closed");
      do
        var c = this._handle.writeSync(t, e, o, n, this._buffer, this._offset, i);
      while (!this._hadError && y(c[0], c[1]));
      if (this._hadError)
        throw s;
      if (l >= il)
        throw bo(this), new RangeError(Hy);
      var h = Ve.concat(u, l);
      return bo(this), h;
    }
    In(this._handle, "zlib binding closed");
    var d = this._handle.write(t, e, o, n, this._buffer, this._offset, i);
    d.buffer = e, d.callback = y;
    function y(b, R) {
      if (this && (this.buffer = null, this.callback = null), !a._hadError) {
        var _ = i - R;
        if (In(_ >= 0, "have should not go down"), _ > 0) {
          var E = a._buffer.slice(a._offset, a._offset + _);
          a._offset += _, f ? a.push(E) : (u.push(E), l += E.length);
        }
        if ((R === 0 || a._offset >= a._chunkSize) && (i = a._chunkSize, a._offset = 0, a._buffer = Ve.allocUnsafe(a._chunkSize)), R === 0) {
          if (o += n - b, n = b, !f)
            return true;
          var m = a._handle.write(t, e, o, n, a._buffer, a._offset, a._chunkSize);
          m.callback = y, m.buffer = e;
          return;
        }
        if (!f)
          return false;
        r();
      }
    }
  };
  Et.inherits(Ht, Z);
  Et.inherits(Vt, Z);
  Et.inherits(Yt, Z);
  Et.inherits(Kt, Z);
  Et.inherits(Xt, Z);
  Et.inherits(Jt, Z);
  Et.inherits(Qt, Z);
});
var Tn = {};
Bn(Tn, { default: () => EA });
X(Tn, vt(ol()));
var EA = vt(ol());
/*!
 * The buffer module from node.js, for the browser.
 *
 * @author   Feross Aboukhadijeh <feross@feross.org> <http://feross.org>
 * @license  MIT
 */
/*!
 * The buffer module from node.js, for the browser.
 *
 * @author   Feross Aboukhadijeh <https://feross.org>
 * @license  MIT
 */
/*! ieee754. BSD-3-Clause License. Feross Aboukhadijeh <https://feross.org/opensource> */
/*! safe-buffer. MIT License. Feross Aboukhadijeh <https://feross.org/opensource> */
export {
  EA as default
};
