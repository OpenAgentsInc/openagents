import {
  R,
  T,
  U,
  cr,
  h,
  pr,
  u,
  x
} from "./chat-client-8crq2eqy.js";
import"./chat-client-13a4mv5g.js";

// ../../node_modules/.pnpm/@electric-sql+pglite@0.3.3/node_modules/@electric-sql/pglite/dist/fs/opfs-ahp.js
u();
var $ = "state.txt";
var G = "data";
var T2 = { DIR: 16384, FILE: 32768 };
var H;
var v;
var F;
var M;
var y;
var b;
var m;
var x2;
var P;
var D;
var S;
var n;
var C;
var O;
var k;
var w;
var f;
var I;
var W;
var j;
var L = class extends cr {
  constructor(e, { initialPoolSize: t = 1000, maintainedPoolSize: o = 100, debug: i = false } = {}) {
    super(e, { debug: i });
    R(this, n);
    R(this, H);
    R(this, v);
    R(this, F);
    R(this, M);
    R(this, y);
    R(this, b, new Map);
    R(this, m, new Map);
    R(this, x2, 0);
    R(this, P, new Map);
    R(this, D, new Map);
    this.lastCheckpoint = 0;
    this.checkpointInterval = 1000 * 60;
    this.poolCounter = 0;
    R(this, S, new Set);
    this.initialPoolSize = t, this.maintainedPoolSize = o;
  }
  async init(e, t) {
    return await T(this, n, C).call(this), super.init(e, t);
  }
  async syncToFs(e = false) {
    await this.maybeCheckpointState(), await this.maintainPool(), e || this.flush();
  }
  async closeFs() {
    for (let e of h(this, m).values())
      e.close();
    h(this, y).flush(), h(this, y).close(), this.pg.Module.FS.quit();
  }
  async maintainPool(e) {
    e = e || this.maintainedPoolSize;
    let t = e - this.state.pool.length, o = [];
    for (let i = 0;i < t; i++)
      o.push(new Promise(async (c) => {
        ++this.poolCounter;
        let a = `${(Date.now() - 1704063600).toString(16).padStart(8, "0")}-${this.poolCounter.toString(16).padStart(8, "0")}`, h2 = await h(this, F).getFileHandle(a, { create: true }), d = await h2.createSyncAccessHandle();
        h(this, b).set(a, h2), h(this, m).set(a, d), T(this, n, k).call(this, { opp: "createPoolFile", args: [a] }), this.state.pool.push(a), c();
      }));
    for (let i = 0;i > t; i--)
      o.push(new Promise(async (c) => {
        let a = this.state.pool.pop();
        T(this, n, k).call(this, { opp: "deletePoolFile", args: [a] });
        let h2 = h(this, b).get(a);
        h(this, m).get(a)?.close(), await h(this, F).removeEntry(h2.name), h(this, b).delete(a), h(this, m).delete(a), c();
      }));
    await Promise.all(o);
  }
  _createPoolFileState(e) {
    this.state.pool.push(e);
  }
  _deletePoolFileState(e) {
    let t = this.state.pool.indexOf(e);
    t > -1 && this.state.pool.splice(t, 1);
  }
  async maybeCheckpointState() {
    Date.now() - this.lastCheckpoint > this.checkpointInterval && await this.checkpointState();
  }
  async checkpointState() {
    let e = new TextEncoder().encode(JSON.stringify(this.state));
    h(this, y).truncate(0), h(this, y).write(e, { at: 0 }), h(this, y).flush(), this.lastCheckpoint = Date.now();
  }
  flush() {
    for (let e of h(this, S))
      try {
        e.flush();
      } catch {
      }
    h(this, S).clear();
  }
  chmod(e, t) {
    T(this, n, O).call(this, { opp: "chmod", args: [e, t] }, () => {
      this._chmodState(e, t);
    });
  }
  _chmodState(e, t) {
    let o = T(this, n, f).call(this, e);
    o.mode = t;
  }
  close(e) {
    let t = T(this, n, I).call(this, e);
    h(this, P).delete(e), h(this, D).delete(t);
  }
  fstat(e) {
    let t = T(this, n, I).call(this, e);
    return this.lstat(t);
  }
  lstat(e) {
    let t = T(this, n, f).call(this, e), o = t.type === "file" ? h(this, m).get(t.backingFilename).getSize() : 0, i = 4096;
    return { dev: 0, ino: 0, mode: t.mode, nlink: 1, uid: 0, gid: 0, rdev: 0, size: o, blksize: i, blocks: Math.ceil(o / i), atime: t.lastModified, mtime: t.lastModified, ctime: t.lastModified };
  }
  mkdir(e, t) {
    T(this, n, O).call(this, { opp: "mkdir", args: [e, t] }, () => {
      this._mkdirState(e, t);
    });
  }
  _mkdirState(e, t) {
    let o = T(this, n, w).call(this, e), i = o.pop(), c = [], a = this.state.root;
    for (let d of o) {
      if (c.push(e), !Object.prototype.hasOwnProperty.call(a.children, d))
        if (t?.recursive)
          this.mkdir(c.join("/"));
        else
          throw new p("ENOENT", "No such file or directory");
      if (a.children[d].type !== "directory")
        throw new p("ENOTDIR", "Not a directory");
      a = a.children[d];
    }
    if (Object.prototype.hasOwnProperty.call(a.children, i))
      throw new p("EEXIST", "File exists");
    let h2 = { type: "directory", lastModified: Date.now(), mode: t?.mode || T2.DIR, children: {} };
    a.children[i] = h2;
  }
  open(e, t, o) {
    if (T(this, n, f).call(this, e).type !== "file")
      throw new p("EISDIR", "Is a directory");
    let c = T(this, n, W).call(this);
    return h(this, P).set(c, e), h(this, D).set(e, c), c;
  }
  readdir(e) {
    let t = T(this, n, f).call(this, e);
    if (t.type !== "directory")
      throw new p("ENOTDIR", "Not a directory");
    return Object.keys(t.children);
  }
  read(e, t, o, i, c) {
    let a = T(this, n, I).call(this, e), h2 = T(this, n, f).call(this, a);
    if (h2.type !== "file")
      throw new p("EISDIR", "Is a directory");
    return h(this, m).get(h2.backingFilename).read(new Uint8Array(t.buffer, o, i), { at: c });
  }
  rename(e, t) {
    T(this, n, O).call(this, { opp: "rename", args: [e, t] }, () => {
      this._renameState(e, t, true);
    });
  }
  _renameState(e, t, o = false) {
    let i = T(this, n, w).call(this, e), c = i.pop(), a = T(this, n, f).call(this, i.join("/"));
    if (!Object.prototype.hasOwnProperty.call(a.children, c))
      throw new p("ENOENT", "No such file or directory");
    let h2 = T(this, n, w).call(this, t), d = h2.pop(), l = T(this, n, f).call(this, h2.join("/"));
    if (o && Object.prototype.hasOwnProperty.call(l.children, d)) {
      let u2 = l.children[d];
      h(this, m).get(u2.backingFilename).truncate(0), this.state.pool.push(u2.backingFilename);
    }
    l.children[d] = a.children[c], delete a.children[c];
  }
  rmdir(e) {
    T(this, n, O).call(this, { opp: "rmdir", args: [e] }, () => {
      this._rmdirState(e);
    });
  }
  _rmdirState(e) {
    let t = T(this, n, w).call(this, e), o = t.pop(), i = T(this, n, f).call(this, t.join("/"));
    if (!Object.prototype.hasOwnProperty.call(i.children, o))
      throw new p("ENOENT", "No such file or directory");
    let c = i.children[o];
    if (c.type !== "directory")
      throw new p("ENOTDIR", "Not a directory");
    if (Object.keys(c.children).length > 0)
      throw new p("ENOTEMPTY", "Directory not empty");
    delete i.children[o];
  }
  truncate(e, t = 0) {
    let o = T(this, n, f).call(this, e);
    if (o.type !== "file")
      throw new p("EISDIR", "Is a directory");
    let i = h(this, m).get(o.backingFilename);
    if (!i)
      throw new p("ENOENT", "No such file or directory");
    i.truncate(t), h(this, S).add(i);
  }
  unlink(e) {
    T(this, n, O).call(this, { opp: "unlink", args: [e] }, () => {
      this._unlinkState(e, true);
    });
  }
  _unlinkState(e, t = false) {
    let o = T(this, n, w).call(this, e), i = o.pop(), c = T(this, n, f).call(this, o.join("/"));
    if (!Object.prototype.hasOwnProperty.call(c.children, i))
      throw new p("ENOENT", "No such file or directory");
    let a = c.children[i];
    if (a.type !== "file")
      throw new p("EISDIR", "Is a directory");
    if (delete c.children[i], t) {
      let h2 = h(this, m).get(a.backingFilename);
      h2?.truncate(0), h(this, S).add(h2), h(this, D).has(e) && (h(this, P).delete(h(this, D).get(e)), h(this, D).delete(e));
    }
    this.state.pool.push(a.backingFilename);
  }
  utimes(e, t, o) {
    T(this, n, O).call(this, { opp: "utimes", args: [e, t, o] }, () => {
      this._utimesState(e, t, o);
    });
  }
  _utimesState(e, t, o) {
    let i = T(this, n, f).call(this, e);
    i.lastModified = o;
  }
  writeFile(e, t, o) {
    let i = T(this, n, w).call(this, e), c = i.pop(), a = T(this, n, f).call(this, i.join("/"));
    if (Object.prototype.hasOwnProperty.call(a.children, c)) {
      let l = a.children[c];
      l.lastModified = Date.now(), T(this, n, k).call(this, { opp: "setLastModified", args: [e, l.lastModified] });
    } else {
      if (this.state.pool.length === 0)
        throw new Error("No more file handles available in the pool");
      let l = { type: "file", lastModified: Date.now(), mode: o?.mode || T2.FILE, backingFilename: this.state.pool.pop() };
      a.children[c] = l, T(this, n, k).call(this, { opp: "createFileNode", args: [e, l] });
    }
    let h2 = a.children[c], d = h(this, m).get(h2.backingFilename);
    t.length > 0 && (d.write(typeof t == "string" ? new TextEncoder().encode(t) : new Uint8Array(t), { at: 0 }), e.startsWith("/pg_wal") && h(this, S).add(d));
  }
  _createFileNodeState(e, t) {
    let o = T(this, n, w).call(this, e), i = o.pop(), c = T(this, n, f).call(this, o.join("/"));
    c.children[i] = t;
    let a = this.state.pool.indexOf(t.backingFilename);
    return a > -1 && this.state.pool.splice(a, 1), t;
  }
  _setLastModifiedState(e, t) {
    let o = T(this, n, f).call(this, e);
    o.lastModified = t;
  }
  write(e, t, o, i, c) {
    let a = T(this, n, I).call(this, e), h2 = T(this, n, f).call(this, a);
    if (h2.type !== "file")
      throw new p("EISDIR", "Is a directory");
    let d = h(this, m).get(h2.backingFilename);
    if (!d)
      throw new p("EBADF", "Bad file descriptor");
    let l = d.write(new Uint8Array(t, o, i), { at: c });
    return a.startsWith("/pg_wal") && h(this, S).add(d), l;
  }
};
H = new WeakMap, v = new WeakMap, F = new WeakMap, M = new WeakMap, y = new WeakMap, b = new WeakMap, m = new WeakMap, x2 = new WeakMap, P = new WeakMap, D = new WeakMap, S = new WeakMap, n = new WeakSet, C = async function() {
  x(this, H, await navigator.storage.getDirectory()), x(this, v, await T(this, n, j).call(this, this.dataDir, { create: true })), x(this, F, await T(this, n, j).call(this, G, { from: h(this, v), create: true })), x(this, M, await h(this, v).getFileHandle($, { create: true })), x(this, y, await h(this, M).createSyncAccessHandle());
  let e = new ArrayBuffer(h(this, y).getSize());
  h(this, y).read(e, { at: 0 });
  let t, o = new TextDecoder().decode(e).split(`
`), i = false;
  try {
    t = JSON.parse(o[0]);
  } catch {
    t = { root: { type: "directory", lastModified: Date.now(), mode: T2.DIR, children: {} }, pool: [] }, h(this, y).truncate(0), h(this, y).write(new TextEncoder().encode(JSON.stringify(t)), { at: 0 }), i = true;
  }
  this.state = t;
  let c = o.slice(1).filter(Boolean).map((l) => JSON.parse(l));
  for (let l of c) {
    let u2 = `_${l.opp}State`;
    if (typeof this[u2] == "function")
      try {
        this[u2].bind(this)(...l.args);
      } catch (N) {
        console.warn("Error applying OPFS AHP WAL entry", l, N);
      }
  }
  let a = [], h2 = async (l) => {
    if (l.type === "file")
      try {
        let u2 = await h(this, F).getFileHandle(l.backingFilename), N = await u2.createSyncAccessHandle();
        h(this, b).set(l.backingFilename, u2), h(this, m).set(l.backingFilename, N);
      } catch (u2) {
        console.error("Error opening file handle for node", l, u2);
      }
    else
      for (let u2 of Object.values(l.children))
        a.push(h2(u2));
  };
  await h2(this.state.root);
  let d = [];
  for (let l of this.state.pool)
    d.push(new Promise(async (u2) => {
      h(this, b).has(l) && console.warn("File handle already exists for pool file", l);
      let N = await h(this, F).getFileHandle(l), U2 = await N.createSyncAccessHandle();
      h(this, b).set(l, N), h(this, m).set(l, U2), u2();
    }));
  await Promise.all([...a, ...d]), await this.maintainPool(i ? this.initialPoolSize : this.maintainedPoolSize);
}, O = function(e, t) {
  let o = T(this, n, k).call(this, e);
  try {
    t();
  } catch (i) {
    throw h(this, y).truncate(o), i;
  }
}, k = function(e) {
  let t = JSON.stringify(e), o = new TextEncoder().encode(`
${t}`), i = h(this, y).getSize();
  return h(this, y).write(o, { at: i }), h(this, S).add(h(this, y)), i;
}, w = function(e) {
  return e.split("/").filter(Boolean);
}, f = function(e, t) {
  let o = T(this, n, w).call(this, e), i = t || this.state.root;
  for (let c of o) {
    if (i.type !== "directory")
      throw new p("ENOTDIR", "Not a directory");
    if (!Object.prototype.hasOwnProperty.call(i.children, c))
      throw new p("ENOENT", "No such file or directory");
    i = i.children[c];
  }
  return i;
}, I = function(e) {
  let t = h(this, P).get(e);
  if (!t)
    throw new p("EBADF", "Bad file descriptor");
  return t;
}, W = function() {
  let e = ++U(this, x2)._;
  for (;h(this, P).has(e); )
    U(this, x2)._++;
  return e;
}, j = async function(e, t) {
  let o = T(this, n, w).call(this, e), i = t?.from || h(this, H);
  for (let c of o)
    i = await i.getDirectoryHandle(c, { create: t?.create });
  return i;
};
var p = class extends Error {
  constructor(A, e) {
    super(e), typeof A == "number" ? this.code = A : typeof A == "string" && (this.code = pr[A]);
  }
};
export {
  L as OpfsAhpFS
};
