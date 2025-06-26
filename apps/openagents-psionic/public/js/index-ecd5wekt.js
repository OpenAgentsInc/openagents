import"./chat-client-13a4mv5g.js";

// node:util
var pt = Object.create;
var dr = Object.defineProperty;
var lt = Object.getOwnPropertyDescriptor;
var gt = Object.getOwnPropertyNames;
var dt = Object.getPrototypeOf;
var bt = Object.prototype.hasOwnProperty;
var p = (r, e) => () => (e || r((e = { exports: {} }).exports, e), e.exports);
var At = (r, e) => {
  for (var t in e)
    dr(r, t, { get: e[t], enumerable: true });
};
var gr = (r, e, t, n) => {
  if (e && typeof e == "object" || typeof e == "function")
    for (let o of gt(e))
      !bt.call(r, o) && o !== t && dr(r, o, { get: () => e[o], enumerable: !(n = lt(e, o)) || n.enumerable });
  return r;
};
var F = (r, e, t) => (gr(r, e, "default"), t && gr(t, e, "default"));
var mt = (r, e, t) => (t = r != null ? pt(dt(r)) : {}, gr(e || !r || !r.__esModule ? dr(t, "default", { value: r, enumerable: true }) : t, r));
var br = p((po, Vr) => {
  Vr.exports = function() {
    if (typeof Symbol != "function" || typeof Object.getOwnPropertySymbols != "function")
      return false;
    if (typeof Symbol.iterator == "symbol")
      return true;
    var e = {}, t = Symbol("test"), n = Object(t);
    if (typeof t == "string" || Object.prototype.toString.call(t) !== "[object Symbol]" || Object.prototype.toString.call(n) !== "[object Symbol]")
      return false;
    var o = 42;
    e[t] = o;
    for (t in e)
      return false;
    if (typeof Object.keys == "function" && Object.keys(e).length !== 0 || typeof Object.getOwnPropertyNames == "function" && Object.getOwnPropertyNames(e).length !== 0)
      return false;
    var i = Object.getOwnPropertySymbols(e);
    if (i.length !== 1 || i[0] !== t || !Object.prototype.propertyIsEnumerable.call(e, t))
      return false;
    if (typeof Object.getOwnPropertyDescriptor == "function") {
      var a = Object.getOwnPropertyDescriptor(e, t);
      if (a.value !== o || a.enumerable !== true)
        return false;
    }
    return true;
  };
});
var N = p((lo, Jr) => {
  var ht = br();
  Jr.exports = function() {
    return ht() && !!Symbol.toStringTag;
  };
});
var Zr = p((go, Hr) => {
  var Lr = typeof Symbol < "u" && Symbol, St = br();
  Hr.exports = function() {
    return typeof Lr != "function" || typeof Symbol != "function" || typeof Lr("foo") != "symbol" || typeof Symbol("bar") != "symbol" ? false : St();
  };
});
var Kr = p((bo, Yr) => {
  var vt = "Function.prototype.bind called on incompatible ", Ar = Array.prototype.slice, Ot = Object.prototype.toString, jt = "[object Function]";
  Yr.exports = function(e) {
    var t = this;
    if (typeof t != "function" || Ot.call(t) !== jt)
      throw new TypeError(vt + t);
    for (var n = Ar.call(arguments, 1), o, i = function() {
      if (this instanceof o) {
        var g = t.apply(this, n.concat(Ar.call(arguments)));
        return Object(g) === g ? g : this;
      } else
        return t.apply(e, n.concat(Ar.call(arguments)));
    }, a = Math.max(0, t.length - n.length), f = [], c = 0;c < a; c++)
      f.push("$" + c);
    if (o = Function("binder", "return function (" + f.join(",") + "){ return binder.apply(this,arguments); }")(i), t.prototype) {
      var l = function() {
      };
      l.prototype = t.prototype, o.prototype = new l, l.prototype = null;
    }
    return o;
  };
});
var V = p((Ao, Qr) => {
  var Pt = Kr();
  Qr.exports = Function.prototype.bind || Pt;
});
var re = p((mo, Xr) => {
  var wt = V();
  Xr.exports = wt.call(Function.call, Object.prototype.hasOwnProperty);
});
var H = p((ho, ie) => {
  var s, x = SyntaxError, oe = Function, U = TypeError, mr = function(r) {
    try {
      return oe('"use strict"; return (' + r + ").constructor;")();
    } catch {
    }
  }, v = Object.getOwnPropertyDescriptor;
  if (v)
    try {
      v({}, "");
    } catch {
      v = null;
    }
  var hr = function() {
    throw new U;
  }, Et = v ? function() {
    try {
      return arguments.callee, hr;
    } catch {
      try {
        return v(arguments, "callee").get;
      } catch {
        return hr;
      }
    }
  }() : hr, I = Zr()(), m = Object.getPrototypeOf || function(r) {
    return r.__proto__;
  }, B = {}, Tt = typeof Uint8Array > "u" ? s : m(Uint8Array), O = { "%AggregateError%": typeof AggregateError > "u" ? s : AggregateError, "%Array%": Array, "%ArrayBuffer%": typeof ArrayBuffer > "u" ? s : ArrayBuffer, "%ArrayIteratorPrototype%": I ? m([][Symbol.iterator]()) : s, "%AsyncFromSyncIteratorPrototype%": s, "%AsyncFunction%": B, "%AsyncGenerator%": B, "%AsyncGeneratorFunction%": B, "%AsyncIteratorPrototype%": B, "%Atomics%": typeof Atomics > "u" ? s : Atomics, "%BigInt%": typeof BigInt > "u" ? s : BigInt, "%BigInt64Array%": typeof BigInt64Array > "u" ? s : BigInt64Array, "%BigUint64Array%": typeof BigUint64Array > "u" ? s : BigUint64Array, "%Boolean%": Boolean, "%DataView%": typeof DataView > "u" ? s : DataView, "%Date%": Date, "%decodeURI%": decodeURI, "%decodeURIComponent%": decodeURIComponent, "%encodeURI%": encodeURI, "%encodeURIComponent%": encodeURIComponent, "%Error%": Error, "%eval%": eval, "%EvalError%": EvalError, "%Float32Array%": typeof Float32Array > "u" ? s : Float32Array, "%Float64Array%": typeof Float64Array > "u" ? s : Float64Array, "%FinalizationRegistry%": typeof FinalizationRegistry > "u" ? s : FinalizationRegistry, "%Function%": oe, "%GeneratorFunction%": B, "%Int8Array%": typeof Int8Array > "u" ? s : Int8Array, "%Int16Array%": typeof Int16Array > "u" ? s : Int16Array, "%Int32Array%": typeof Int32Array > "u" ? s : Int32Array, "%isFinite%": isFinite, "%isNaN%": isNaN, "%IteratorPrototype%": I ? m(m([][Symbol.iterator]())) : s, "%JSON%": typeof JSON == "object" ? JSON : s, "%Map%": typeof Map > "u" ? s : Map, "%MapIteratorPrototype%": typeof Map > "u" || !I ? s : m(new Map()[Symbol.iterator]()), "%Math%": Math, "%Number%": Number, "%Object%": Object, "%parseFloat%": parseFloat, "%parseInt%": parseInt, "%Promise%": typeof Promise > "u" ? s : Promise, "%Proxy%": typeof Proxy > "u" ? s : Proxy, "%RangeError%": RangeError, "%ReferenceError%": ReferenceError, "%Reflect%": typeof Reflect > "u" ? s : Reflect, "%RegExp%": RegExp, "%Set%": typeof Set > "u" ? s : Set, "%SetIteratorPrototype%": typeof Set > "u" || !I ? s : m(new Set()[Symbol.iterator]()), "%SharedArrayBuffer%": typeof SharedArrayBuffer > "u" ? s : SharedArrayBuffer, "%String%": String, "%StringIteratorPrototype%": I ? m(""[Symbol.iterator]()) : s, "%Symbol%": I ? Symbol : s, "%SyntaxError%": x, "%ThrowTypeError%": Et, "%TypedArray%": Tt, "%TypeError%": U, "%Uint8Array%": typeof Uint8Array > "u" ? s : Uint8Array, "%Uint8ClampedArray%": typeof Uint8ClampedArray > "u" ? s : Uint8ClampedArray, "%Uint16Array%": typeof Uint16Array > "u" ? s : Uint16Array, "%Uint32Array%": typeof Uint32Array > "u" ? s : Uint32Array, "%URIError%": URIError, "%WeakMap%": typeof WeakMap > "u" ? s : WeakMap, "%WeakRef%": typeof WeakRef > "u" ? s : WeakRef, "%WeakSet%": typeof WeakSet > "u" ? s : WeakSet };
  try {
    null.error;
  } catch (r) {
    ee = m(m(r)), O["%Error.prototype%"] = ee;
  }
  var ee, Ft = function r(e) {
    var t;
    if (e === "%AsyncFunction%")
      t = mr("async function () {}");
    else if (e === "%GeneratorFunction%")
      t = mr("function* () {}");
    else if (e === "%AsyncGeneratorFunction%")
      t = mr("async function* () {}");
    else if (e === "%AsyncGenerator%") {
      var n = r("%AsyncGeneratorFunction%");
      n && (t = n.prototype);
    } else if (e === "%AsyncIteratorPrototype%") {
      var o = r("%AsyncGenerator%");
      o && (t = m(o.prototype));
    }
    return O[e] = t, t;
  }, te = { "%ArrayBufferPrototype%": ["ArrayBuffer", "prototype"], "%ArrayPrototype%": ["Array", "prototype"], "%ArrayProto_entries%": ["Array", "prototype", "entries"], "%ArrayProto_forEach%": ["Array", "prototype", "forEach"], "%ArrayProto_keys%": ["Array", "prototype", "keys"], "%ArrayProto_values%": ["Array", "prototype", "values"], "%AsyncFunctionPrototype%": ["AsyncFunction", "prototype"], "%AsyncGenerator%": ["AsyncGeneratorFunction", "prototype"], "%AsyncGeneratorPrototype%": ["AsyncGeneratorFunction", "prototype", "prototype"], "%BooleanPrototype%": ["Boolean", "prototype"], "%DataViewPrototype%": ["DataView", "prototype"], "%DatePrototype%": ["Date", "prototype"], "%ErrorPrototype%": ["Error", "prototype"], "%EvalErrorPrototype%": ["EvalError", "prototype"], "%Float32ArrayPrototype%": ["Float32Array", "prototype"], "%Float64ArrayPrototype%": ["Float64Array", "prototype"], "%FunctionPrototype%": ["Function", "prototype"], "%Generator%": ["GeneratorFunction", "prototype"], "%GeneratorPrototype%": ["GeneratorFunction", "prototype", "prototype"], "%Int8ArrayPrototype%": ["Int8Array", "prototype"], "%Int16ArrayPrototype%": ["Int16Array", "prototype"], "%Int32ArrayPrototype%": ["Int32Array", "prototype"], "%JSONParse%": ["JSON", "parse"], "%JSONStringify%": ["JSON", "stringify"], "%MapPrototype%": ["Map", "prototype"], "%NumberPrototype%": ["Number", "prototype"], "%ObjectPrototype%": ["Object", "prototype"], "%ObjProto_toString%": ["Object", "prototype", "toString"], "%ObjProto_valueOf%": ["Object", "prototype", "valueOf"], "%PromisePrototype%": ["Promise", "prototype"], "%PromiseProto_then%": ["Promise", "prototype", "then"], "%Promise_all%": ["Promise", "all"], "%Promise_reject%": ["Promise", "reject"], "%Promise_resolve%": ["Promise", "resolve"], "%RangeErrorPrototype%": ["RangeError", "prototype"], "%ReferenceErrorPrototype%": ["ReferenceError", "prototype"], "%RegExpPrototype%": ["RegExp", "prototype"], "%SetPrototype%": ["Set", "prototype"], "%SharedArrayBufferPrototype%": ["SharedArrayBuffer", "prototype"], "%StringPrototype%": ["String", "prototype"], "%SymbolPrototype%": ["Symbol", "prototype"], "%SyntaxErrorPrototype%": ["SyntaxError", "prototype"], "%TypedArrayPrototype%": ["TypedArray", "prototype"], "%TypeErrorPrototype%": ["TypeError", "prototype"], "%Uint8ArrayPrototype%": ["Uint8Array", "prototype"], "%Uint8ClampedArrayPrototype%": ["Uint8ClampedArray", "prototype"], "%Uint16ArrayPrototype%": ["Uint16Array", "prototype"], "%Uint32ArrayPrototype%": ["Uint32Array", "prototype"], "%URIErrorPrototype%": ["URIError", "prototype"], "%WeakMapPrototype%": ["WeakMap", "prototype"], "%WeakSetPrototype%": ["WeakSet", "prototype"] }, C = V(), J = re(), It = C.call(Function.call, Array.prototype.concat), Bt = C.call(Function.apply, Array.prototype.splice), ne = C.call(Function.call, String.prototype.replace), L = C.call(Function.call, String.prototype.slice), Ut = C.call(Function.call, RegExp.prototype.exec), xt = /[^%.[\]]+|\[(?:(-?\d+(?:\.\d+)?)|(["'])((?:(?!\2)[^\\]|\\.)*?)\2)\]|(?=(?:\.|\[\])(?:\.|\[\]|%$))/g, Dt = /\\(\\)?/g, Rt = function(e) {
    var t = L(e, 0, 1), n = L(e, -1);
    if (t === "%" && n !== "%")
      throw new x("invalid intrinsic syntax, expected closing `%`");
    if (n === "%" && t !== "%")
      throw new x("invalid intrinsic syntax, expected opening `%`");
    var o = [];
    return ne(e, xt, function(i, a, f, c) {
      o[o.length] = f ? ne(c, Dt, "$1") : a || i;
    }), o;
  }, kt = function(e, t) {
    var n = e, o;
    if (J(te, n) && (o = te[n], n = "%" + o[0] + "%"), J(O, n)) {
      var i = O[n];
      if (i === B && (i = Ft(n)), typeof i > "u" && !t)
        throw new U("intrinsic " + e + " exists, but is not available. Please file an issue!");
      return { alias: o, name: n, value: i };
    }
    throw new x("intrinsic " + e + " does not exist!");
  };
  ie.exports = function(e, t) {
    if (typeof e != "string" || e.length === 0)
      throw new U("intrinsic name must be a non-empty string");
    if (arguments.length > 1 && typeof t != "boolean")
      throw new U('"allowMissing" argument must be a boolean');
    if (Ut(/^%?[^%]*%?$/, e) === null)
      throw new x("`%` may not be present anywhere but at the beginning and end of the intrinsic name");
    var n = Rt(e), o = n.length > 0 ? n[0] : "", i = kt("%" + o + "%", t), a = i.name, f = i.value, c = false, l = i.alias;
    l && (o = l[0], Bt(n, It([0, 1], l)));
    for (var g = 1, S = true;g < n.length; g += 1) {
      var d = n[g], T = L(d, 0, 1), _ = L(d, -1);
      if ((T === '"' || T === "'" || T === "`" || _ === '"' || _ === "'" || _ === "`") && T !== _)
        throw new x("property names with quotes must have matching quotes");
      if ((d === "constructor" || !S) && (c = true), o += "." + d, a = "%" + o + "%", J(O, a))
        f = O[a];
      else if (f != null) {
        if (!(d in f)) {
          if (!t)
            throw new U("base intrinsic for " + e + " exists, but the property is not available.");
          return;
        }
        if (v && g + 1 >= n.length) {
          var z = v(f, d);
          S = !!z, S && "get" in z && !("originalValue" in z.get) ? f = z.get : f = f[d];
        } else
          S = J(f, d), f = f[d];
        S && !c && (O[a] = f);
      }
    }
    return f;
  };
});
var ce = p((So, Z) => {
  var Sr = V(), D = H(), ue = D("%Function.prototype.apply%"), ye = D("%Function.prototype.call%"), se = D("%Reflect.apply%", true) || Sr.call(ye, ue), ae = D("%Object.getOwnPropertyDescriptor%", true), j = D("%Object.defineProperty%", true), Mt = D("%Math.max%");
  if (j)
    try {
      j({}, "a", { value: 1 });
    } catch {
      j = null;
    }
  Z.exports = function(e) {
    var t = se(Sr, ye, arguments);
    if (ae && j) {
      var n = ae(t, "length");
      n.configurable && j(t, "length", { value: 1 + Mt(0, e.length - (arguments.length - 1)) });
    }
    return t;
  };
  var fe = function() {
    return se(Sr, ue, arguments);
  };
  j ? j(Z.exports, "apply", { value: fe }) : Z.exports.apply = fe;
});
var Y = p((vo, ge) => {
  var pe = H(), le = ce(), Nt = le(pe("String.prototype.indexOf"));
  ge.exports = function(e, t) {
    var n = pe(e, !!t);
    return typeof n == "function" && Nt(e, ".prototype.") > -1 ? le(n) : n;
  };
});
var Ae = p((Oo, be) => {
  var Ct = N()(), $t = Y(), vr = $t("Object.prototype.toString"), K = function(e) {
    return Ct && e && typeof e == "object" && Symbol.toStringTag in e ? false : vr(e) === "[object Arguments]";
  }, de = function(e) {
    return K(e) ? true : e !== null && typeof e == "object" && typeof e.length == "number" && e.length >= 0 && vr(e) !== "[object Array]" && vr(e.callee) === "[object Function]";
  }, qt = function() {
    return K(arguments);
  }();
  K.isLegacyArguments = de;
  be.exports = qt ? K : de;
});
var Se = p((jo, he) => {
  var Gt = Object.prototype.toString, Wt = Function.prototype.toString, _t = /^\s*(?:function)?\*/, me = N()(), Or = Object.getPrototypeOf, zt = function() {
    if (!me)
      return false;
    try {
      return Function("return function*() {}")();
    } catch {
    }
  }, jr;
  he.exports = function(e) {
    if (typeof e != "function")
      return false;
    if (_t.test(Wt.call(e)))
      return true;
    if (!me) {
      var t = Gt.call(e);
      return t === "[object GeneratorFunction]";
    }
    if (!Or)
      return false;
    if (typeof jr > "u") {
      var n = zt();
      jr = n ? Or(n) : false;
    }
    return Or(e) === jr;
  };
});
var Pe = p((Po, je) => {
  var Oe = Function.prototype.toString, R = typeof Reflect == "object" && Reflect !== null && Reflect.apply, wr, Q;
  if (typeof R == "function" && typeof Object.defineProperty == "function")
    try {
      wr = Object.defineProperty({}, "length", { get: function() {
        throw Q;
      } }), Q = {}, R(function() {
        throw 42;
      }, null, wr);
    } catch (r) {
      r !== Q && (R = null);
    }
  else
    R = null;
  var Vt = /^\s*class\b/, Er = function(e) {
    try {
      var t = Oe.call(e);
      return Vt.test(t);
    } catch {
      return false;
    }
  }, Pr = function(e) {
    try {
      return Er(e) ? false : (Oe.call(e), true);
    } catch {
      return false;
    }
  }, X = Object.prototype.toString, Jt = "[object Object]", Lt = "[object Function]", Ht = "[object GeneratorFunction]", Zt = "[object HTMLAllCollection]", Yt = "[object HTML document.all class]", Kt = "[object HTMLCollection]", Qt = typeof Symbol == "function" && !!Symbol.toStringTag, Xt = !(0 in [,]), Tr = function() {
    return false;
  };
  typeof document == "object" && (ve = document.all, X.call(ve) === X.call(document.all) && (Tr = function(e) {
    if ((Xt || !e) && (typeof e > "u" || typeof e == "object"))
      try {
        var t = X.call(e);
        return (t === Zt || t === Yt || t === Kt || t === Jt) && e("") == null;
      } catch {
      }
    return false;
  }));
  var ve;
  je.exports = R ? function(e) {
    if (Tr(e))
      return true;
    if (!e || typeof e != "function" && typeof e != "object")
      return false;
    try {
      R(e, null, wr);
    } catch (t) {
      if (t !== Q)
        return false;
    }
    return !Er(e) && Pr(e);
  } : function(e) {
    if (Tr(e))
      return true;
    if (!e || typeof e != "function" && typeof e != "object")
      return false;
    if (Qt)
      return Pr(e);
    if (Er(e))
      return false;
    var t = X.call(e);
    return t !== Lt && t !== Ht && !/^\[object HTML/.test(t) ? false : Pr(e);
  };
});
var Fr = p((wo, Ee) => {
  var rn = Pe(), en = Object.prototype.toString, we = Object.prototype.hasOwnProperty, tn = function(e, t, n) {
    for (var o = 0, i = e.length;o < i; o++)
      we.call(e, o) && (n == null ? t(e[o], o, e) : t.call(n, e[o], o, e));
  }, nn = function(e, t, n) {
    for (var o = 0, i = e.length;o < i; o++)
      n == null ? t(e.charAt(o), o, e) : t.call(n, e.charAt(o), o, e);
  }, on = function(e, t, n) {
    for (var o in e)
      we.call(e, o) && (n == null ? t(e[o], o, e) : t.call(n, e[o], o, e));
  }, an = function(e, t, n) {
    if (!rn(t))
      throw new TypeError("iterator must be a function");
    var o;
    arguments.length >= 3 && (o = n), en.call(e) === "[object Array]" ? tn(e, t, o) : typeof e == "string" ? nn(e, t, o) : on(e, t, o);
  };
  Ee.exports = an;
});
var Br = p((Eo, Te) => {
  var Ir = ["BigInt64Array", "BigUint64Array", "Float32Array", "Float64Array", "Int16Array", "Int32Array", "Int8Array", "Uint16Array", "Uint32Array", "Uint8Array", "Uint8ClampedArray"], fn = typeof globalThis > "u" ? global : globalThis;
  Te.exports = function() {
    for (var e = [], t = 0;t < Ir.length; t++)
      typeof fn[Ir[t]] == "function" && (e[e.length] = Ir[t]);
    return e;
  };
});
var Ur = p((To, Fe) => {
  var un = H(), rr = un("%Object.getOwnPropertyDescriptor%", true);
  if (rr)
    try {
      rr([], "length");
    } catch {
      rr = null;
    }
  Fe.exports = rr;
});
var Rr = p((Fo, De) => {
  var Ie = Fr(), yn = Br(), Dr = Y(), sn = Dr("Object.prototype.toString"), Be = N()(), er = Ur(), cn = typeof globalThis > "u" ? global : globalThis, Ue = yn(), pn = Dr("Array.prototype.indexOf", true) || function(e, t) {
    for (var n = 0;n < e.length; n += 1)
      if (e[n] === t)
        return n;
    return -1;
  }, ln = Dr("String.prototype.slice"), xe = {}, xr = Object.getPrototypeOf;
  Be && er && xr && Ie(Ue, function(r) {
    var e = new cn[r];
    if (Symbol.toStringTag in e) {
      var t = xr(e), n = er(t, Symbol.toStringTag);
      if (!n) {
        var o = xr(t);
        n = er(o, Symbol.toStringTag);
      }
      xe[r] = n.get;
    }
  });
  var gn = function(e) {
    var t = false;
    return Ie(xe, function(n, o) {
      if (!t)
        try {
          t = n.call(e) === o;
        } catch {
        }
    }), t;
  };
  De.exports = function(e) {
    if (!e || typeof e != "object")
      return false;
    if (!Be || !(Symbol.toStringTag in e)) {
      var t = ln(sn(e), 8, -1);
      return pn(Ue, t) > -1;
    }
    return er ? gn(e) : false;
  };
});
var qe = p((Io, $e) => {
  var ke = Fr(), dn = Br(), Me = Y(), kr = Ur(), bn = Me("Object.prototype.toString"), Ne = N()(), Re = typeof globalThis > "u" ? global : globalThis, An = dn(), mn = Me("String.prototype.slice"), Ce = {}, Mr = Object.getPrototypeOf;
  Ne && kr && Mr && ke(An, function(r) {
    if (typeof Re[r] == "function") {
      var e = new Re[r];
      if (Symbol.toStringTag in e) {
        var t = Mr(e), n = kr(t, Symbol.toStringTag);
        if (!n) {
          var o = Mr(t);
          n = kr(o, Symbol.toStringTag);
        }
        Ce[r] = n.get;
      }
    }
  });
  var hn = function(e) {
    var t = false;
    return ke(Ce, function(n, o) {
      if (!t)
        try {
          var i = n.call(e);
          i === o && (t = i);
        } catch {
        }
    }), t;
  }, Sn = Rr();
  $e.exports = function(e) {
    return Sn(e) ? !Ne || !(Symbol.toStringTag in e) ? mn(bn(e), 8, -1) : hn(e) : false;
  };
});
var rt = p((u) => {
  var vn = Ae(), On = Se(), A = qe(), Ge = Rr();
  function k(r) {
    return r.call.bind(r);
  }
  var We = typeof BigInt < "u", _e = typeof Symbol < "u", b = k(Object.prototype.toString), jn = k(Number.prototype.valueOf), Pn = k(String.prototype.valueOf), wn = k(Boolean.prototype.valueOf);
  We && (ze = k(BigInt.prototype.valueOf));
  var ze;
  _e && (Ve = k(Symbol.prototype.valueOf));
  var Ve;
  function q(r, e) {
    if (typeof r != "object")
      return false;
    try {
      return e(r), true;
    } catch {
      return false;
    }
  }
  u.isArgumentsObject = vn;
  u.isGeneratorFunction = On;
  u.isTypedArray = Ge;
  function En(r) {
    return typeof Promise < "u" && r instanceof Promise || r !== null && typeof r == "object" && typeof r.then == "function" && typeof r.catch == "function";
  }
  u.isPromise = En;
  function Tn(r) {
    return typeof ArrayBuffer < "u" && ArrayBuffer.isView ? ArrayBuffer.isView(r) : Ge(r) || Le(r);
  }
  u.isArrayBufferView = Tn;
  function Fn(r) {
    return A(r) === "Uint8Array";
  }
  u.isUint8Array = Fn;
  function In(r) {
    return A(r) === "Uint8ClampedArray";
  }
  u.isUint8ClampedArray = In;
  function Bn(r) {
    return A(r) === "Uint16Array";
  }
  u.isUint16Array = Bn;
  function Un(r) {
    return A(r) === "Uint32Array";
  }
  u.isUint32Array = Un;
  function xn(r) {
    return A(r) === "Int8Array";
  }
  u.isInt8Array = xn;
  function Dn(r) {
    return A(r) === "Int16Array";
  }
  u.isInt16Array = Dn;
  function Rn(r) {
    return A(r) === "Int32Array";
  }
  u.isInt32Array = Rn;
  function kn(r) {
    return A(r) === "Float32Array";
  }
  u.isFloat32Array = kn;
  function Mn(r) {
    return A(r) === "Float64Array";
  }
  u.isFloat64Array = Mn;
  function Nn(r) {
    return A(r) === "BigInt64Array";
  }
  u.isBigInt64Array = Nn;
  function Cn(r) {
    return A(r) === "BigUint64Array";
  }
  u.isBigUint64Array = Cn;
  function tr(r) {
    return b(r) === "[object Map]";
  }
  tr.working = typeof Map < "u" && tr(new Map);
  function $n(r) {
    return typeof Map > "u" ? false : tr.working ? tr(r) : r instanceof Map;
  }
  u.isMap = $n;
  function nr(r) {
    return b(r) === "[object Set]";
  }
  nr.working = typeof Set < "u" && nr(new Set);
  function qn(r) {
    return typeof Set > "u" ? false : nr.working ? nr(r) : r instanceof Set;
  }
  u.isSet = qn;
  function or(r) {
    return b(r) === "[object WeakMap]";
  }
  or.working = typeof WeakMap < "u" && or(new WeakMap);
  function Gn(r) {
    return typeof WeakMap > "u" ? false : or.working ? or(r) : r instanceof WeakMap;
  }
  u.isWeakMap = Gn;
  function Cr(r) {
    return b(r) === "[object WeakSet]";
  }
  Cr.working = typeof WeakSet < "u" && Cr(new WeakSet);
  function Wn(r) {
    return Cr(r);
  }
  u.isWeakSet = Wn;
  function ir(r) {
    return b(r) === "[object ArrayBuffer]";
  }
  ir.working = typeof ArrayBuffer < "u" && ir(new ArrayBuffer);
  function Je(r) {
    return typeof ArrayBuffer > "u" ? false : ir.working ? ir(r) : r instanceof ArrayBuffer;
  }
  u.isArrayBuffer = Je;
  function ar(r) {
    return b(r) === "[object DataView]";
  }
  ar.working = typeof ArrayBuffer < "u" && typeof DataView < "u" && ar(new DataView(new ArrayBuffer(1), 0, 1));
  function Le(r) {
    return typeof DataView > "u" ? false : ar.working ? ar(r) : r instanceof DataView;
  }
  u.isDataView = Le;
  var Nr = typeof SharedArrayBuffer < "u" ? SharedArrayBuffer : undefined;
  function $(r) {
    return b(r) === "[object SharedArrayBuffer]";
  }
  function He(r) {
    return typeof Nr > "u" ? false : (typeof $.working > "u" && ($.working = $(new Nr)), $.working ? $(r) : r instanceof Nr);
  }
  u.isSharedArrayBuffer = He;
  function _n(r) {
    return b(r) === "[object AsyncFunction]";
  }
  u.isAsyncFunction = _n;
  function zn(r) {
    return b(r) === "[object Map Iterator]";
  }
  u.isMapIterator = zn;
  function Vn(r) {
    return b(r) === "[object Set Iterator]";
  }
  u.isSetIterator = Vn;
  function Jn(r) {
    return b(r) === "[object Generator]";
  }
  u.isGeneratorObject = Jn;
  function Ln(r) {
    return b(r) === "[object WebAssembly.Module]";
  }
  u.isWebAssemblyCompiledModule = Ln;
  function Ze(r) {
    return q(r, jn);
  }
  u.isNumberObject = Ze;
  function Ye(r) {
    return q(r, Pn);
  }
  u.isStringObject = Ye;
  function Ke(r) {
    return q(r, wn);
  }
  u.isBooleanObject = Ke;
  function Qe(r) {
    return We && q(r, ze);
  }
  u.isBigIntObject = Qe;
  function Xe(r) {
    return _e && q(r, Ve);
  }
  u.isSymbolObject = Xe;
  function Hn(r) {
    return Ze(r) || Ye(r) || Ke(r) || Qe(r) || Xe(r);
  }
  u.isBoxedPrimitive = Hn;
  function Zn(r) {
    return typeof Uint8Array < "u" && (Je(r) || He(r));
  }
  u.isAnyArrayBuffer = Zn;
  ["isProxy", "isExternal", "isModuleNamespaceObject"].forEach(function(r) {
    Object.defineProperty(u, r, { enumerable: false, value: function() {
      throw new Error(r + " is not supported in userland");
    } });
  });
});
var tt = p((Uo, et) => {
  et.exports = function(e) {
    return e && typeof e == "object" && typeof e.copy == "function" && typeof e.fill == "function" && typeof e.readUInt8 == "function";
  };
});
var nt = p((xo, $r) => {
  typeof Object.create == "function" ? $r.exports = function(e, t) {
    t && (e.super_ = t, e.prototype = Object.create(t.prototype, { constructor: { value: e, enumerable: false, writable: true, configurable: true } }));
  } : $r.exports = function(e, t) {
    if (t) {
      e.super_ = t;
      var n = function() {
      };
      n.prototype = t.prototype, e.prototype = new n, e.prototype.constructor = e;
    }
  };
});
var yt = p((y) => {
  var ot = Object.getOwnPropertyDescriptors || function(e) {
    for (var t = Object.keys(e), n = {}, o = 0;o < t.length; o++)
      n[t[o]] = Object.getOwnPropertyDescriptor(e, t[o]);
    return n;
  }, Yn = /%[sdj%]/g;
  y.format = function(r) {
    if (!lr(r)) {
      for (var e = [], t = 0;t < arguments.length; t++)
        e.push(h(arguments[t]));
      return e.join(" ");
    }
    for (var t = 1, n = arguments, o = n.length, i = String(r).replace(Yn, function(f) {
      if (f === "%%")
        return "%";
      if (t >= o)
        return f;
      switch (f) {
        case "%s":
          return String(n[t++]);
        case "%d":
          return Number(n[t++]);
        case "%j":
          try {
            return JSON.stringify(n[t++]);
          } catch {
            return "[Circular]";
          }
        default:
          return f;
      }
    }), a = n[t];t < o; a = n[++t])
      pr(a) || !M(a) ? i += " " + a : i += " " + h(a);
    return i;
  };
  y.deprecate = function(r, e) {
    if (typeof process < "u" && process.noDeprecation === true)
      return r;
    if (typeof process > "u")
      return function() {
        return y.deprecate(r, e).apply(this, arguments);
      };
    var t = false;
    function n() {
      if (!t) {
        if (process.throwDeprecation)
          throw new Error(e);
        process.traceDeprecation ? console.trace(e) : console.error(e), t = true;
      }
      return r.apply(this, arguments);
    }
    return n;
  };
  var fr = {}, it = /^$/;
  process.env.NODE_DEBUG && (ur = process.env.NODE_DEBUG, ur = ur.replace(/[|\\{}()[\]^$+?.]/g, "\\$&").replace(/\*/g, ".*").replace(/,/g, "$|^").toUpperCase(), it = new RegExp("^" + ur + "$", "i"));
  var ur;
  y.debuglog = function(r) {
    if (r = r.toUpperCase(), !fr[r])
      if (it.test(r)) {
        var e = process.pid;
        fr[r] = function() {
          var t = y.format.apply(y, arguments);
          console.error("%s %d: %s", r, e, t);
        };
      } else
        fr[r] = function() {
        };
    return fr[r];
  };
  function h(r, e) {
    var t = { seen: [], stylize: Qn };
    return arguments.length >= 3 && (t.depth = arguments[2]), arguments.length >= 4 && (t.colors = arguments[3]), _r(e) ? t.showHidden = e : e && y._extend(t, e), w(t.showHidden) && (t.showHidden = false), w(t.depth) && (t.depth = 2), w(t.colors) && (t.colors = false), w(t.customInspect) && (t.customInspect = true), t.colors && (t.stylize = Kn), sr(t, r, t.depth);
  }
  y.inspect = h;
  h.colors = { bold: [1, 22], italic: [3, 23], underline: [4, 24], inverse: [7, 27], white: [37, 39], grey: [90, 39], black: [30, 39], blue: [34, 39], cyan: [36, 39], green: [32, 39], magenta: [35, 39], red: [31, 39], yellow: [33, 39] };
  h.styles = { special: "cyan", number: "yellow", boolean: "yellow", undefined: "grey", null: "bold", string: "green", date: "magenta", regexp: "red" };
  function Kn(r, e) {
    var t = h.styles[e];
    return t ? "\x1B[" + h.colors[t][0] + "m" + r + "\x1B[" + h.colors[t][1] + "m" : r;
  }
  function Qn(r, e) {
    return r;
  }
  function Xn(r) {
    var e = {};
    return r.forEach(function(t, n) {
      e[t] = true;
    }), e;
  }
  function sr(r, e, t) {
    if (r.customInspect && e && yr(e.inspect) && e.inspect !== y.inspect && !(e.constructor && e.constructor.prototype === e)) {
      var n = e.inspect(t, r);
      return lr(n) || (n = sr(r, n, t)), n;
    }
    var o = ro(r, e);
    if (o)
      return o;
    var i = Object.keys(e), a = Xn(i);
    if (r.showHidden && (i = Object.getOwnPropertyNames(e)), W(e) && (i.indexOf("message") >= 0 || i.indexOf("description") >= 0))
      return qr(e);
    if (i.length === 0) {
      if (yr(e)) {
        var f = e.name ? ": " + e.name : "";
        return r.stylize("[Function" + f + "]", "special");
      }
      if (G(e))
        return r.stylize(RegExp.prototype.toString.call(e), "regexp");
      if (cr(e))
        return r.stylize(Date.prototype.toString.call(e), "date");
      if (W(e))
        return qr(e);
    }
    var c = "", l = false, g = ["{", "}"];
    if (at(e) && (l = true, g = ["[", "]"]), yr(e)) {
      var S = e.name ? ": " + e.name : "";
      c = " [Function" + S + "]";
    }
    if (G(e) && (c = " " + RegExp.prototype.toString.call(e)), cr(e) && (c = " " + Date.prototype.toUTCString.call(e)), W(e) && (c = " " + qr(e)), i.length === 0 && (!l || e.length == 0))
      return g[0] + c + g[1];
    if (t < 0)
      return G(e) ? r.stylize(RegExp.prototype.toString.call(e), "regexp") : r.stylize("[Object]", "special");
    r.seen.push(e);
    var d;
    return l ? d = eo(r, e, t, a, i) : d = i.map(function(T) {
      return Wr(r, e, t, a, T, l);
    }), r.seen.pop(), to(d, c, g);
  }
  function ro(r, e) {
    if (w(e))
      return r.stylize("undefined", "undefined");
    if (lr(e)) {
      var t = "'" + JSON.stringify(e).replace(/^"|"$/g, "").replace(/'/g, "\\'").replace(/\\"/g, '"') + "'";
      return r.stylize(t, "string");
    }
    if (ft(e))
      return r.stylize("" + e, "number");
    if (_r(e))
      return r.stylize("" + e, "boolean");
    if (pr(e))
      return r.stylize("null", "null");
  }
  function qr(r) {
    return "[" + Error.prototype.toString.call(r) + "]";
  }
  function eo(r, e, t, n, o) {
    for (var i = [], a = 0, f = e.length;a < f; ++a)
      ut(e, String(a)) ? i.push(Wr(r, e, t, n, String(a), true)) : i.push("");
    return o.forEach(function(c) {
      c.match(/^\d+$/) || i.push(Wr(r, e, t, n, c, true));
    }), i;
  }
  function Wr(r, e, t, n, o, i) {
    var a, f, c;
    if (c = Object.getOwnPropertyDescriptor(e, o) || { value: e[o] }, c.get ? c.set ? f = r.stylize("[Getter/Setter]", "special") : f = r.stylize("[Getter]", "special") : c.set && (f = r.stylize("[Setter]", "special")), ut(n, o) || (a = "[" + o + "]"), f || (r.seen.indexOf(c.value) < 0 ? (pr(t) ? f = sr(r, c.value, null) : f = sr(r, c.value, t - 1), f.indexOf(`
`) > -1 && (i ? f = f.split(`
`).map(function(l) {
      return "  " + l;
    }).join(`
`).slice(2) : f = `
` + f.split(`
`).map(function(l) {
      return "   " + l;
    }).join(`
`))) : f = r.stylize("[Circular]", "special")), w(a)) {
      if (i && o.match(/^\d+$/))
        return f;
      a = JSON.stringify("" + o), a.match(/^"([a-zA-Z_][a-zA-Z_0-9]*)"$/) ? (a = a.slice(1, -1), a = r.stylize(a, "name")) : (a = a.replace(/'/g, "\\'").replace(/\\"/g, '"').replace(/(^"|"$)/g, "'"), a = r.stylize(a, "string"));
    }
    return a + ": " + f;
  }
  function to(r, e, t) {
    var n = 0, o = r.reduce(function(i, a) {
      return n++, a.indexOf(`
`) >= 0 && n++, i + a.replace(/\u001b\[\d\d?m/g, "").length + 1;
    }, 0);
    return o > 60 ? t[0] + (e === "" ? "" : e + `
 `) + " " + r.join(`,
  `) + " " + t[1] : t[0] + e + " " + r.join(", ") + " " + t[1];
  }
  y.types = rt();
  function at(r) {
    return Array.isArray(r);
  }
  y.isArray = at;
  function _r(r) {
    return typeof r == "boolean";
  }
  y.isBoolean = _r;
  function pr(r) {
    return r === null;
  }
  y.isNull = pr;
  function no(r) {
    return r == null;
  }
  y.isNullOrUndefined = no;
  function ft(r) {
    return typeof r == "number";
  }
  y.isNumber = ft;
  function lr(r) {
    return typeof r == "string";
  }
  y.isString = lr;
  function oo(r) {
    return typeof r == "symbol";
  }
  y.isSymbol = oo;
  function w(r) {
    return r === undefined;
  }
  y.isUndefined = w;
  function G(r) {
    return M(r) && zr(r) === "[object RegExp]";
  }
  y.isRegExp = G;
  y.types.isRegExp = G;
  function M(r) {
    return typeof r == "object" && r !== null;
  }
  y.isObject = M;
  function cr(r) {
    return M(r) && zr(r) === "[object Date]";
  }
  y.isDate = cr;
  y.types.isDate = cr;
  function W(r) {
    return M(r) && (zr(r) === "[object Error]" || r instanceof Error);
  }
  y.isError = W;
  y.types.isNativeError = W;
  function yr(r) {
    return typeof r == "function";
  }
  y.isFunction = yr;
  function io(r) {
    return r === null || typeof r == "boolean" || typeof r == "number" || typeof r == "string" || typeof r == "symbol" || typeof r > "u";
  }
  y.isPrimitive = io;
  y.isBuffer = tt();
  function zr(r) {
    return Object.prototype.toString.call(r);
  }
  function Gr(r) {
    return r < 10 ? "0" + r.toString(10) : r.toString(10);
  }
  var ao = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function fo() {
    var r = new Date, e = [Gr(r.getHours()), Gr(r.getMinutes()), Gr(r.getSeconds())].join(":");
    return [r.getDate(), ao[r.getMonth()], e].join(" ");
  }
  y.log = function() {
    console.log("%s - %s", fo(), y.format.apply(y, arguments));
  };
  y.inherits = nt();
  y._extend = function(r, e) {
    if (!e || !M(e))
      return r;
    for (var t = Object.keys(e), n = t.length;n--; )
      r[t[n]] = e[t[n]];
    return r;
  };
  function ut(r, e) {
    return Object.prototype.hasOwnProperty.call(r, e);
  }
  var P = typeof Symbol < "u" ? Symbol("util.promisify.custom") : undefined;
  y.promisify = function(e) {
    if (typeof e != "function")
      throw new TypeError('The "original" argument must be of type Function');
    if (P && e[P]) {
      var t = e[P];
      if (typeof t != "function")
        throw new TypeError('The "util.promisify.custom" argument must be of type Function');
      return Object.defineProperty(t, P, { value: t, enumerable: false, writable: false, configurable: true }), t;
    }
    function t() {
      for (var n, o, i = new Promise(function(c, l) {
        n = c, o = l;
      }), a = [], f = 0;f < arguments.length; f++)
        a.push(arguments[f]);
      a.push(function(c, l) {
        c ? o(c) : n(l);
      });
      try {
        e.apply(this, a);
      } catch (c) {
        o(c);
      }
      return i;
    }
    return Object.setPrototypeOf(t, Object.getPrototypeOf(e)), P && Object.defineProperty(t, P, { value: t, enumerable: false, writable: false, configurable: true }), Object.defineProperties(t, ot(e));
  };
  y.promisify.custom = P;
  function uo(r, e) {
    if (!r) {
      var t = new Error("Promise was rejected with a falsy value");
      t.reason = r, r = t;
    }
    return e(r);
  }
  function yo(r) {
    if (typeof r != "function")
      throw new TypeError('The "original" argument must be of type Function');
    function e() {
      for (var t = [], n = 0;n < arguments.length; n++)
        t.push(arguments[n]);
      var o = t.pop();
      if (typeof o != "function")
        throw new TypeError("The last argument must be of type Function");
      var i = this, a = function() {
        return o.apply(i, arguments);
      };
      r.apply(this, t).then(function(f) {
        process.nextTick(a.bind(null, null, f));
      }, function(f) {
        process.nextTick(uo.bind(null, f, a));
      });
    }
    return Object.setPrototypeOf(e, Object.getPrototypeOf(r)), Object.defineProperties(e, ot(r)), e;
  }
  y.callbackify = yo;
});
var E = {};
At(E, { TextDecoder: () => ct, TextEncoder: () => st, default: () => so });
F(E, mt(yt()));
var st = globalThis.TextEncoder;
var ct = globalThis.TextDecoder;
var so = { TextEncoder: st, TextDecoder: ct };
export {
  so as default,
  st as TextEncoder,
  ct as TextDecoder
};
