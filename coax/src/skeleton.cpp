// The changed procedure bodies, checked first (#13142). Included by main.cpp
// before front_end, which is its only caller.
//
// A resident compiler (serve.cpp) remembers the last compiles of an entry
// unit whose type check passed, as a skeleton: every top-level item's tokens
// outside a procedure body, and each body's own tokens, as bytes (kind,
// spelling and value: ReuseIndex::put). They are compared whole, never by a
// hash. The next compile
// of that entry differs from one of them, most often, only inside the bodies
// of a few procedures. Then every other declaration is known to pass again:
// the checker reads a procedure's body only while it checks that procedure
// (CK_run's body pass, statements.coax), and everything a body's check reads
// of the rest of the program (types, constants, signatures, templates, module
// scopes) is outside the bodies and unchanged.
//
// So while the compile goes on to parse the whole program, a forked child
// parses it with the unchanged bodies emptied and runs the ordinary checker
// over that. When that check reports errors inside the changed bodies, they
// are the errors the whole program's check reports, in its order, and the
// compile reports them and stops without checking the rest: a type error in
// an edited file is reported in the time it takes to parse.
//
// It is the whole check's answer, or it is not used. The child's answer is
// taken only when
//   - the remembered compile had the same options, units and module
//     structure, and every token outside the changed bodies is the same;
//   - each changed body is the body of a procedure without type parameters,
//     or of a #test or #forall check, in the remembered parse and in the
//     child's;
//   - the child's checker stopped on no storage limit, and each error it
//     reports is inside a changed body (an emptied body that returns a value
//     reports E0338 at its procedure; those are dropped, and nothing else may
//     be outside). With an error in a body, the checker expands no generic
//     instance and runs no later pass, in the child or in the whole check;
//   - what the remembered check added to each store, plus what the child
//     added, is under the near-limit fraction of the child's own (smaller)
//     limits, so the whole check stops on no limit and prints no note the
//     child's does not explain.
// Otherwise the compile checks the whole program as before. COAX_SKELETON=0
// turns the child off; COAX_SKELETON_VERIFY=1 also runs the whole check and
// stops with status 70 if its errors are not the child's (the tests set it);
// COAX_SKELETON_DEBUG=1 says on stderr what the child answered and when.
namespace coax {

struct PassedSkeleton {
  struct Item {
    uint8_t candidate = 0;  // an identifier, #test or #forall, then tokens that end in a brace group
    uint8_t elidable = 0;   // that group is the body of a procedure without type parameters, or of a check
    int32_t unit = -1;
    uint64_t outside = 0, outside_size = 0;  // in bytes: its tokens outside the group (all of them when no candidate)
    uint64_t body = 0, body_size = 0;        // in bytes: the tokens inside the group
  };
  std::string options;      // what changes the parse or the check
  std::string units;        // the units' paths, modules, loads, aliases and exports
  std::vector<Item> items;
  std::string bytes;
  bool settled = false;     // no limit was near when its check passed
  CheckUsage used;
};

bool passed_same_options(const PassedSkeleton &a, const PassedSkeleton &b) { return a.options == b.options; }
std::string passed_record(const std::string &entry, const PassedSkeleton &p) {
  std::string out;
  out.reserve(p.bytes.size() + p.items.size() * sizeof(PassedSkeleton::Item) + p.units.size() + 512);
  auto u64 = [&](uint64_t v) { out.append((const char *)&v, sizeof v); };
  auto str = [&](const std::string &s) { u64(s.size()); out += s; };
  str(entry);
  str(p.options);
  str(p.units);
  u64(p.settled);
  for (uint64_t v : {p.used.nodes, p.used.input, p.used.instances, p.used.instance_limit, p.used.types, p.used.type_limit, p.used.bytes, p.used.byte_limit}) u64(v);
  u64(p.items.size());
  for (const PassedSkeleton::Item &i : p.items) { u64(i.candidate); u64(i.elidable); u64((uint64_t)(int64_t)i.unit); u64(i.outside); u64(i.outside_size); u64(i.body); u64(i.body_size); }
  str(p.bytes);
  return out;
}
std::shared_ptr<const PassedSkeleton> passed_parse(const std::string &record, std::string *entry) {
  const char *at = record.data(), *end = at + record.size();
  bool ok = true;
  auto u64 = [&]() { uint64_t v = 0; if ((size_t)(end - at) < sizeof v) { ok = false; return v; } memcpy(&v, at, sizeof v); at += sizeof v; return v; };
  auto str = [&]() { const uint64_t n = u64(); if (!ok || (uint64_t)(end - at) < n) { ok = false; return std::string(); } std::string s(at, (size_t)n); at += n; return s; };
  auto p = std::make_shared<PassedSkeleton>();
  *entry = str();
  p->options = str();
  p->units = str();
  p->settled = u64() != 0;
  uint64_t *fields[] = {&p->used.nodes, &p->used.input, &p->used.instances, &p->used.instance_limit, &p->used.types, &p->used.type_limit, &p->used.bytes, &p->used.byte_limit};
  for (uint64_t *f : fields) *f = u64();
  const uint64_t count = u64();
  if (!ok || count > (uint64_t)(end - at) / (7 * sizeof(uint64_t))) return nullptr;
  p->items.resize((size_t)count);
  for (PassedSkeleton::Item &i : p->items) { i.candidate = u64() != 0; i.elidable = u64() != 0; i.unit = (int32_t)(int64_t)u64(); i.outside = u64(); i.outside_size = u64(); i.body = u64(); i.body_size = u64(); }
  p->bytes = str();
  if (!ok || at != end) return nullptr;
  // Every item's bytes are inside the record's.
  for (const PassedSkeleton::Item &i : p->items)
    if (i.outside > p->bytes.size() || i.outside_size > p->bytes.size() - i.outside || i.body > p->bytes.size() || i.body_size > p->bytes.size() - i.body) return nullptr;
  return p;
}

namespace {

// This compile's items, where each is in the tokens, and its skeleton.
struct SkeletonNow {
  struct Range { int32_t first = 0, last = -1, open = -1, close = -1, unit = -1; };  // tokens [first, last]; the group is [open, close]
  std::vector<Range> ranges;
  PassedSkeleton skeleton;
  bool made = false;
  // The item whose tokens span offset, or -1.
  int32_t item_at(const std::vector<Token> &tk, int32_t offset) const {
    size_t lo = 0, hi = ranges.size();
    while (lo < hi) { const size_t mid = (lo + hi) / 2; if (tk[(size_t)ranges[mid].first].loc.offset <= offset) lo = mid + 1; else hi = mid; }
    if (lo == 0) return -1;
    const Token &last = tk[(size_t)ranges[lo - 1].last];
    return offset < last.loc.offset + std::max(last.text.count, 1) ? (int32_t)(lo - 1) : -1;
  }
};

bool skeleton_off() {
  const char *env = getenv("COAX_SKELETON");
  return env && (!strcmp(env, "0") || !strcmp(env, "off"));
}

// Items as the check cache splits them (ReuseIndex::build, verify.cpp): a run
// of a unit's tokens that ends at a depth-0 `;`, or at the `}` that returns
// to depth 0 (with a `;` straight after it), or where the unit ends.
bool skeleton_make(const Source *src, const std::vector<Token> &tk, const std::string &options, SkeletonNow *out) {
  out->made = false;
  if (src->units.empty() || tk.empty()) return false;
  std::vector<std::pair<int32_t, int32_t>> starts;
  for (size_t u = 0; u < src->units.size(); u++) starts.push_back({src->units[u]->begin, (int32_t)u});
  std::sort(starts.begin(), starts.end());
  auto unit_of = [&](int32_t offset) -> int32_t {
    auto at = std::upper_bound(starts.begin(), starts.end(), std::make_pair(offset, INT32_MAX));
    if (at == starts.begin()) return -1;
    const int32_t u = (at - 1)->second;
    return offset < src->units[(size_t)u]->end ? u : -1;
  };
  int32_t last_offset = -1;
  for (size_t i = 0; i < tk.size();) {
    if (tk[i].kind == Tok::eof) { i++; continue; }
    SkeletonNow::Range r;
    r.first = (int32_t)i;
    r.unit = unit_of(tk[i].loc.offset);
    int depth = 0;
    size_t j = i;
    for (; j < tk.size(); j++) {
      const Token &t = tk[j];
      if (t.kind == Tok::eof || (j > i && unit_of(t.loc.offset) != r.unit)) break;
      if (t.loc.offset <= last_offset) return false;
      last_offset = t.loc.offset;
      if (t.kind == Tok::lparen || t.kind == Tok::lbracket || t.kind == Tok::lbrace) depth++;
      else if (t.kind == Tok::rparen || t.kind == Tok::rbracket || t.kind == Tok::rbrace) depth--;
      if (depth < 0) return false;
      if (depth == 0 && t.kind == Tok::semicolon) { j++; break; }
      if (depth == 0 && t.kind == Tok::rbrace) {
        j++;
        if (j < tk.size() && tk[j].kind == Tok::semicolon && unit_of(tk[j].loc.offset) == r.unit) { last_offset = tk[j].loc.offset; j++; }
        break;
      }
    }
    if (depth != 0) return false;
    r.last = (int32_t)j - 1;
    // The brace group that ends an item an identifier begins.
    int32_t close = r.last;
    if (tk[(size_t)close].kind == Tok::semicolon) close--;
    const Token &head = tk[(size_t)r.first];
    const bool begins = head.kind == Tok::ident || (head.kind == Tok::directive && (str_eq(head.text, "test") || str_eq(head.text, "forall")));
    if (begins && close > r.first && tk[(size_t)close].kind == Tok::rbrace) {
      int inner = 0;
      for (int32_t t = close; t > r.first; t--) {
        const Tok kind = tk[(size_t)t].kind;
        if (kind == Tok::rparen || kind == Tok::rbracket || kind == Tok::rbrace) inner++;
        else if (kind == Tok::lparen || kind == Tok::lbracket || kind == Tok::lbrace) inner--;
        if (inner == 0) { if (kind == Tok::lbrace) { r.open = t; r.close = close; } break; }
      }
    }
    out->ranges.push_back(r);
    i = j;
  }
  PassedSkeleton &s = out->skeleton;
  s.options = options;
  std::string text = "coax.skeleton-units.v1\n";
  for (size_t u = 0; u < src->units.size(); u++) {
    const SourceUnit &unit = *src->units[u];
    // (Whether a unit's checks are left out as a library's is in its items: they are there or not.)
    text += tree_path(unit.canonical) + "\x1f" + unit.module + "\x1f" + (unit.claims ? "c" : "-") + (unit.authority ? "a" : "-");
    for (int32_t load : unit.loads) text += "\x1fload " + std::to_string(load);
    for (const auto &a : unit.aliases) text += "\x1f" "alias " + a.name + " " + std::to_string(a.target);
    for (const auto &e : unit.exports) text += "\x1f" "export " + e.name;
    text += "\n";
  }
  s.units = std::move(text);
  // Each item's bytes are its own: the items are written on threads (burst,
  // main.cpp), a run of them to a piece, and the pieces joined in order.
  const size_t count = out->ranges.size();
  s.items.resize(count);
  const size_t run = 256;
  std::vector<std::string> pieces((count + run - 1) / run);
  burst(pieces.size(), 1, [&](size_t begin, size_t end, unsigned) {
    for (size_t piece = begin; piece < end; piece++) {
      std::string &bytes = pieces[piece];
      const size_t first = piece * run, last = std::min(count, first + run);
      bytes.reserve((size_t)(out->ranges[last - 1].last - out->ranges[first].first + 1) * 14);
      for (size_t k = first; k < last; k++) {
        const SkeletonNow::Range &r = out->ranges[k];
        PassedSkeleton::Item &item = s.items[k];
        item.candidate = r.open >= 0;
        item.unit = r.unit;
        item.outside = bytes.size();  // within its piece, until the pieces are joined
        for (int32_t t = r.first; t <= (r.open >= 0 ? r.open : r.last); t++) ReuseIndex::put(bytes, tk[(size_t)t]);
        for (int32_t t = r.close; r.open >= 0 && t <= r.last; t++) ReuseIndex::put(bytes, tk[(size_t)t]);
        item.outside_size = bytes.size() - item.outside;
        item.body = bytes.size();
        for (int32_t t = r.open + 1; r.open >= 0 && t < r.close; t++) ReuseIndex::put(bytes, tk[(size_t)t]);
        item.body_size = bytes.size() - item.body;
      }
    }
  });
  size_t total = 0;
  for (const std::string &piece : pieces) total += piece.size();
  s.bytes.reserve(total);
  for (size_t piece = 0; piece < pieces.size(); piece++) {
    for (size_t k = piece * run; k < std::min(count, (piece + 1) * run); k++) { s.items[k].outside += s.bytes.size(); s.items[k].body += s.bytes.size(); }
    s.bytes += pieces[piece];
  }
  out->made = true;
  return true;
}

// Whether item k's brace group is exactly the body of the one declaration in
// it: a procedure without type parameters that begins the item with its
// name, or a #test or #forall check.
// by_item: the procedures and the checks' procedures of the parse, by the item they begin.
bool skeleton_body(const SkeletonNow &now, const std::vector<Token> &tk, const std::vector<const ProcDecl *> &by_item, const std::vector<uint8_t> &claims, int32_t k) {
  const SkeletonNow::Range &r = now.ranges[(size_t)k];
  const ProcDecl *p = by_item[(size_t)k];
  if (!p || claims[(size_t)k] != 1 || r.open < 0 || p->generic || p->foreign || p->origin || !p->body) return false;
  const Token &head = tk[(size_t)r.first];
  if (head.kind == Tok::ident && (p->loc.offset != head.loc.offset || !str_eq(p->name, head.text))) return false;
  return p->body->kind == StmtKind::block && p->body->loc.offset == tk[(size_t)r.open].loc.offset;
}
// Every declaration of the parse, counted by the item it starts in; and the procedures.
void skeleton_claims(const SkeletonNow &now, const std::vector<Token> &tk, const File *file, std::vector<const ProcDecl *> *by_item, std::vector<uint8_t> *claims) {
  by_item->assign(now.ranges.size(), nullptr);
  claims->assign(now.ranges.size(), 0);
  auto claim = [&](Loc loc) { const int32_t k = now.item_at(tk, loc.offset); if (k >= 0 && (*claims)[(size_t)k] < 2) (*claims)[(size_t)k]++; return k; };
  for (const StructDecl *d : file->structs) if (!d->origin) claim(d->loc);
  for (const StructDecl *d : file->generic_structs) claim(d->loc);
  for (const EnumDecl *d : file->enums) claim(d->loc);
  for (const UnionDecl *d : file->unions) claim(d->loc);
  for (const ConstDecl *d : file->constants) claim(d->loc);
  for (const CheckDecl *c : file->checks) { const int32_t k = claim(c->loc); if (k >= 0 && tk[(size_t)now.ranges[(size_t)k].first].kind == Tok::directive && c->loc.offset == tk[(size_t)now.ranges[(size_t)k].first].loc.offset) (*by_item)[(size_t)k] = c->proc; }
  auto procedure = [&](const ProcDecl *p) { if (p->origin) return; const int32_t k = claim(p->loc); if (k >= 0) (*by_item)[(size_t)k] = p; };
  for (const ProcDecl *p : file->procs) procedure(p);
  for (const ProcDecl *p : file->generics) procedure(p);
}

// After this compile's check passed: which items' groups are bodies, and what the check used.
void skeleton_passed(SkeletonNow *now, const std::vector<Token> &tk, const File *file) {
  std::vector<const ProcDecl *> by_item;
  std::vector<uint8_t> claims;
  skeleton_claims(*now, tk, file, &by_item, &claims);
  for (size_t k = 0; k < now->ranges.size(); k++) now->skeleton.items[k].elidable = skeleton_body(*now, tk, by_item, claims, (int32_t)k);
  now->skeleton.used = g_check_usage;
  now->skeleton.settled = true;
  for (const auto &[name, c] : g_capacity) if (capacity_near(c.used, c.limit)) now->skeleton.settled = false;
}

// The items whose bodies differ from a remembered compile's; false, with why, when anything else does.
bool skeleton_changed(const PassedSkeleton &was, const PassedSkeleton &now, std::vector<int32_t> *changed, std::string *why) {
  changed->clear();
  if (!was.settled) { *why = "a use was near its limit when it passed"; return false; }
  if (was.options != now.options) { *why = "other options"; return false; }
  if (was.units != now.units) { *why = "other units, modules, loads, aliases or exports"; return false; }
  if (was.items.size() != now.items.size()) { *why = "it had " + std::to_string(was.items.size()) + " items and this has " + std::to_string(now.items.size()); return false; }
  for (size_t k = 0; k < now.items.size(); k++) {
    const PassedSkeleton::Item &a = was.items[k], &b = now.items[k];
    if (a.candidate != b.candidate || a.unit != b.unit || a.outside_size != b.outside_size ||
        memcmp(was.bytes.data() + a.outside, now.bytes.data() + b.outside, (size_t)a.outside_size) != 0) { *why = "item " + std::to_string(k) + " differs outside a body"; return false; }
    if (a.body_size == b.body_size && memcmp(was.bytes.data() + a.body, now.bytes.data() + b.body, (size_t)a.body_size) == 0) continue;
    if (!a.elidable) { *why = "item " + std::to_string(k) + " differs, and is no procedure's or check's body"; return false; }
    changed->push_back((int32_t)k);
  }
  return true;
}

// The child and its answer.
struct SkeletonJob {
  pid_t pid = -1;
  int fd = -1;
  bool asked = false, definite = false;
  std::vector<CheckCapture::Error> errors;
  std::string in;  // what the child has written so far
  std::chrono::steady_clock::time_point began;
  ~SkeletonJob() { stop(); }
  void stop() {
    if (fd >= 0) { close(fd); fd = -1; }
    if (pid > 0) { kill(pid, SIGKILL); int status; while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {} pid = -1; }
  }
  // The child's answer, if it has one by now (the compile's own check reports
  // the same errors a little later). True when the child's errors are the
  // whole check's.
  // wait_ms: how long to wait for an answer that is not there yet.
  bool answer(int wait_ms = 0) {
    if (asked) return definite;
    if (pid <= 0) return false;
    const bool debug = getenv("COAX_SKELETON_DEBUG") != nullptr;
    char buffer[1 << 14];
    for (;;) {
      // The child writes its whole answer and leaves: the end of the stream is the answer.
      pollfd p{fd, POLLIN, 0};
      const int ready = poll(&p, 1, wait_ms);
      if (ready < 0 && errno == EINTR) continue;
      if (ready <= 0) { if (debug) fprintf(stderr, "coax skeleton: no answer yet after %.3f s\n", seconds_since(began)); return false; }  // asked again later
      const ssize_t n = read(fd, buffer, sizeof buffer);
      if (n < 0 && errno == EINTR) continue;
      if (n <= 0) break;
      in.append(buffer, (size_t)n);
    }
    asked = true;
    close(fd);
    fd = -1;
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    pid = -1;
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) return false;
    const char *at = in.data(), *end = at + in.size();
    bool ok = true;
    auto u64 = [&]() { uint64_t v = 0; if ((size_t)(end - at) < sizeof v) { ok = false; return v; } memcpy(&v, at, sizeof v); at += sizeof v; return v; };
    auto str = [&]() { const uint64_t n = u64(); if (!ok || (uint64_t)(end - at) < n) { ok = false; return std::string(); } std::string s(at, (size_t)n); at += n; return s; };
    if (u64() != 0x534b454c45544f4eull) return false;
    const std::string why = str();
    const uint64_t count = u64();
    for (uint64_t i = 0; ok && i < count; i++) {
      CheckCapture::Error e;
      e.loc.line = (int32_t)u64(); e.loc.col = (int32_t)u64(); e.loc.offset = (int32_t)u64();
      e.code = str(); e.message = str(); e.fix = str();
      errors.push_back(std::move(e));
    }
    if (!ok || u64() != 0x454e44ull || at != end) { errors.clear(); return false; }
    if (debug) fprintf(stderr, "coax skeleton: %s, read at %.3f s\n", why.c_str(), seconds_since(began));
    return definite = !errors.empty();
  }
};

// In the child. Checks the program with every unchanged body emptied; answers
// with the errors when they are the whole check's, and with none otherwise.
[[noreturn]] void skeleton_child(int out, const Source *src, const std::vector<Token> &tk, const SkeletonNow &now, const PassedSkeleton &was,
                                 const std::vector<int32_t> &changed, const EmitOptions &opts, int library) {
  const auto began = std::chrono::steady_clock::now();
  double parsed = 0;
  // Nothing of this process reaches the caller's streams or the resident.
  const int null = open("/dev/null", O_RDWR);
  if (null >= 0) { dup2(null, 0); dup2(null, 1); dup2(null, 2); }
  // why: what the answer is, for COAX_SKELETON_DEBUG. errors: empty unless they are the whole check's.
  auto leave = [&](std::string why, const std::vector<CheckCapture::Error> &errors) {
    std::string answer;
    auto u64 = [&](uint64_t v) { answer.append((const char *)&v, sizeof v); };
    auto str = [&](const std::string &s) { u64(s.size()); answer += s; };
    char took[96];
    snprintf(took, sizeof took, " (the child took %.3f s, its parse %.3f s)", seconds_since(began), parsed);
    u64(0x534b454c45544f4eull);
    str(why + took);
    u64(errors.size());
    for (const CheckCapture::Error &e : errors) {
      u64((uint64_t)e.loc.line); u64((uint64_t)e.loc.col); u64((uint64_t)e.loc.offset);
      str(e.code); str(e.message); str(e.fix);
    }
    u64(0x454e44ull);
    size_t sent = 0;
    while (sent < answer.size()) { const ssize_t n = write(out, answer.data() + sent, answer.size() - sent); if (n < 0 && errno == EINTR) continue; if (n <= 0) break; sent += (size_t)n; }
    _exit(0);
  };
  const std::vector<CheckCapture::Error> none;
  std::vector<uint8_t> is_changed(now.ranges.size(), 0);
  for (int32_t k : changed) is_changed[(size_t)k] = 1;
  std::vector<Token> kept;
  kept.reserve(tk.size() / 4);
  for (size_t k = 0, t = 0; t < tk.size(); t++) {
    while (k < now.ranges.size() && now.ranges[k].last < (int32_t)t) k++;
    if (k < now.ranges.size() && was.items[k].elidable && !is_changed[k] && (int32_t)t > now.ranges[k].open && (int32_t)t < now.ranges[k].close) continue;
    kept.push_back(tk[t]);
  }
  Arena arena;
  File file;
  if (!parse(&arena, src, kept, &file, opts.development)) leave("the emptied program does not parse", none);
  parsed = seconds_since(began);
  check_options(&file, opts, library);
  // The parse agrees with the remembered one about what is a body.
  std::vector<const ProcDecl *> by_item;
  std::vector<uint8_t> claims;
  skeleton_claims(now, tk, &file, &by_item, &claims);
  for (size_t k = 0; k < now.ranges.size(); k++)
    if (was.items[k].elidable && !skeleton_body(now, tk, by_item, claims, (int32_t)k)) leave("an item's brace group is no longer a body", none);
  CheckCapture capture;
  g_check_capture = &capture;
  const bool passed = check(&arena, &file, false);
  if (passed) leave("the changed bodies pass", none);
  if (capture.refused) leave("the check stopped on storage", none);
  for (const auto &[name, c] : g_capacity) if (capacity_near(c.used, c.limit)) leave("a use is near its limit: " + name, none);
  // The whole check adds at most what the remembered one added and what this
  // one added, to stores no smaller than this one's.
  const CheckUsage &u = g_check_usage, &w = was.used;
  // (The node store holds three times its input's nodes: with the near-limit
  // fraction at a half or more, room left grows with the input.)
  if (capacity_near_percent() < 50 || capacity_near(u.input + u.nodes + w.nodes, u.input * 3) || capacity_near(u.instances + w.instances, u.instance_limit) ||
      capacity_near(u.types + w.types, u.type_limit) || capacity_near(u.bytes + w.bytes, u.byte_limit)) leave("the whole check may come near a limit", none);
  std::vector<CheckCapture::Error> errors;
  size_t emptied = 0;
  for (const CheckCapture::Error &e : capture.errors) {
    if (e.code == "E0339" || e.code == "E0381") leave("a storage error", none);
    // Coverage reads the bodies of every check, and the child emptied them.
    if (e.code == "E0380") leave("a coverage error: it reads other checks' bodies", none);
    const int32_t k = now.item_at(tk, e.loc.offset);
    if (k < 0) leave("an error outside every item: " + e.code, none);
    // An emptied body that must return a value: not the whole program's error.
    if (!is_changed[(size_t)k]) {
      if (e.code == "E0338" && was.items[(size_t)k].elidable && e.loc.offset == tk[(size_t)now.ranges[(size_t)k].first].loc.offset) { emptied++; continue; }
      leave("an error outside the changed bodies: " + e.code + " at offset " + std::to_string(e.loc.offset), none);
    }
    errors.push_back(e);
  }
  if (errors.empty()) leave("the changed bodies pass", none);
  leave(std::to_string(errors.size()) + " error(s) in " + std::to_string(changed.size()) + " changed bodies, " + std::to_string(emptied) + " emptied bodies must return, " + std::to_string(kept.size()) + " of " + std::to_string(tk.size()) + " tokens checked", errors);
  _exit(0);
}

// Starts the child when a remembered compile of this entry differs from this one only in procedure bodies.
void skeleton_start(SkeletonJob *job, const Source *src, const std::vector<Token> &tk, const SkeletonNow &now, const EmitOptions &opts, int library) {
  if (!now.made || skeleton_off()) return;
  std::shared_ptr<const PassedSkeleton> was;
  std::vector<int32_t> changed;
  const bool debug = getenv("COAX_SKELETON_DEBUG") != nullptr;
  const auto remembered = serve_passed(tree_path(src->units[0]->canonical));
  if (debug && remembered.empty()) fprintf(stderr, "coax skeleton: no compile of this entry is remembered\n");
  for (const auto &p : remembered) {
    std::string why;
    if (skeleton_changed(*p, now.skeleton, &changed, &why)) { was = p; break; }
    if (debug) fprintf(stderr, "coax skeleton: a remembered compile is not used: %s\n", why.c_str());
  }
  if (debug && was && changed.empty()) fprintf(stderr, "coax skeleton: no body changed\n");
  if (!was || changed.empty()) return;
  int ends[2];
  if (pipe(ends) != 0) return;
  fflush(stdout);
  fflush(stderr);
  const pid_t parent = getpid();
  const pid_t pid = fork();
  if (pid < 0) { close(ends[0]); close(ends[1]); return; }
  if (pid == 0) {
    close(ends[0]);
    job_child(parent);  // ends with the compile, and holds none of its build tokens
    serve_detach();
    skeleton_child(ends[1], src, tk, now, *was, changed, opts, library);
  }
  close(ends[1]);
  fcntl(ends[0], F_SETFD, FD_CLOEXEC);
  job->pid = pid;
  job->fd = ends[0];
  job->began = std::chrono::steady_clock::now();
}

}  // namespace
}  // namespace coax
