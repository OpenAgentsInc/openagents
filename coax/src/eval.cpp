// Compile-time execution of the checked subset. This deliberately walks the
// typed AST; a bytecode VM is a later, measured optimization. Values hold
// scalars, byte strings and aggregates, never host addresses or resources
// that could leak into a target executable. Every rule here matches the C
// the emitter writes: the same overflow, bounds, cast and order rules, except
// that compile-time arithmetic is checked in every build mode.
#include "coax.h"

#include <cmath>
#include <algorithm>
#include <map>
#include <optional>

// The same generated Coax kernels are compiled here and embedded in C output.
// Keep their rounding explicit even in test harnesses that include main.cpp.
#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC push_options
#pragma GCC optimize ("fp-contract=off")
#endif
namespace coax_strict_math {
#define _Bool bool
#define _Static_assert static_assert
#define _Alignof alignof
#include "../bootstrap/strict-math.c"
#undef _Alignof
#undef _Static_assert
#undef _Bool
#undef COAX_LAYOUT
#undef COAX_STR
#undef COAX_INT_OPS
}
#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC pop_options
#endif

namespace coax {

CtStorage::CtStorage(std::shared_ptr<CtMemory> memory)
    : std::vector<Value, CtAllocator<Value>>(CtAllocator<Value>(memory)), owner(std::move(memory)) {}
CtStorage::~CtStorage() { owner->forget(this); owner->release_container(); }

CtString::CtString(std::shared_ptr<CtMemory> memory, size_t n)
    : owner(std::move(memory)), size(n) {
  owner->take_string(size);
  try { bytes.reset(new char[size]); }
  catch (...) { owner->release_string(size); throw; }
}
CtString::~CtString() { owner->release_string(size); }


// A view can retain another aggregate, including a long chain of them. The
// last owner may disappear at scope exit or during cycle cleanup. Queue raw
// storage intrusively so releasing children never recurses on the host stack.
void CtMemory::dispose(CtStorage *storage) noexcept {
  storage->pending_next = pending;
  pending = storage;
  if (draining) return;
  draining = true;
  while (pending) {
    CtStorage *next = pending;
    pending = next->pending_next;
    delete next;
  }
  draining = false;
}

void CtMemory::remember(const std::shared_ptr<CtStorage> &storage) {
  CtStorage *s = storage.get();
  s->live_prev = nullptr;
  s->live_next = live;
  if (live) live->live_prev = s;
  live = s;
  s->linked = true;
  ++live_count;
}
void CtMemory::forget(CtStorage *s) noexcept {
  if (!s->linked) return;
  if (s->live_prev) s->live_prev->live_next = s->live_next;
  else live = s->live_next;
  if (s->live_next) s->live_next->live_prev = s->live_prev;
  s->linked = false;
  --live_count;
}
void CtMemory::clear_cycles() noexcept {
  // Views can form cycles. File teardown invalidates the entire interpreted
  // graph after emission, so break those cycles before dropping frozen roots.
  // Hold every live storage first: clearing one can release others.
  std::vector<std::shared_ptr<CtStorage>> all;
  try { all.reserve(live_count); } catch (...) {}
  for (CtStorage *s = live; s; s = s->live_next)
    if (auto storage = s->weak_from_this().lock()) all.push_back(std::move(storage));
  for (auto &storage : all) storage->clear();
}
File::~File() { if (ct_memory) ct_memory->clear_cycles(); }

std::vector<CtMemory::Holder> CtMemory::holders(bool by_storages, uint64_t request) const {
  std::map<int32_t, Holder> at;  // by origin offset
  if (request && where) {
    Holder &h = at[where->offset];
    h.origin = *where;
    h.proc = active && !active->empty() ? active->back() : nullptr;
    if (by_storages) h.storages += request; else h.slots += request;
  }
  for (const CtStorage *s = live; s; s = s->live_next) {
    Holder &h = at[s->origin.offset];
    h.origin = s->origin;
    if (!h.proc) h.proc = s->origin_proc;
    h.slots += s->capacity();
    h.storages++;
  }
  std::vector<Holder> out;
  for (auto &[offset, h] : at) out.push_back(h);
  std::stable_sort(out.begin(), out.end(), [by_storages](const Holder &a, const Holder &b) {
    return by_storages ? a.storages > b.storages : a.slots > b.slots;
  });
  if (out.size() > CAPACITY_TOP) out.resize(CAPACITY_TOP);
  return out;
}
void CtMemory::take_slots(size_t n) {
  if (!CB_admit(slots, n, slot_limit)) {
    try { at_failure = holders(false, n); } catch (...) { at_failure.clear(); }
    throw CtLimit{"value slots", slots + (uint64_t)n, slot_limit};
  }
  slots += n;
  if (slots > peak_slots) {
    peak_slots = slots;
    // The holders near the peak, for the near-limit report: once the use
    // passes the threshold, then at each further 1/32 of the limit.
    if (!snapshot_from) {
      uint64_t lo = 0, hi = slot_limit;  // the least use capacity_near reports
      while (lo < hi) { const uint64_t mid = lo + (hi - lo) / 2; if (capacity_near(mid, slot_limit)) hi = mid; else lo = mid + 1; }
      snapshot_from = lo ? lo : 1;
    }
    if (peak_slots >= snapshot_from) {
      try { at_peak = holders(false, n); at_peak_slots = peak_slots; } catch (...) {}
      snapshot_from = peak_slots + std::max<uint64_t>(slot_limit / 32, 1);
    }
  }
}
void CtMemory::take_string(size_t n) {
  if (!CB_admit(string_bytes, n, CB_string_live_limit())) throw CtStringLimit{};
  string_bytes += n;
  if (string_bytes > string_peak_bytes) string_peak_bytes = string_bytes;
}
void CtMemory::take_container() {
  if (!CB_admit(containers, 1, container_limit)) {
    try { at_failure = holders(true, 1); } catch (...) { at_failure.clear(); }
    throw CtLimit{"arrays and structs", containers + 1, container_limit};
  }
  ++containers;
  if (containers > peak_containers) peak_containers = containers;
}

bool has_call(const Expr *e) {
  if (!e) return false;
  if (e->kind == ExprKind::call) return true;
  if (has_call(e->lhs) || has_call(e->rhs) || has_call(e->third)) return true;
  for (const Expr *a : e->args) if (has_call(a)) return true;
  for (const FieldInit &f : e->fields) if (has_call(f.value)) return true;
  return false;
}

namespace {

struct CtLocal {
  Str name;
  Value value;
  bool initialized = true;
};

struct CtFrame {
  const ProcDecl *procedure = nullptr;
  std::vector<CtLocal> locals;

  // The same answer as str_eq, without a memcmp call for the short, often
  // one-letter names most lookups compare.
  static bool same_name(Str a, Str b) {
    if (a.count != b.count) return false;
    if (a.data == b.data || a.count == 0) return true;
    if (a.data[0] != b.data[0]) return false;
    for (int32_t i = 1; i < a.count; i++) if (a.data[i] != b.data[i]) return false;
    return true;
  }
  CtLocal *find(Str name) {
    for (size_t i = locals.size(); i-- > 0;)
      if (same_name(locals[i].name, name)) return &locals[i];
    return nullptr;
  }
};

struct ExprOutcome {
  Value value;
  bool returning = false;
  ExprOutcome() = default;
  ExprOutcome(Value v) : value(std::move(v)) {}
  static ExprOutcome returned(Value v) { ExprOutcome r(std::move(v));r.returning=true;return r; }
};
struct PlaceOutcome {
  Value *value = nullptr;
  Value returned_value;
  bool returning = false;
  PlaceOutcome() = default;
  PlaceOutcome(Value *v) : value(v) {}
  PlaceOutcome(ExprOutcome r) : returned_value(std::move(r.value)),returning(r.returning) {}
};
// Operand return is explicit and local to this recursive evaluation. It never
// uses Evaluator.failed or a process/frame-global pending-return flag.
#define CT_EVAL(name, node) \
  auto name##_outcome=expression((node),frame,allow_calls); \
  if(name##_outcome.returning)return name##_outcome; \
  if(failed)return {}; \
  Value name=std::move(name##_outcome.value)

// Keeps storage alive while a located element is in use. Most places keep
// at most one view and one temporary, so the first ones live inline and only
// more of them allocate (a host allocation, never compile-time storage).
struct Holder {
  std::shared_ptr<CtStorage> first_keep;
  std::vector<std::shared_ptr<CtStorage>> more_keep;
  std::optional<Value> first_temp;
  std::vector<std::unique_ptr<Value>> more_temps;
  void keep(std::shared_ptr<CtStorage> s) {
    if (!first_keep) first_keep = std::move(s);
    else more_keep.push_back(std::move(s));
  }
  Value *temp(Value v) {
    if (!first_temp) { first_temp.emplace(std::move(v)); return &*first_temp; }
    more_temps.push_back(std::make_unique<Value>(std::move(v)));
    return more_temps.back().get();
  }
};

enum class Flow : uint8_t { normal, ret, brk, cont };

__int128 as_int128(Type t, uint64_t bits) {
  return t->is_signed ? (__int128)(int64_t)bits : (__int128)bits;
}

uint64_t canon_bits(Type t, uint64_t bits) {
  if (t->bits >= 64) return bits;
  uint64_t mask = (UINT64_C(1) << t->bits) - 1;
  bits &= mask;
  if (t->is_signed && (bits >> (t->bits - 1)) & 1) bits |= ~mask;
  return bits;
}

bool fits_type(Type t, __int128 v) {
  if (t->is_signed) return v >= -((__int128)1 << (t->bits - 1)) && v <= ((__int128)1 << (t->bits - 1)) - 1;
  return v >= 0 && v <= (((__int128)1 << t->bits) - 1);
}

struct Evaluator {
  File *file;
  uint64_t steps = 0, max_steps = CB_default_steps();
  uint64_t string_work = 0;
  uint64_t traversal_work = 0, traversal_limit = g_traversal_limit ? g_traversal_limit : CB_traversal_limit();
  uint32_t dependency_depth = 0;
  Loc current_loc{};
  bool failed = false;
  bool quiet = false;  // checks: record the failure instead of printing it
  Loc fail_loc;
  std::string fail_code, fail_message;
  std::vector<Loc> calls;
  std::vector<const ProcDecl *> executed;      // in the order first entered
  // A procedure is in executed when its ct_executed_mark is this evaluator's.
  const uint64_t executed_mark = ++executed_marks;
  static inline uint64_t executed_marks = 0;
  // Each call depth's locals vector, kept between calls so a call doesn't
  // allocate one (cleared, so each call starts with none).
  std::vector<std::vector<CtLocal>> local_pool;
  // Compile-time call and constant-dependency depth. Fixed, with a stated
  // memory bound: the evaluator recurses on the native stack.
  static constexpr uint32_t MAX_DEPTH = 64;
  size_t peak_calls = 0;
  uint32_t peak_dependencies = 0;
  // --profile steps: the procedures being evaluated, innermost last, and each
  // one's own steps, total steps and calls.
  bool profile = false;
  std::vector<const ProcDecl *> frames;
  std::map<const ProcDecl *, StepProfileRow> profiled;
  // ---- capacity attribution (#12926) ----
  // Where the steps and the aggregate traversal went: each procedure's own
  // (charged while it is the innermost call), each call site's argument
  // copies, and each top-level site's own (a constant, an assertion, a check's
  // body). Kept at call boundaries, so an ordinary step costs nothing more.
  struct CostRow { std::string what; Loc loc; uint64_t steps = 0, traversal = 0, calls = 0; bool copies = false; };
  std::vector<CostRow> costs;
  std::map<const void *, uint32_t> cost_index;  // call sites and top-level sites
  struct Open { uint64_t steps = 0, traversal = 0; };  // the active call's callees, inclusive
  std::vector<Open> open;
  std::vector<const ProcDecl *> active;  // procedures being evaluated, innermost last
  const void *site = nullptr;            // the top-level site being evaluated
  uint64_t site_steps = 0, site_traversal = 0;  // its start
  Open site_inner;                       // its callees, inclusive
  const uint64_t cost_mark = ++cost_marks;
  static inline uint64_t cost_marks = 0;
  // The file's memory attributes requests to this evaluator while it runs.
  bool memory_attached = attach_memory();
  bool attach_memory() {
    if (file && file->ct_memory) { file->ct_memory->where = &current_loc; file->ct_memory->active = &active; }
    return true;
  }
  ~Evaluator() {
    if (file && file->ct_memory && file->ct_memory->where == &current_loc) { file->ct_memory->where = nullptr; file->ct_memory->active = nullptr; }
  }
  uint32_t cost_row(ProcDecl *p) {
    if (p->ct_cost_mark != cost_mark) {
      p->ct_cost_mark = cost_mark;
      p->ct_cost_row = (uint32_t)costs.size();
      costs.push_back({"'" + to_std(p->name) + "'", p->loc});
    }
    return p->ct_cost_row;
  }
  uint32_t cost_row(const void *key, const std::string &what, Loc loc, bool copies = false) {
    auto [at, made] = cost_index.emplace(key, (uint32_t)costs.size());
    if (made) costs.push_back({what, loc, 0, 0, 0, copies});
    return at->second;
  }
  void charge_parent(uint64_t ds, uint64_t dt) {
    Open &o = open.empty() ? site_inner : open.back();
    o.steps += ds;
    o.traversal += dt;
  }
  // Ends the current top-level site (its own cost is what its callees did not use) and starts key.
  void begin_site(const void *key, const std::string &what, Loc loc) {
    end_site();
    site = key;
    site_steps = steps;
    site_traversal = traversal_work;
    site_inner = {};
    if (key) cost_row(key, what, loc);
  }
  void end_site() noexcept {
    if (!site) return;
    auto at = cost_index.find(site);
    if (at != cost_index.end()) {
      CostRow &r = costs[at->second];
      r.steps += steps - site_steps - std::min(site_inner.steps, steps - site_steps);
      r.traversal += traversal_work - site_traversal - std::min(site_inner.traversal, traversal_work - site_traversal);
      r.calls++;
    }
    site = nullptr;
  }
  // A call's cost, charged when it returns or unwinds (a limit throws through it).
  struct CallCost {
    Evaluator &e;
    uint32_t row;
    uint64_t steps, traversal;
    CallCost(Evaluator &e, ProcDecl *p) : e(e), row(e.cost_row(p)), steps(e.steps), traversal(e.traversal_work) {
      e.open.push_back({});
      e.active.push_back(p);
    }
    ~CallCost() {
      const Open inner = e.open.back();
      e.open.pop_back();
      e.active.pop_back();
      const uint64_t ds = e.steps - steps, dt = e.traversal_work - traversal;
      CostRow &r = e.costs[row];
      r.steps += ds - std::min(inner.steps, ds);
      r.traversal += dt - std::min(inner.traversal, dt);
      r.calls++;
      e.charge_parent(ds, dt);
    }
  };
  // The consumers of steps (traversal: of aggregate traversal), largest first.
  CapacityDetail cost_detail(bool traversal, const CostRow **top = nullptr) {
    end_site();
    CapacityDetail d;
    d.src = file->src;
    d.unit = traversal ? "visits" : "steps";
    std::vector<std::pair<uint64_t, const CostRow *>> rows;
    for (const CostRow &r : costs)
      if (const uint64_t amount = traversal ? r.traversal : r.steps) rows.push_back({amount, &r});
    std::stable_sort(rows.begin(), rows.end(), [](const auto &a, const auto &b) { return a.first > b.first; });
    if (top) *top = rows.empty() ? nullptr : rows[0].second;
    for (const auto &[amount, r] : rows) {
      std::string what = r->what[0] == '\'' ? "in " + r->what : r->what;
      if (r->calls > 1) what += " (" + std::to_string(r->calls) + (r->copies ? " calls" : " times") + ")";
      d.consumers.push_back({what, r->loc, true, amount});
      if (d.consumers.size() == CAPACITY_TOP) break;
    }
    return d;
  }
  // A step-budget refusal or near-limit use: check is a #test/#forall (own: it has its own #steps).
  CapacityRefusal steps_capacity(bool check, bool own) {
    const CostRow *top = nullptr;
    CapacityRefusal r{"compile-time steps", steps_exceeded && steps_needed > steps ? steps_needed : steps, max_steps, cost_detail(false, &top)};
    const std::string name = top ? top->what : "";
    if (check)
      r.detail.fix = (top ? "do less work " + std::string(top->what[0] == '\'' ? "in " : "") + name + ", " : std::string()) +
                     "split the check, or give it a larger budget with " +
                     (own ? "a larger #steps N on the check" : "#steps N on the check (or coax test --steps N for every check)");
    else
      r.detail.fix = (top ? "compute " + name + " at run time or load it as data instead of at compile time, or " : std::string()) +
                     "raise the compilation's budget with #run_steps N; in the entry file (or --run-steps N)";
    return r;
  }
  CapacityRefusal traversal_capacity(bool check) {
    const CostRow *top = nullptr;
    CapacityRefusal r{"compile-time traversal work", traversal_exceeded ? traversal_work + 1 : traversal_work, traversal_limit, cost_detail(true, &top)};
    std::string fix;
    if (top && top->copies)
      fix = "pass a view, an index or only the fields it needs instead of " + top->what + " by value";
    else if (top)
      fix = "copy fewer whole arrays and structs " + std::string(top->what[0] == '\'' ? "in " : "") + top->what +
            " (pass a view or an index, assign single fields)";
    else fix = "copy fewer whole arrays and structs (pass a view or an index, assign single fields)";
    r.detail.fix = fix + (check ? "; or split the check (each check has its own traversal budget), or raise its budget with #traversal N on the check (or --traversal N for every check)"
                                : "; or raise the compilation's traversal budget with --traversal N");
    return r;
  }
  // An aggregate storage refusal: what was asked for, and who held the storage then.
  CapacityRefusal storage_capacity(const CtLimit &l, bool check) {
    const bool slots = std::string(l.what) == "value slots";
    CapacityRefusal r{slots ? "compile-time value slots" : "compile-time arrays and structs", l.needed, l.limit, {}};
    r.detail = storage_detail(file->ct_memory->at_failure, slots);
    r.detail.fix = storage_fix(r.detail, slots, check);
    return r;
  }
  CapacityDetail storage_detail(const std::vector<CtMemory::Holder> &holders, bool slots) {
    CapacityDetail d;
    d.src = file->src;
    d.unit = slots ? "slots" : "arrays and structs";
    for (const CtMemory::Holder &h : holders) {
      std::string what = h.storages == 0 ? std::string("requested here")
                         : std::string(h.storages == 1 ? "1 array or struct" : std::to_string(h.storages) + " arrays and structs") + " made here";
      if (h.proc) what += " in '" + to_std(h.proc->name) + "'";
      d.consumers.push_back({what, h.origin, true, slots ? h.slots : h.storages});
    }
    capacity_rank(&d.consumers);
    return d;
  }
  static std::string storage_fix(const CapacityDetail &d, bool slots, bool check) {
    std::string where = "the values";
    if (!d.consumers.empty()) {
      const SourcePoint at = source_point(d.src, d.consumers[0].loc);
      where = "the " + std::string(slots ? "array" : "values") + " made at " + at.path + ":" + std::to_string(at.loc.line);
    }
    if (check) return "make " + where + " smaller, or split the check so fewer values are live at once (each value slot is one array element or field)";
    return "make " + where + " smaller, or build the table at run time (or load it as data) instead of at compile time";
  }
  void err(Loc loc, const char *code, const std::string &message) {
    if (failed) return;
    failed = true;
    fail_loc = loc;
    fail_code = code;
    fail_message = message;
    if (quiet) return;
    print_failure(calls);
  }
  void print_failure(const std::vector<Loc> &trace) {
    error_at(file->src, fail_loc, fail_code.c_str(), fail_message);
    // A bounded call trace refers to source, not generated C or interpreter code.
    // A loaded file's call is named by its own path and line (GitHub #12660).
    for (size_t i = trace.size(); i-- > 0;) {
      const SourcePoint at = source_point(file->src, trace[i]);
      fprintf(stderr, "  note: compile-time call at %s:%d:%d\n", at.path.c_str(), at.loc.line, at.loc.col);
    }
  }
  // An exhausted step budget outside a check is printed once the calls have
  // unwound and charged their steps, with its consumers (held_failure).
  bool held_failure = false;
  std::vector<Loc> held_trace;
  uint64_t steps_needed = 0;

  // Gives back one step a caller charged twice, and the profile's with it.
  void refund() {
    steps--;
    if (profile) profiled[frames.empty() ? nullptr : frames.back()].own--;
  }
  bool steps_exceeded = false, traversal_exceeded = false;
  // One step (or cost steps) for a node. The common case, no failure, budget
  // left and no profile, stays inline; the rest is tick_slow.
  __attribute__((always_inline)) bool tick(Loc loc, uint32_t cost = 1) {
    current_loc = loc;
    if (__builtin_expect(!failed && steps <= max_steps && cost <= max_steps - steps && !profile, 1)) {
      steps += cost;
      return true;
    }
    return tick_slow(loc, cost);
  }
  __attribute__((noinline)) bool tick_slow(Loc loc, uint32_t cost) {
    if (failed) return false;
    if (steps > max_steps || cost > max_steps - steps) {
      steps_exceeded = true;
      steps_needed = steps + cost;
      if (!quiet) { held_failure = true; held_trace = calls; }
      const bool was_quiet = quiet;
      quiet = true;
      err(loc, "E0400", "compile-time evaluation exceeded " + std::to_string(max_steps) + " steps");
      quiet = was_quiet;
      return false;
    }
    steps += cost;
    if (profile) profiled[frames.empty() ? nullptr : frames.back()].own += cost;
    return true;
  }

  // A separate finite meter for aggregate traversal; it never changes the
  // expression/statement step budget. Admit before visiting the next value.
  void traverse() {
    if (!CB_admit(traversal_work, 1, traversal_limit)) { traversal_exceeded = true; throw CtTraversalLimit{}; }
    ++traversal_work;
  }

  // ---- values ----

  static Value scalar(Type t, uint64_t bits) {
    Value v;
    v.type = t;
    v.bits = bits;
    return v;
  }
  static Value floating(Type t, double f) {
    Value v;
    v.type = t;
    v.f = t == &T_F32 ? (double)(float)f : f;
    return v;
  }

  // A string is source or snapshot text (s), or a borrow of a []u8's bytes
  // (agg, offset, count) made by cast(string), as in emitted C: writes to
  // the bytes show through the string.
  static int64_t text_count(const Value &v) { return v.agg ? v.count : v.s.count; }
  static uint8_t text_byte(const Value &v, int64_t i) {
    return v.agg ? (uint8_t)(*v.agg)[(size_t)(v.offset + i)].bits : (uint8_t)v.s.data[i];
  }
  static bool text_equal(const Value &a, const Value &b) {
    if (!a.agg && !b.agg) return str_eq(a.s, b.s);
    int64_t n = text_count(a);
    if (n != text_count(b)) return false;
    for (int64_t i = 0; i < n; i++) if (text_byte(a, i) != text_byte(b, i)) return false;
    return true;
  }
  // A compile-time result outlives the bytes a string borrows: copy them
  // once, charging retained bytes and work, and share the copy.
  Value snapshot(const Value &v) {
    int64_t n = text_count(v);
    if (n < 0 || (uint64_t)n > INT32_MAX) throw CtStringLimit{};
    size_t size = (size_t)n + 1;
    if (!CB_admit(string_work, size, CB_string_work_limit())) throw CtStringLimit{};
    auto storage = std::make_shared<CtString>(file->ct_memory, size);
    string_work += size;
    for (int64_t i = 0; i < n; i++) storage->bytes[(size_t)i] = (char)text_byte(v, i);
    storage->bytes[(size_t)n] = 0;
    Value s;
    s.type = &T_STRING;
    s.s = Str{storage->bytes.get(), (int32_t)n};
    s.string_owner = std::move(storage);
    return s;
  }
  // Every borrowed string in a value to be frozen becomes a snapshot.
  void own_text(Value &v) {
    if (v.type == &T_STRING) { if (v.agg) v = snapshot(v); return; }
    if ((is_aggregate(v.type) || is_sum(v.type)) && v.agg)
      for (Value &e : *v.agg) { traverse(); own_text(e); }
  }

  std::shared_ptr<CtStorage> storage() { return storage(current_loc); }
  // at: the expression or declaration that makes it, for the slot report (#12926).
  std::shared_ptr<CtStorage> storage(Loc at) {
    auto memory = file->ct_memory;
    memory->take_container();
    CtStorage *raw;
    try { raw = new CtStorage(memory); }
    catch (...) { memory->release_container(); throw; }
    raw->origin = at;
    raw->origin_proc = active.empty() ? nullptr : active.back();
    // If control-block allocation fails, shared_ptr invokes this deleter.
    std::shared_ptr<CtStorage> result(raw, [memory](CtStorage *p) { memory->dispose(p); });
    memory->remember(result);
    return result;
  }

  Type payload_type(Type t, uint32_t tag, Loc loc) {
    if(t->kind==TypeKind::result){if(tag<2)return tag==0?t->elem:t->sum_error;}
    else if(t->kind==TypeKind::optional){if(tag<2)return tag==0?&T_VOID:t->elem;}
    else if(t->kind==TypeKind::union_ && t->union_decl && tag<(uint32_t)t->union_decl->variants.count){
      const auto &v=t->union_decl->variants[(int32_t)tag];if(v.ordinal==tag)return v.type;
    }
    err(loc,"E0401","invalid active sum tag during compile-time evaluation");return &T_INVALID;
  }
  bool valid_sum(const Value &v, Loc loc) {
    Type p=payload_type(v.type,v.sum_tag,loc);if(failed)return false;
    const size_t count=p==&T_VOID?0:1;
    if((v.agg?v.agg->size():0)!=count || (count && (*v.agg)[0].type!=p)){
      err(loc,"E0401","invalid active sum payload during compile-time evaluation");return false;
    }
    return true;
  }
  Value sum_value(Type t,uint32_t tag,Value payload,Loc loc) {
    Value v;v.type=t;v.sum_tag=tag;Type p=payload_type(t,tag,loc);if(failed)return v;
    if(p!=&T_VOID){if(payload.type!=p){err(loc,"E0401","sum payload disagrees with checked type");return v;}v.agg=storage(loc);v.agg->push_back(std::move(payload));}
    return v;
  }

  // Select the representation path, not defaultability/admission. Coax has
  // already checked every by-value field; views do not inline their pointees.
  std::map<Type,bool> inline_sum_cache;
  bool inline_sum(Type t) {
    auto found=inline_sum_cache.find(t);if(found!=inline_sum_cache.end())return found->second;
    inline_sum_cache.emplace(t,false);
    bool result=is_sum(t);
    if(t->kind==TypeKind::array)result=inline_sum(t->elem);
    else if(t->kind==TypeKind::struct_)for(const Field &field:t->struct_decl->fields)result=result || inline_sum(field.type);
    inline_sum_cache[t]=result;return result;
  }

  // The value of a declaration without an initializer: zero, or the
  // struct's field defaults. Arrays cost one step per element.
  Value zero(Type t, Loc loc) {
    Value v;
    v.type = t;
    if (is_sum(t)) {
      if(t->sum_default<0){err(loc,"E0401","nondefaultable sum reached compile-time initialization");return v;}
      return sum_value(t,(uint32_t)t->sum_default,{},loc);
    }
    if (t->kind == TypeKind::struct_) {
      StructDecl *d = t->struct_decl;
      field_defaults(d);
      v.agg = storage(loc);
      for (const Field &f : d->fields) {
        traverse();
        if (failed) return v;
        v.agg->push_back(f.init ? literal(f.init) : zero(f.type, loc));
      }
    } else if (t->kind == TypeKind::array) {
      if (!tick(loc, (uint32_t)t->length)) return v;
      v.agg = storage(loc);
      v.agg->reserve((size_t)t->length);
      Value element = zero(t->elem, loc);
      for (int64_t i = 0; i < t->length && !failed; i++) v.agg->push_back(clone(element, true));
    }
    return v;
  }

  // Aggregates copy deeply; views and strings keep sharing what they borrow.
  Value clone(const Value &v, bool nested = false) {
    // A number, bool or enum is its type and bits (or f): nothing to share.
    const TypeKind kind = v.type->kind;
    if (kind == TypeKind::integer || kind == TypeKind::floating || kind == TypeKind::bool_ || kind == TypeKind::enum_) {
      if (nested) traverse();
      Value c;
      c.type = v.type;
      c.bits = v.bits;
      c.f = v.f;
      c.offset = v.offset;
      c.count = v.count;
      c.sum_tag = v.sum_tag;
      c.s = v.s;
      if (__builtin_expect(v.string_owner != nullptr || v.agg != nullptr, 0)) { c.string_owner = v.string_owner; c.agg = v.agg; }
      return c;
    }
    if (nested || is_aggregate(v.type) || is_sum(v.type)) traverse();
    if(is_sum(v.type) && !valid_sum(v,current_loc))return {};
    Value c = v;
    if ((is_aggregate(v.type) || is_sum(v.type)) && v.agg) {
      c.agg = storage(current_loc);
      c.agg->reserve(v.agg->size());
      for (const Value &e : *v.agg) c.agg->push_back(clone(e, true));
    }
    return c;
  }

  // Assignment writes into existing storage, so views of it see the change.
  void assign_into(Value &dst, const Value &src, bool nested = false) {
    if (nested || is_aggregate(dst.type) || is_sum(dst.type)) traverse();
    if(is_sum(dst.type)){
      if(!valid_sum(dst,current_loc) || !valid_sum(src,current_loc))return;
      if(dst.sum_tag==src.sum_tag && dst.agg && src.agg)assign_into((*dst.agg)[0],(*src.agg)[0],true);
      else dst=clone(src);
      return;
    }
    if (is_aggregate(dst.type) && dst.agg && src.agg) {
      for (size_t i = 0; i < dst.agg->size(); i++) assign_into((*dst.agg)[i], (*src.agg)[i], true);
    } else {
      dst = src;
    }
  }

  // A frozen literal or compile-time constant node as a value.
  Value literal(const Expr *e) {
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::bool_lit: case ExprKind::enum_value: return scalar(e->type, e->int_value);
      case ExprKind::float_lit: return floating(e->type, e->float_value);
      case ExprKind::string_lit: { Value v; v.type = &T_STRING; v.s = e->text; return v; }
      case ExprKind::ct_value: return clone(*e->ct);
      default: return {};
    }
  }

  bool contains_view(const Value &v, bool nested = false) {
    if (nested || is_aggregate(v.type) || is_sum(v.type) || v.type->kind == TypeKind::view) traverse();
    if (v.type->kind == TypeKind::view) return true;
    if(is_sum(v.type) && !valid_sum(v,current_loc))return false;
    if (v.agg && (is_aggregate(v.type) || is_sum(v.type)))
      for (const Value &e : *v.agg) if (contains_view(e, true)) return true;
    return false;
  }

  void freeze(Expr *e, const Value &value, Loc loc) {
    if (failed) return;
    if (contains_view(value)) {
      err(loc, "E0409", "a compile-time result cannot contain a view; views borrow memory that exists only while compiling");
      return;
    }
    if (failed) return;
    // Prepare all fallible ownership and aggregate copies before touching the
    // syntax node; a limit or host-allocation failure must leave it intact.
    Value *saved = nullptr;
    Str text = value.s;
    switch (value.type->kind) {
      case TypeKind::integer: case TypeKind::bool_: case TypeKind::enum_:
      case TypeKind::floating: break;
      case TypeKind::string_:
        if (value.agg) {
          Value owned = snapshot(value);
          text = owned.s;
          file->frozen_values.push_back(std::make_unique<Value>(std::move(owned)));
        } else if (value.string_owner) file->frozen_values.push_back(std::make_unique<Value>(value));
        break;
      default: {
        Value owned = clone(value);
        own_text(owned);
        file->frozen_values.push_back(std::make_unique<Value>(std::move(owned)));
        saved = file->frozen_values.back().get();
        break;
      }
    }
    e->type = value.type;
    e->lhs = e->rhs = e->third = nullptr;
    switch (value.type->kind) {
      case TypeKind::integer: e->kind = ExprKind::int_lit; e->int_value = value.bits; break;
      case TypeKind::bool_: e->kind = ExprKind::bool_lit; e->int_value = value.bits; break;
      case TypeKind::enum_: e->kind = ExprKind::enum_value; e->int_value = value.bits; break;
      case TypeKind::floating: e->kind = ExprKind::float_lit; e->float_value = value.f; break;
      case TypeKind::string_:
        e->kind = ExprKind::string_lit; e->text = text; break;
      default:
        e->kind = ExprKind::ct_value;
        e->ct = saved;
        break;
    }
  }

  void field_defaults(StructDecl *d) {
    if (d->defaults == 2) return;
    if (d->defaults == 1) {
      err(d->loc, "E0402", "compile-time cycle in the defaults of struct '" + to_std(d->name) + "'");
      return;
    }
    struct Guard {
      StructDecl *decl;
      bool finished = false;
      ~Guard() { if (!finished) decl->defaults = 0; }
    } guard{d};
    d->defaults = 1;
    for (Field &f : d->fields) {
      if (!f.init || failed) continue;
      traverse();
      CtFrame empty;
      Value v = expr(f.init, empty, false);
      freeze(f.init, v, f.init->loc);
    }
    if (!failed) { d->defaults = 2; guard.finished = true; }
  }

  Value constant(ConstDecl *c) {
    if (c->evaluated == 1) {
      err(c->loc, "E0402", "compile-time constant cycle at '" + to_std(c->name) + "'");
      return {};
    }
    if (++dependency_depth > MAX_DEPTH) {
      err(c->loc, "E0403", "compile-time dependency depth exceeds 64");
      dependency_depth--;
      return {};
    }
    peak_dependencies = std::max(peak_dependencies, dependency_depth);
    CtFrame empty;
    bool done = c->evaluated == 2;
    // Allocation limits may throw through expr/clone. Restore the transient
    // cycle marker before another check or mutant can observe it.
    struct Guard {
      ConstDecl *constant;
      uint32_t &depth;
      uint8_t previous;
      bool finished = false;
      ~Guard() { if (!finished) constant->evaluated = previous; --depth; }
    } guard{c, dependency_depth, c->evaluated};
    c->evaluated = 1;
    Value result = expr(c->expr, empty, false);
    if (!failed && !done) freeze(c->expr, result, c->loc);
    c->evaluated = 2;
    guard.finished = true;
    return result;
  }

  // ---- arithmetic: the same rules as the C runtime, checked in every mode ----

  // The bits of an integer operation, or false after reporting its error.
  bool int_bits(Tok op, Type t, uint64_t a_bits, uint64_t b_bits, Type bt, Loc loc, uint64_t *out) {
    __int128 a = as_int128(t, a_bits), b = as_int128(bt ? bt : t, b_bits), r = 0;
    switch (op) {
      case Tok::plus: r = a + b; break;
      case Tok::minus: r = a - b; break;
      case Tok::star: r = a * b; break;
      case Tok::slash:
      case Tok::percent:
        if (b == 0) { err(loc, "E0407", "division by zero during compile-time evaluation"); return false; }
        r = op == Tok::slash ? a / b : a % b;
        break;
      case Tok::amp: *out = canon_bits(t, a_bits & b_bits); return true;
      case Tok::pipe: *out = canon_bits(t, a_bits | b_bits); return true;
      case Tok::caret: *out = canon_bits(t, a_bits ^ b_bits); return true;
      case Tok::shl: case Tok::shr: {
        if (b < 0 || b >= t->bits) {
          err(loc, "E0411", "shift by " + std::to_string((long long)b) + " is out of range for " + type_name(t));
          return false;
        }
        if (op == Tok::shl) { *out = canon_bits(t, a_bits << (int)b); return true; }
        if (t->is_signed) { *out = canon_bits(t, (uint64_t)((int64_t)a_bits >> (int)b)); return true; }
        uint64_t mask = t->bits >= 64 ? UINT64_MAX : (UINT64_C(1) << t->bits) - 1;
        *out = (a_bits & mask) >> (int)b;
        return true;
      }
      default: break;
    }
    if (!fits_type(t, r)) {
      err(loc, "E0407", std::string("integer overflow in ") + type_name(t) + " during compile-time evaluation");
      return false;
    }
    *out = canon_bits(t, (uint64_t)r);
    return true;
  }
  Value int_op(Tok op, Type t, uint64_t a_bits, uint64_t b_bits, Type bt, Loc loc) {
    uint64_t bits;
    if (!int_bits(op, t, a_bits, b_bits, bt, loc, &bits)) return {};
    return scalar(t, bits);
  }

  Value float_op(Tok op, Type t, double a, double b) {
    switch (op) {
      case Tok::plus: return floating(t, a + b);
      case Tok::minus: return floating(t, a - b);
      case Tok::star: return floating(t, a * b);
      case Tok::slash: return floating(t, a / b);
      default: return {};
    }
  }

  // Fixed sin/cos/exp/log: f32 widens exactly, then rounds once, as C does.
  ExprOutcome math(const Expr *e, CtFrame &frame, bool allow_calls) {
    CT_EVAL(first,e->args[0]);double a=first.f,b=0,c=0;
    if (e->args.count >= 2) {CT_EVAL(second,e->args[1]);b=second.f;}
    if (e->args.count == 3) {CT_EVAL(third,e->args[2]);c=third.f;}
    if (failed) return {};
    bool f32 = e->type == &T_F32;
    float x = (float)a, y = (float)b, z = (float)c;
    double r = 0;
    switch (e->intrinsic) {
      case Intrinsic::sin: r = coax_strict_math::SM_sin(a); if(f32) r=(float)r; break;
      case Intrinsic::cos: r = coax_strict_math::SM_cos(a); if(f32) r=(float)r; break;
      case Intrinsic::tan: r = f32 ? ::tanf(x) : std::tan(a); break;
      case Intrinsic::exp: r = coax_strict_math::SM_exp(a); if(f32) r=(float)r; break;
      case Intrinsic::log: r = coax_strict_math::SM_log(a); if(f32) r=(float)r; break;
      case Intrinsic::sqrt: r = f32 ? ::sqrtf(x) : std::sqrt(a); break;
      case Intrinsic::floor: r = f32 ? ::floorf(x) : std::floor(a); break;
      case Intrinsic::abs: r = f32 ? ::fabsf(x) : std::fabs(a); break;
      case Intrinsic::pow: r = f32 ? ::powf(x, y) : std::pow(a, b); break;
      case Intrinsic::fma: r = f32 ? ::fmaf(x, y, z) : std::fma(a, b, c); break;
      case Intrinsic::none: break;
    }
    if (!std::isfinite(r)) {
      err(e->loc, "E0412", to_std(e->text) + " has no finite result for these arguments while compiling");
      return {};
    }
    return floating(e->type, r);
  }

  Value do_cast(const Expr *e, Value v) {
    Type to = e->type, from = v.type;
    if (from == to) return v;
    if (to->kind == TypeKind::view && from->kind == TypeKind::view) { v.type=to; return v; }
    if (to == &T_STRING) {
      // The string borrows the view's bytes, as the spec and emitted C have
      // it: later writes through the array show in the string. A frozen
      // result takes a snapshot (freeze).
      if (v.count < 0 || (uint64_t)v.count > INT32_MAX) throw CtStringLimit{};
      Value s;
      s.type = &T_STRING;
      s.agg = std::move(v.agg);
      s.offset = v.offset;
      s.count = v.count;
      return s;
    }
    if (is_float(to)) {
      if (is_float(from)) {
        if (to == &T_F32 && std::isfinite(v.f) && std::isinf((float)v.f)) {
          err(e->loc, "E0411", "cast of " + std::to_string(v.f) + " to f32 is out of range");
          return {};
        }
        return floating(to, v.f);
      }
      // One rounding, straight from the integer, as C converts.
      if (to == &T_F32) return floating(to, from->is_signed ? (double)(float)(int64_t)v.bits : (double)(float)v.bits);
      return floating(to, from->is_signed ? (double)(int64_t)v.bits : (double)v.bits);
    }
    if (is_float(from)) {
      double f = std::trunc(v.f);
      if (std::isnan(f) || f <= -1.8e19 || f >= 1.9e19 || !fits_type(to, (__int128)f)) {
        err(e->loc, "E0411", std::string("cast of ") + std::to_string(v.f) + " to " + type_name(to) + " is out of range");
        return {};
      }
      return scalar(to, canon_bits(to, (uint64_t)(__int128)f));
    }
    // Integers and enums.
    __int128 n = as_int128(from, v.bits);
    if (to->kind == TypeKind::enum_) {
      for (const EnumMember &m : to->enum_decl->members)
        if (as_int128(to, m.bits) == n) return scalar(to, m.bits);
      err(e->loc, "E0411", std::string("no member of ") + type_name(to) + " has the value " + std::to_string((long long)n));
      return {};
    }
    if (e->trunc) return scalar(to, canon_bits(to, v.bits));
    if (!fits_type(to, n)) {
      err(e->loc, "E0411", std::string("cast to ") + type_name(to) + " is out of range");
      return {};
    }
    return scalar(to, canon_bits(to, (uint64_t)n));
  }

  int64_t index_value(const Value &v) {
    return v.type->is_signed ? (int64_t)v.bits : (v.bits > (uint64_t)INT64_MAX ? -1 : (int64_t)v.bits);
  }

  // ---- places ----

  // The storage an expression names, or a temporary holding its value.
  PlaceOutcome place(Expr *e, CtFrame &frame, Holder &h, bool allow_calls, CtLocal **root = nullptr) {
    if (!tick(e->loc)) return nullptr;
    switch (e->kind) {
      case ExprKind::ident: {
        if (e->constant) {
          // A frozen aggregate constant is read in place: an element read
          // (FONT[i]) must not copy the whole array first.
          if (e->constant->evaluated == 2 && e->constant->expr->kind == ExprKind::ct_value) return e->constant->expr->ct;
          Value v = constant(e->constant);
          if (failed) return nullptr;
          if (e->constant->expr->kind == ExprKind::ct_value) return e->constant->expr->ct;
          return h.temp(v);
        }
        CtLocal *local = frame.find(e->text);
        if (!local) {
          err(e->loc, "E0401", "compile-time expression cannot capture runtime value '" + to_std(e->text) + "'");
          return nullptr;
        }
        if (root) *root = local;
        return &local->value;
      }
      case ExprKind::ct_value: return e->ct;
      case ExprKind::field: {
        if (e->field_index < 0) break;
        auto located=place(e->lhs,frame,h,allow_calls,root);if(located.returning)return ExprOutcome::returned(std::move(located.returned_value));Value *base=located.value;
        if (!base || failed) return nullptr;
        return &(*base->agg)[(size_t)e->field_index];
      }
      case ExprKind::index: {
        Type lt = e->lhs->type;
        if (lt == &T_STRING) break;
        Value *base = nullptr;
        std::optional<Value> copied;  // a view that is not a local's, copied as expression() reads it
        const Value *local_view = nullptr;
        if (lt->kind == TypeKind::view && e->lhs->kind == ExprKind::ident && !e->lhs->constant) {
          // A local view indexed in place (w[i], the common case): the same
          // step, lookup and checks as reading the view (expression(),
          // which charges a step, refunds it and lets place() charge it
          // again), without copying it. Nothing an index expression does can
          // change or drop a caller's local, so the local keeps the storage.
          if (!tick(e->lhs->loc)) return nullptr;
          CtLocal *local = frame.find(e->lhs->text);
          if (!local) {
            err(e->lhs->loc, "E0401", "compile-time expression cannot capture runtime value '" + to_std(e->lhs->text) + "'");
            return nullptr;
          }
          if (!local->initialized) {
            err(e->lhs->loc, "E0405", "compile-time read of an uninitialized variable");
            return nullptr;
          }
          local_view = &local->value;
          if (root) *root = nullptr;  // a view's elements are always initialized storage
        } else if (lt->kind == TypeKind::view) {
          CT_EVAL(value,e->lhs);copied.emplace(std::move(value));
          if (root) *root = nullptr;  // a view's elements are always initialized storage
        } else {
          auto located=place(e->lhs,frame,h,allow_calls,root);if(located.returning)return ExprOutcome::returned(std::move(located.returned_value));base=located.value;
        }
        if (failed) return nullptr;
        Scalar iv;
        {
          std::optional<ExprOutcome> sc_ret;
          const int sc = scalar_eval(e->rhs, frame, allow_calls, iv, sc_ret);
          if (sc == SC_RETURN) return std::move(*sc_ret);
          if (sc == SC_STOP || failed) return nullptr;
        }
        int64_t i = iv.type->is_signed ? (int64_t)iv.bits : (iv.bits > (uint64_t)INT64_MAX ? -1 : (int64_t)iv.bits);
        int64_t count = base ? lt->length : local_view ? local_view->count : copied->count;
        if (i < 0 || i >= count) {
          err(e->loc, "E0410", "index " + std::to_string((long long)(int64_t)iv.bits) + " is out of bounds for " +
                                   std::to_string((long long)count) + " elements");
          return nullptr;
        }
        if (base) return &(*base->agg)[(size_t)i];
        if (local_view) return &(*local_view->agg)[(size_t)(local_view->offset + i)];
        Value *element = &(*copied->agg)[(size_t)(copied->offset + i)];
        h.keep(std::move(copied->agg));
        return element;
      }
      default: break;
    }
    refund();  // expr() charges its own step
    CT_EVAL(value,e);return h.temp(std::move(value));
  }

  // Non-procedure entry points preserve the Value API and explicitly reject a
  // malformed checked tree that tries to leave an absent current frame.
  Value expr(Expr *e,CtFrame &frame,bool allow_calls){
    auto r=expression(e,frame,allow_calls);
    if(r.returning){err(e->loc,"E0401","compile-time propagation has no enclosing procedure");return {};}
    return std::move(r.value);
  }


  // ---- scalar expressions without a Value ----
  //
  // Most of a check's work is numbers: indexes, counters, coordinates and
  // comparisons. scalar_eval computes a number, bool or enum expression as
  // expression() does, node for node: the same steps, in the same order,
  // the same checks and the same errors at the same places, without building,
  // moving and destroying a Value for every node. A node it doesn't handle
  // goes to expression(). It returns SC_OK with the result in out, SC_STOP
  // when evaluation failed (as expression() returning {} with failed set), or
  // SC_RETURN with the outcome to return in ret (a propagation).
  struct Scalar { Type type; uint64_t bits; double f; };
  enum { SC_OK, SC_STOP, SC_RETURN };
  static bool scalar_type(Type t) {
    const TypeKind k = t->kind;
    return k == TypeKind::integer || k == TypeKind::floating || k == TypeKind::bool_ || k == TypeKind::enum_;
  }
  static Value scalar_value(const Scalar &v) {
    Value r;
    r.type = v.type;
    r.bits = v.bits;
    r.f = v.f;
    return r;
  }
  int scalar_fallback(Expr *e, CtFrame &frame, bool allow_calls, Scalar &out, std::optional<ExprOutcome> &ret) {
    ExprOutcome r = expression(e, frame, allow_calls);
    if (r.returning) { ret.emplace(std::move(r)); return SC_RETURN; }
    if (failed) return SC_STOP;
    out = {r.value.type, r.value.bits, r.value.f};
    return SC_OK;
  }
  int scalar_eval(Expr *e, CtFrame &frame, bool allow_calls, Scalar &out, std::optional<ExprOutcome> &ret) {
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::bool_lit: case ExprKind::enum_value:
        if (!tick(e->loc)) return SC_STOP;
        out = {e->type, (uint64_t)e->int_value, 0};
        return SC_OK;
      case ExprKind::float_lit:
        if (!tick(e->loc)) return SC_STOP;
        out = {e->type, 0, e->type == &T_F32 ? (double)(float)e->float_value : e->float_value};
        return SC_OK;
      case ExprKind::ident:
        if (!e->constant) {
          // expression(): a step, refunded; place(): the step again.
          if (!tick(e->loc)) return SC_STOP;
          refund();
          if (!tick(e->loc)) return SC_STOP;
          CtLocal *local = frame.find(e->text);
          if (!local) {
            err(e->loc, "E0401", "compile-time expression cannot capture runtime value '" + to_std(e->text) + "'");
            return SC_STOP;
          }
          if (!local->initialized) {
            err(e->loc, "E0405", "compile-time read of an uninitialized variable");
            return SC_STOP;
          }
          const Value &v = local->value;
          out = {v.type, v.bits, v.f};
          return SC_OK;
        }
        break;
      case ExprKind::field: case ExprKind::index: {
        if (e->kind == ExprKind::field && e->field_index == -2) break;
        if (e->kind == ExprKind::index && e->lhs->type == &T_STRING) break;
        if (!tick(e->loc)) return SC_STOP;
        Holder h;
        CtLocal *root = nullptr;
        refund();
        auto located = place(e, frame, h, allow_calls, &root);
        if (located.returning) { ret.emplace(ExprOutcome::returned(std::move(located.returned_value))); return SC_RETURN; }
        Value *v = located.value;
        if (!v || failed) return SC_STOP;
        if (root && !root->initialized) {
          err(e->loc, "E0405", "compile-time read of an uninitialized variable");
          return SC_STOP;
        }
        out = {v->type, v->bits, v->f};
        return SC_OK;
      }
      case ExprKind::unary: {
        if (!scalar_type(e->lhs->type)) break;
        if (!tick(e->loc)) return SC_STOP;
        Scalar a;
        int r = scalar_eval(e->lhs, frame, allow_calls, a, ret);
        if (r != SC_OK) return r;
        if (failed) return SC_STOP;
        if (e->op == Tok::bang) { out = {&T_BOOL, (uint64_t)!a.bits, 0}; return SC_OK; }
        if (is_float(a.type)) { out = {a.type, 0, a.type == &T_F32 ? (double)(float)-a.f : -a.f}; return SC_OK; }
        uint64_t bits;
        if (!int_bits(Tok::minus, a.type, 0, a.bits, nullptr, e->loc, &bits)) return SC_STOP;
        out = {a.type, bits, 0};
        return SC_OK;
      }
      case ExprKind::binary: {
        if (!scalar_type(e->lhs->type) || !scalar_type(e->rhs->type)) break;
        if (!tick(e->loc)) return SC_STOP;
        Scalar a, b;
        int r = scalar_eval(e->lhs, frame, allow_calls, a, ret);
        if (r != SC_OK) return r;
        if (failed) return SC_STOP;
        if (e->op == Tok::andand && !a.bits) { out = {&T_BOOL, 0, 0}; return SC_OK; }
        if (e->op == Tok::oror && a.bits) { out = {&T_BOOL, 1, 0}; return SC_OK; }
        r = scalar_eval(e->rhs, frame, allow_calls, b, ret);
        if (r != SC_OK) return r;
        if (failed) return SC_STOP;
        Type t = a.type;
        switch (e->op) {
          case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
          case Tok::amp: case Tok::pipe: case Tok::caret:
            if (is_float(t)) {
              double f = 0;
              switch (e->op) {
                case Tok::plus: f = a.f + b.f; break;
                case Tok::minus: f = a.f - b.f; break;
                case Tok::star: f = a.f * b.f; break;
                case Tok::slash: f = a.f / b.f; break;
                default: out = {&T_VOID, 0, 0}; return SC_OK;  // float_op's {}: no such operator is checked
              }
              out = {t, 0, t == &T_F32 ? (double)(float)f : f};
              return SC_OK;
            } else {
              uint64_t bits;
              if (!int_bits(e->op, t, a.bits, b.bits, nullptr, e->loc, &bits)) return SC_STOP;
              out = {t, bits, 0};
              return SC_OK;
            }
          case Tok::shl: case Tok::shr: {
            uint64_t bits;
            if (!int_bits(e->op, t, a.bits, b.bits, b.type, e->loc, &bits)) return SC_STOP;
            out = {t, bits, 0};
            return SC_OK;
          }
          case Tok::andand: case Tok::oror: out = {&T_BOOL, (uint64_t)(b.bits != 0), 0}; return SC_OK;
          default: break;
        }
        bool result = false;
        if (is_float(t)) {
          switch (e->op) {
            case Tok::eqeq: result = a.f == b.f; break;
            case Tok::noteq: result = a.f != b.f; break;
            case Tok::lt: result = a.f < b.f; break;
            case Tok::le: result = a.f <= b.f; break;
            case Tok::gt: result = a.f > b.f; break;
            case Tok::ge: result = a.f >= b.f; break;
            default: break;
          }
        } else {
          __int128 x = as_int128(t, a.bits), y = as_int128(t, b.bits);
          switch (e->op) {
            case Tok::eqeq: result = x == y; break;
            case Tok::noteq: result = x != y; break;
            case Tok::lt: result = x < y; break;
            case Tok::le: result = x <= y; break;
            case Tok::gt: result = x > y; break;
            case Tok::ge: result = x >= y; break;
            default: break;
          }
        }
        out = {&T_BOOL, (uint64_t)result, 0};
        return SC_OK;
      }
      case ExprKind::ifx: {
        if (!tick(e->loc)) return SC_STOP;
        Scalar c;
        int r = scalar_eval(e->lhs, frame, allow_calls, c, ret);
        if (r != SC_OK) return r;
        if (failed) return SC_STOP;
        Expr *branch = c.bits ? e->rhs : e->third;
        if (!scalar_type(branch->type)) {
          ExprOutcome v = expression(branch, frame, allow_calls);
          if (v.returning) { ret.emplace(std::move(v)); return SC_RETURN; }
          if (failed) return SC_STOP;
          out = {v.value.type, v.value.bits, v.value.f};
          return SC_OK;
        }
        return scalar_eval(branch, frame, allow_calls, out, ret);
      }
      default: break;
    }
    return scalar_fallback(e, frame, allow_calls, out, ret);
  }

  // ---- expressions ----

  ExprOutcome expression(Expr *e, CtFrame &frame, bool allow_calls) {
    if ((e->kind == ExprKind::binary && scalar_type(e->lhs->type) && scalar_type(e->rhs->type)) ||
        (e->kind == ExprKind::unary && scalar_type(e->lhs->type))) {
      Scalar v;
      std::optional<ExprOutcome> ret;
      const int r = scalar_eval(e, frame, allow_calls, v, ret);
      if (r == SC_RETURN) return std::move(*ret);
      if (r == SC_STOP) return {};
      return scalar_value(v);
    }
    if (!tick(e->loc)) return {};
    switch (e->kind) {
      case ExprKind::int_lit: case ExprKind::bool_lit: case ExprKind::enum_value:
      case ExprKind::float_lit: case ExprKind::string_lit: case ExprKind::ct_value:
        return literal(e);
      case ExprKind::ident:
      case ExprKind::field:
      case ExprKind::index: {
        if (e->kind == ExprKind::field && e->field_index == -2) {
          // .count: an array's is its length; a view's or string's is stored.
          Type lt = e->lhs->type;
          if (lt->kind == TypeKind::array) {
            if (has_call(e->lhs) || (e->lhs->effects & 8)) {CT_EVAL(ignored,e->lhs);}
            return scalar(T_INT, (uint64_t)lt->length);
          }
          CT_EVAL(v,e->lhs);
          return scalar(T_INT, (uint64_t)(lt == &T_STRING ? text_count(v) : v.count));
        }
        if (e->kind == ExprKind::index && e->lhs->type == &T_STRING) {
          CT_EVAL(s,e->lhs);
          if (failed) return {};
          CT_EVAL(iv,e->rhs);
          if (failed) return {};
          int64_t i = index_value(iv);
          if (i < 0 || i >= text_count(s)) {
            err(e->loc, "E0410", "index " + std::to_string((long long)(int64_t)iv.bits) + " is out of bounds for a " +
                                     std::to_string((long long)text_count(s)) + "-byte string");
            return {};
          }
          return scalar(&T_U8, text_byte(s, i));
        }
        Holder h;
        CtLocal *root = nullptr;
        refund();
        auto located=place(e,frame,h,allow_calls,&root);if(located.returning)return ExprOutcome::returned(std::move(located.returned_value));Value *v=located.value;
        if (!v || failed) return {};
        if (root && !root->initialized) {
          err(e->loc, "E0405", "compile-time read of an uninitialized variable");
          return {};
        }
        return clone(*v);
      }
      case ExprKind::run: {
        CtFrame isolated;
        auto result=expression(e->lhs,isolated,true);if(result.returning){err(e->loc,"E0401","compile-time propagation has no enclosing procedure");return {};}Value value=std::move(result.value);
        freeze(e, value, e->loc);
        return value;
      }
      case ExprKind::call: {
        if (!allow_calls) {
          err(e->loc, "E0408", "a constant procedure call requires #run");
          return {};
        }
        if (e->byte_intrinsic != ByteIntrinsic::none) {
          if(e->byte_intrinsic == ByteIntrinsic::bytes_overlap) {err(e->loc,"E0404","runtime storage overlap cannot execute at compile time");return {};}
          CT_EVAL(a,e->args[0]);Value r;r.type=e->type;
          if(e->byte_intrinsic == ByteIntrinsic::secure_zero) {
            if(a.count<0 || (a.count && (!a.agg || a.offset<0 || (uint64_t)a.offset>a.agg->size() || (uint64_t)a.count>a.agg->size()-(uint64_t)a.offset))) {
              err(e->loc,"E0404","invalid secure_zero view");return {};
            }
            Value cleared;cleared.type=a.type->elem;
            for(int64_t i=0;i<a.count;i++) {if(!tick(e->loc))return {};assign_into((*a.agg)[(size_t)(a.offset+i)],cleared);}
            return r;
          }
          if(e->byte_intrinsic == ByteIntrinsic::value_barrier){r.bits=a.bits;return r;}
          if(e->byte_intrinsic == ByteIntrinsic::mul_wide) {
            CT_EVAL(b,e->args[1]);
            unsigned __int128 product=(unsigned __int128)a.bits*b.bits;
            r=zero(e->type,e->loc);(*r.agg)[0].bits=(uint64_t)(product>>64);(*r.agg)[1].bits=(uint64_t)product;return r;
          }
          if(e->byte_intrinsic >= ByteIntrinsic::add_wrap) {
            CT_EVAL(b,e->args[1]);
            unsigned width=e->type==&T_U32?32:64,k=(unsigned)(b.bits&(width-1));
            switch(e->byte_intrinsic) {
              case ByteIntrinsic::add_wrap:r.bits=a.bits+b.bits;break;
              case ByteIntrinsic::sub_wrap:r.bits=a.bits-b.bits;break;
              case ByteIntrinsic::mul_wrap:r.bits=a.bits*b.bits;break;
              case ByteIntrinsic::rotl:r.bits=(a.bits<<k)|(a.bits>>((width-k)&(width-1)));break;
              case ByteIntrinsic::rotr:r.bits=(a.bits>>k)|(a.bits<<((width-k)&(width-1)));break;
              default:break;
            }
            if(width==32)r.bits&=UINT32_MAX;
            return r;
          }
          if(e->byte_intrinsic == ByteIntrinsic::f32_bits){float x=(float)a.f;uint32_t b;memcpy(&b,&x,4);r.bits=b;}
          else if(e->byte_intrinsic == ByteIntrinsic::f64_bits){memcpy(&r.bits,&a.f,8);}
          else if(e->byte_intrinsic == ByteIntrinsic::f32_from_bits){uint32_t b=(uint32_t)a.bits;float x;memcpy(&x,&b,4);r.f=x;}
          else {memcpy(&r.f,&a.bits,8);}
          if(is_float(r.type) && !std::isfinite(r.f)) {err(e->loc,"E0412","non-finite compile-time bit conversion");return {};}
          return r;
        }
        if (e->intrinsic != Intrinsic::none) return math(e, frame, allow_calls);
        if (e->builtin_print) {
          err(e->loc, "E0404", "compile-time execution cannot perform output or host I/O");
          return {};
        }
        if (e->proc->foreign) {
          err(e->loc, "E0404", "compile-time execution cannot call the host procedure '" + to_std(e->proc->name) + "'");
          return {};
        }
        if (calls.size() >= MAX_DEPTH) {
          err(e->loc, "E0403", "compile-time call depth exceeds 64");
          return {};
        }
        CtFrame callee;callee.procedure=e->proc;
        const size_t depth = calls.size();
        if (local_pool.size() <= depth) local_pool.resize(depth + 1);
        callee.locals.swap(local_pool[depth]);
        struct ReturnLocals {
          // By index: a nested call can grow local_pool.
          std::vector<std::vector<CtLocal>> &pool;
          size_t depth;
          std::vector<CtLocal> &locals;
          ~ReturnLocals() { locals.clear(); pool[depth].swap(locals); }
        } return_locals{local_pool, depth, callee.locals};
        // Argument copies are charged to this call site, not to the caller (#12926).
        const uint64_t args_traversal = traversal_work;
        const uint64_t args_inner = open.empty() ? site_inner.traversal : open.back().traversal;
        struct ArgsCost {
          Evaluator &ev; const Expr *call; uint64_t traversal, inner; bool done = false;
          ~ArgsCost() { finish(); }
          void finish() noexcept {
            if (done) return;
            done = true;
            const uint64_t dt = ev.traversal_work - traversal;
            const uint64_t nested = (ev.open.empty() ? ev.site_inner.traversal : ev.open.back().traversal) - inner;
            if (dt <= nested) return;
            try {
              CostRow &r = ev.costs[ev.cost_row(call, "copying arguments into '" + to_std(call->proc->name) + "'", call->loc, true)];
              r.traversal += dt - nested;
              r.calls++;
              ev.charge_parent(0, dt - nested);
            } catch (...) {}
          }
        } args_cost{*this, e, args_traversal, args_inner};
        for (int i = 0; i < e->args.count; i++) {
          if (scalar_type(e->args[i]->type)) {
            Scalar v;
            std::optional<ExprOutcome> sc_ret;
            const int sc = scalar_eval(e->args[i], frame, allow_calls, v, sc_ret);
            if (sc == SC_RETURN) return std::move(*sc_ret);
            if (sc == SC_STOP || failed) return {};
            callee.locals.push_back({e->proc->params[i].name, scalar_value(v), true});
            continue;
          }
          CT_EVAL(value,e->args[i]);
          if (failed) return {};
          callee.locals.push_back({e->proc->params[i].name, std::move(value), true});
        }
        args_cost.finish();
        if (quiet && e->proc->ct_executed_mark != executed_mark) { e->proc->ct_executed_mark = executed_mark; executed.push_back(e->proc); }
        calls.push_back(e->loc);
        peak_calls = std::max(peak_calls, calls.size());
        Value result;
        CallCost call_cost{*this, e->proc};
        const uint64_t before = steps;
        const bool recursive = profile && std::find(frames.begin(), frames.end(), e->proc) != frames.end();
        if (profile) frames.push_back(e->proc);
        stmt(e->proc->body, callee, &result);
        if (profile) {
          frames.pop_back();
          StepProfileRow &row = profiled[e->proc];
          row.calls++;
          if (!recursive) row.total += steps - before;
        }
        calls.pop_back();
        return result;
      }
      case ExprKind::sum:
      case ExprKind::some: {
        Value payload;if(e->lhs){CT_EVAL(value,e->lhs);payload=std::move(value);}
        return sum_value(e->type,(uint32_t)e->sum_variant,std::move(payload),e->loc);
      }
      case ExprKind::null_:return sum_value(e->type,0,{},e->loc);
      case ExprKind::try_: {
        CT_EVAL(value,e->lhs);if(!valid_sum(value,e->loc))return {};
        const uint32_t success=value.type->kind==TypeKind::optional?1:0;
        if(value.sum_tag==success)return value.agg?clone((*value.agg)[0]):Value{};
        if(!e->propagate_type || !frame.procedure || frame.procedure->ret!=e->propagate_type){
          err(e->loc,"E0401","propagation target disagrees with checked current frame");return {};
        }
        Value payload=value.agg?clone((*value.agg)[0]):Value{};
        return ExprOutcome::returned(sum_value(e->propagate_type,(uint32_t)e->sum_variant,std::move(payload),e->loc));
      }
      case ExprKind::or_else: {
        CT_EVAL(value,e->lhs);if(!valid_sum(value,e->loc))return {};
        const uint32_t success=value.type->kind==TypeKind::optional?1:0;
        if(value.sum_tag==success)return value.agg?clone((*value.agg)[0]):Value{};
        return expression(e->rhs,frame,allow_calls);
      }
      case ExprKind::unary: {
        CT_EVAL(a,e->lhs);
        if (failed) return {};
        if (e->op == Tok::bang) return scalar(&T_BOOL, !a.bits);
        if (is_float(a.type)) return floating(a.type, -a.f);
        return int_op(Tok::minus, a.type, 0, a.bits, nullptr, e->loc);
      }
      case ExprKind::binary: {
        CT_EVAL(a,e->lhs);
        if (failed) return {};
        if (e->op == Tok::andand && !a.bits) return scalar(&T_BOOL, 0);
        if (e->op == Tok::oror && a.bits) return scalar(&T_BOOL, 1);
        CT_EVAL(b,e->rhs);
        if (failed) return {};
        Type t = a.type;
        switch (e->op) {
          case Tok::plus: case Tok::minus: case Tok::star: case Tok::slash: case Tok::percent:
          case Tok::amp: case Tok::pipe: case Tok::caret:
            if (is_float(t)) return float_op(e->op, t, a.f, b.f);
            return int_op(e->op, t, a.bits, b.bits, nullptr, e->loc);
          case Tok::shl: case Tok::shr: return int_op(e->op, t, a.bits, b.bits, b.type, e->loc);
          case Tok::andand: case Tok::oror: return scalar(&T_BOOL, b.bits != 0);
          default: break;
        }
        bool result = false;
        if(t->kind==TypeKind::optional){
          if(!valid_sum(a,e->loc) || !valid_sum(b,e->loc))return {};
          result=a.sum_tag==b.sum_tag;
          if(e->op==Tok::noteq)result=!result;
        } else if (t == &T_STRING) {
          result = text_equal(a, b);
          if (e->op == Tok::noteq) result = !result;
        } else if (is_float(t)) {
          switch (e->op) {
            case Tok::eqeq: result = a.f == b.f; break;
            case Tok::noteq: result = a.f != b.f; break;
            case Tok::lt: result = a.f < b.f; break;
            case Tok::le: result = a.f <= b.f; break;
            case Tok::gt: result = a.f > b.f; break;
            case Tok::ge: result = a.f >= b.f; break;
            default: break;
          }
        } else {
          __int128 x = as_int128(t, a.bits), y = as_int128(t, b.bits);
          switch (e->op) {
            case Tok::eqeq: result = x == y; break;
            case Tok::noteq: result = x != y; break;
            case Tok::lt: result = x < y; break;
            case Tok::le: result = x <= y; break;
            case Tok::gt: result = x > y; break;
            case Tok::ge: result = x >= y; break;
            default: break;
          }
        }
        return scalar(&T_BOOL, result);
      }
      case ExprKind::cast: {
        CT_EVAL(v,e->lhs);
        if (failed) return {};
        return do_cast(e, std::move(v));
      }
      case ExprKind::ifx: {
        CT_EVAL(c,e->lhs);
        if (failed) return {};
        return expression(c.bits ? e->rhs : e->third, frame, allow_calls);
      }
      case ExprKind::struct_lit: {
        Value v;
        if(!inline_sum(e->type))v=zero(e->type,e->loc);
        else {
          v.type=e->type;v.agg=storage(e->loc);
          StructDecl *decl=e->type->struct_decl;field_defaults(decl);
          for(int32_t i=0;i<decl->fields.count;i++){
            traverse();bool supplied=false;
            for(const FieldInit &field:e->fields)if(field.field==i){supplied=true;break;}
            const Field &field=decl->fields[i];
            // A supplied field gets storage without constructing a default
            // value. Its initializer is evaluated below, in authored order.
            v.agg->push_back(supplied ? Value{} : field.init ? literal(field.init) : zero(field.type,e->loc));
            if(failed)return {};
          }
        }
        for (const FieldInit &fi : e->fields) {
          if (failed) return {};
          traverse();
          CT_EVAL(value,fi.value);(*v.agg)[(size_t)fi.field]=std::move(value);
        }
        return v;
      }
      case ExprKind::array_lit: {
        Value v;
        v.type = e->type;
        v.agg = storage(e->loc);
        for (Expr *a : e->args) {
          if (failed) return {};
          traverse();
          CT_EVAL(value,a);v.agg->push_back(std::move(value));
        }
        return v;
      }
      case ExprKind::slice: {
        Type lt = e->lhs->type;
        Holder h;
        Value base;
        int64_t count = 0;
        if (lt->kind == TypeKind::array) {
          auto located=place(e->lhs,frame,h,allow_calls);if(located.returning)return ExprOutcome::returned(std::move(located.returned_value));Value *storage=located.value;
          if (!storage || failed) return {};
          base.type = e->type;
          base.agg = storage->agg;
          base.offset = 0;
          count = lt->length;
        } else {
          CT_EVAL(value,e->lhs);base=std::move(value);
          if (failed) return {};
          count = lt == &T_STRING ? text_count(base) : base.count;
        }
        int64_t lo = 0, hi = count;
        if (e->rhs) {
          CT_EVAL(l,e->rhs);
          if (failed) return {};
          CT_EVAL(r,e->third);
          if (failed) return {};
          lo = index_value(l);
          hi = index_value(r);
        }
        if (lo < 0 || hi < lo || hi > count) {
          err(e->loc, "E0410", "slice " + std::to_string((long long)lo) + "..<" + std::to_string((long long)hi) +
                                   " is out of bounds for " + std::to_string((long long)count) + " elements");
          return {};
        }
        if (lt == &T_STRING && !base.agg) {
          base.s = Str{base.s.data + lo, (int32_t)(hi - lo)};
          return base;
        }
        base.type = e->type;
        base.offset += lo;
        base.count = hi - lo;
        return base;
      }
    }
    return {};
  }

  // ---- statements ----

#define CT_STMT_EVAL(name, node) \
  auto name##_outcome=expression((node),frame,true); \
  if(name##_outcome.returning){*returned=std::move(name##_outcome.value);return Flow::ret;} \
  if(failed)return Flow::normal; \
  Value name=std::move(name##_outcome.value)
// The same for an expression known to be a number, bool or enum (scalar_eval).
#define CT_STMT_SCALAR(name, node) \
  Scalar name; \
  { std::optional<ExprOutcome> name##_ret; const int name##_sc=scalar_eval((node),frame,true,name,name##_ret); \
    if(name##_sc==SC_RETURN){*returned=std::move(name##_ret->value);return Flow::ret;} \
    if(name##_sc==SC_STOP || failed)return Flow::normal; }

  Flow stmt(Stmt *s, CtFrame &frame, Value *returned) {
    if (!tick(s->loc)) return Flow::normal;
    switch (s->kind) {
      case StmtKind::block: {
        size_t mark = frame.locals.size();
        std::vector<Stmt *> deferred;
        Flow flow = Flow::normal;
        for (Stmt *child : s->body) {
          if (failed) break;
          if (child->kind == StmtKind::defer) {
            deferred.push_back(child->then_);
            continue;
          }
          if ((flow = stmt(child, frame, returned)) != Flow::normal) break;
        }
        // Cleanup runs in reverse registration order on every exit.
        for (size_t i = deferred.size(); i-- > 0 && !failed;) {
          Value ignored;
          if(stmt(deferred[i],frame,&ignored)!=Flow::normal && !failed)
            err(deferred[i]->loc,"E0401","control transfer in deferred cleanup");
        }
        frame.locals.resize(mark);
        return flow;
      }
      case StmtKind::var: {
        Value value;
        if (s->expr && scalar_type(s->expr->type)) { CT_STMT_SCALAR(initial,s->expr);value=scalar_value(initial); }
        else if (s->expr) { CT_STMT_EVAL(initial,s->expr);value=std::move(initial); }
        else value = zero(s->type, s->loc);
        frame.locals.push_back({s->name, std::move(value), !s->uninit});
        return Flow::normal;
      }
      case StmtKind::assign: {
        Holder h;
        CtLocal *root = nullptr;
        auto located=this->place(s->target,frame,h,true,&root);
        if(located.returning){*returned=std::move(located.returned_value);return Flow::ret;}
        Value *place=located.value;
        if(!place || failed)return Flow::normal;
        Value value;
        if (scalar_type(s->expr->type)) { CT_STMT_SCALAR(scalar,s->expr);value=scalar_value(scalar); }
        else { CT_STMT_EVAL(computed,s->expr);value=std::move(computed); }
        if (s->op != Tok::equal) {
          if (root && !root->initialized) {
            err(s->loc, "E0405", "compile-time read of an uninitialized variable");
            return Flow::normal;
          }
          Tok op = s->op == Tok::plus_equal ? Tok::plus : s->op == Tok::minus_equal ? Tok::minus
                 : s->op == Tok::star_equal ? Tok::star : s->op == Tok::slash_equal ? Tok::slash
                 : s->op == Tok::percent_equal ? Tok::percent : s->op == Tok::amp_equal ? Tok::amp
                 : s->op == Tok::pipe_equal ? Tok::pipe : s->op == Tok::caret_equal ? Tok::caret
                 : s->op == Tok::shl_equal ? Tok::shl : Tok::shr;
          Type t = place->type;
          value = is_float(t) ? float_op(op, t, place->f, value.f) : int_op(op, t, place->bits, value.bits, value.type, s->loc);
          if (failed) return Flow::normal;
        }
        assign_into(*place, value);
        if (root) root->initialized = true;
        return Flow::normal;
      }
      case StmtKind::expr: {CT_STMT_EVAL(ignored,s->expr);return Flow::normal;}
      case StmtKind::ret:
        if(s->expr){auto value=expression(s->expr,frame,true);*returned=std::move(value.value);}
        return Flow::ret;
      case StmtKind::break_: return Flow::brk;
      case StmtKind::continue_: return Flow::cont;
      case StmtKind::defer: return Flow::normal;  // registered by the enclosing block
      case StmtKind::assert_: {
        CT_STMT_SCALAR(condition,s->expr);
        if (!failed && !condition.bits) err(s->loc, "E0406", "assertion failed");
        return Flow::normal;
      }
      case StmtKind::if_: {
        CT_STMT_SCALAR(condition,s->expr);
        if (failed) return Flow::normal;
        if (condition.bits) return stmt(s->then_, frame, returned);
        return s->else_ ? stmt(s->else_, frame, returned) : Flow::normal;
      }
      case StmtKind::switch_: {
        CT_STMT_EVAL(value,s->expr);
        uint64_t tag=value.bits;
        if(is_sum(value.type)){if(!valid_sum(value,s->loc))return Flow::normal;tag=value.sum_tag;}
        if(value.type->kind==TypeKind::enum_)tag=canon_bits(value.type->elem,tag);
        for(SwitchCase *arm:s->cases) if((value.type->kind==TypeKind::enum_ ? canon_bits(value.type->elem,arm->tag_value) : arm->tag_value)==tag){
          size_t mark=frame.locals.size();
          if(arm->binding){
            if(!is_sum(value.type) || !value.agg || value.agg->size()!=1){err(arm->loc,"E0401","invalid checked case payload");return Flow::normal;}
            frame.locals.push_back({arm->binding->name,clone((*value.agg)[0]),true});
          }
          Flow flow=stmt(arm->body,frame,returned);frame.locals.resize(mark);return flow;
        }
        err(s->loc,"E0401","invalid checked switch tag");return Flow::normal;
      }
      case StmtKind::while_:
        while (!failed) {
          CT_STMT_SCALAR(c,s->expr);
          if (failed || !c.bits) break;
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) return f;
          if (f == Flow::brk) break;
        }
        return Flow::normal;
      case StmtKind::for_range: {
        CT_STMT_SCALAR(lo,s->expr);
        if (failed) return Flow::normal;
        CT_STMT_SCALAR(hi,s->expr2);
        if (failed) return Flow::normal;
        Type t = s->type;
        size_t mark = frame.locals.size();
        frame.locals.push_back({s->name, scalar_value(lo), true});
        for (__int128 i = as_int128(t, lo.bits); i < as_int128(t, hi.bits) && !failed; i++) {
          frame.locals[mark].value = scalar(t, canon_bits(t, (uint64_t)i));
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) { frame.locals.resize(mark); return f; }
          if (f == Flow::brk) break;
        }
        frame.locals.resize(mark);
        return Flow::normal;
      }
      case StmtKind::for_each: {
        Type ct = s->expr->type;
        Holder h;
        std::shared_ptr<CtStorage> storage;
        int64_t offset = 0, count = 0;
        Value collection;
        if (ct->kind == TypeKind::array) {
          // Arrays are iterated in place: writes by the body are seen by later iterations.
          auto located=place(s->expr,frame,h,true);
          if(located.returning){*returned=std::move(located.returned_value);return Flow::ret;}
          Value *a=located.value;
          if(!a || failed)return Flow::normal;
          storage = a->agg;
          count = ct->length;
        } else {
          CT_STMT_EVAL(value,s->expr);collection=std::move(value);
          if (failed) return Flow::normal;
          if (ct == &T_STRING) count = text_count(collection);
          else { storage = collection.agg; offset = collection.offset; count = collection.count; }
        }
        size_t mark = frame.locals.size();
        frame.locals.push_back({s->name, {}, true});
        if (s->name2.count) frame.locals.push_back({s->name2, {}, true});
        for (int64_t i = 0; i < count && !failed; i++) {
          frame.locals[mark].value = ct == &T_STRING ? scalar(&T_U8, text_byte(collection, i))
                                                     : clone((*storage)[(size_t)(offset + i)], true);
          if (s->name2.count) frame.locals[mark + 1].value = scalar(T_INT, (uint64_t)i);
          Flow f = stmt(s->then_, frame, returned);
          if (f == Flow::ret) { frame.locals.resize(mark); return f; }
          if (f == Flow::brk) break;
        }
        frame.locals.resize(mark);
        return Flow::normal;
      }
    }
    return Flow::normal;
  }

  // ---- folding #run sites in procedure bodies ----

  void fold(Expr *e) {
    if (!e || failed) return;
    if (e->kind == ExprKind::run) {
      begin_site(e, "#run" + (fold_owner ? " in '" + to_std(fold_owner->name) + "'" : std::string()), e->loc);
      CtFrame empty;
      expr(e, empty, true);
      return;
    }
    fold(e->lhs);
    fold(e->rhs);
    fold(e->third);
    for (Expr *a : e->args) fold(a);
    for (FieldInit &f : e->fields) fold(f.value);
  }

  void fold_stmt(Stmt *s) {
    if (!s || failed) return;
    fold(s->expr);
    fold(s->expr2);
    fold(s->target);
    for (Stmt *child : s->body) fold_stmt(child);
    fold_stmt(s->then_);
    fold_stmt(s->else_);
    for(SwitchCase *arm:s->cases)fold_stmt(arm->body);
  }

  const ProcDecl *fold_owner = nullptr;  // the procedure whose #run sites are being folded
  bool run() {
    // --timings: each declaration's evaluation seconds and steps (g_decl_costs).
    auto charge = [&](const void *decl, const char *kind, Str name, Loc loc, auto &&work) {
      DeclCost *cost = g_decl_costs.at(file, decl, kind, name, loc);
      const uint64_t before = steps;
      { DeclClock clock{cost ? &cost->eval_seconds : nullptr}; work(); }
      if (cost) cost->eval_steps += steps - before;
    };
    for (StructDecl *d : file->structs) {
      if (failed) break;
      begin_site(d, "field defaults of '" + to_std(d->name) + "'", d->loc);
      charge(d->origin ? (const void *)d->origin : d, "struct", (d->origin ? d->origin : d)->name, (d->origin ? d->origin : d)->loc, [&] { field_defaults(d); });
    }
    for (ConstDecl *c : file->constants) {
      if (failed) break;
      begin_site(c, "constant '" + to_std(c->name) + "'", c->loc);
      if (!is_untyped(c->type)) charge(c, "constant", c->name, c->loc, [&] { constant(c); });  // untyped constants were inlined as literals
    }
    for (Expr *a : file->assertions) {
      if (failed) break;
      begin_site(a, "#assert", a->loc);
      CtFrame empty;
      Value condition = expr(a, empty, false);
      if (!failed && !condition.bits) err(a->loc, "E0406", "compile-time assertion failed");
    }
    for (ProcDecl *p : file->procs) {
      const ProcDecl *o = p->origin ? p->origin : p;
      fold_owner = p;
      charge(o, "procedure", o->name, o->loc, [&] { fold_stmt(p->body); });
    }
    fold_owner = nullptr;
    for (CheckDecl *c : file->checks) fold_stmt(c->proc->body);
    end_site();
    return !failed;
  }

  // How a #forall prints a value in a counterexample.
  static std::string show(const Value &v) {
    Type t = v.type;
    if (t == &T_BOOL) return v.bits ? "true" : "false";
    if (t->kind == TypeKind::enum_) {
      for (const EnumMember &m : t->enum_decl->members)
        if (m.bits == v.bits) return to_std(t->enum_decl->name) + "." + to_std(m.name);
    }
    if (t->is_signed) return std::to_string((long long)(int64_t)v.bits);
    return std::to_string((unsigned long long)v.bits);
  }

  // The Coax finite-domain kernel owns admission, products and traversal.
  // This record owns only resolved type facts and enum storage for borrowed views.
  struct Domain {
    Type type = nullptr;
    coax_t_FD_Domain fact{};
    std::vector<uint64_t> enum_values;
    coax_t_FD_Domain view() {
      auto result = fact;
      result.values = {enum_values.data(), (int64_t)enum_values.size()};
      return result;
    }
    void attach_enum() {
      if (type->kind == TypeKind::enum_)
        for (const EnumMember &member : type->enum_decl->members) enum_values.push_back(member.bits);
    }
  };

  bool domain(const Param &prm, const Binding &b, Domain *out) {
    Type t = prm.type;
    out->type = t;
    Type base = t->kind == TypeKind::enum_ && t->elem ? t->elem : t;
    uint32_t kind = b.lo ? 5 : t == &T_BOOL ? 1 : t->kind == TypeKind::enum_ ? 4 :
                    t->kind == TypeKind::integer ? (t->is_signed ? 2 : 3) : 0;
    int64_t first = 0, last = 0;
    if (b.lo) {
      CtFrame empty;
      Value lo = expr(b.lo, empty, false);
      if (failed) return false;
      Value hi = expr(b.hi, empty, false);
      if (failed) return false;
      first = (int64_t)lo.bits;
      last = (int64_t)hi.bits;
    }
    int64_t members = t->kind == TypeKind::enum_ ? t->enum_decl->members.count : 0;
    auto admitted = FD_domain(kind, base->bits, base->is_signed, first, last, members);
    if (admitted.reason) {
      Loc at = b.lo ? b.lo->loc : prm.loc;
      if (admitted.reason == 2) err(at, "E0379", "#forall requires a nonempty increasing range");
      else if (admitted.reason == 3) err(at, "E0379", "#forall cannot enumerate an empty enum");
      else if (admitted.reason == 4) err(at, "E0400", "#forall range has more than 16777216 values");
      else err(at, "E0379", "#forall cannot enumerate this domain");
      return false;
    }
    out->fact = admitted.domain;
    return true;
  }

  bool domains(CheckDecl *c, std::vector<Domain> *out, uint64_t *total) {
    out->resize((size_t)c->proc->params.count);
    *total = 1;
    for (int32_t i = 0; i < c->proc->params.count && !failed; i++) {
      if (!domain(c->proc->params[i], c->domains[i], &(*out)[(size_t)i])) return false;
      auto product = FD_combine(*total, (*out)[(size_t)i].fact.count);
      if (product.reason) {
        err(c->loc, "E0400", "#forall has more than 16777216 cases");
        return false;
      }
      *total = product.total;
    }
    if (failed) return false;
    for (auto &item : *out) item.attach_enum();
    return true;
  }

  void profile_rows(CheckOutcome *out) {
    if (!profile) return;
    for (auto &[proc, row] : profiled) { row.proc = proc; if (!proc) row.total = steps; out->profile.push_back(row); }
    std::stable_sort(out->profile.begin(), out->profile.end(), [](const StepProfileRow &a, const StepProfileRow &b) { return a.own > b.own; });
  }

  CheckOutcome check(CheckDecl *c) {
    CheckOutcome out;
    ProcDecl *p = c->proc;
    begin_site(c, std::string(c->forall ? "#forall" : "#test") + " '" + to_std(c->name) + "' itself", c->loc);
    std::vector<Domain> bound;
    uint64_t total = 1;
    domains(c, &bound, &total);
    std::vector<coax_t_FD_Domain> facts;
    for (auto &item : bound) facts.push_back(item.view());
    std::vector<uint64_t> words(bound.size());
    auto cursor = FD_begin(total);
    while (!failed && cursor.ready) {
      uint32_t reason = FD_tuple({facts.data(), (int64_t)facts.size()}, cursor.index,
                                 {words.data(), (int64_t)words.size()});
      if (reason) { err(c->loc, "E0400", "invalid finite-domain traversal facts"); break; }
      CtFrame frame;frame.procedure=p;
      for (size_t i = 0; i < bound.size(); i++)
        frame.locals.push_back({p->params[(int32_t)i].name, scalar(bound[i].type, words[i]), true});
      Value ignored;
      stmt(p->body, frame, &ignored);
      out.cases++;
      if (failed) {
        for (size_t i = 0; i < bound.size(); i++)
          out.counterexample += (i ? ", " : "") + to_std(p->params[(int32_t)i].name) + " = " + show(scalar(bound[i].type, words[i]));
        break;
      }
      cursor = FD_advance(cursor);
      if (cursor.reason) err(c->loc, "E0400", "invalid finite-domain cursor");
    }
    out.executed = std::move(executed);
    out.passed = !failed;
    out.steps = steps;
    out.budget = max_steps;
    out.over_budget = failed && fail_code == "E0400" && steps_exceeded;
    profile_rows(&out);
    out.loc = fail_loc;
    out.code = fail_code;
    out.message = fail_message;
    return out;
  }
};

}  // namespace

// E0400 for aggregate storage: which limit, what was asked for, the limit, and
// where a larger one applies (checks have their own, budget.coax).
static std::string storage_limit_message(const CtLimit &l) {
  std::string m = "compile-time aggregate storage limit exceeded: " + std::to_string(l.needed) + " " + l.what +
                  " needed, the limit is " + std::to_string(l.limit);
  const bool slots = std::string(l.what) == "value slots";
  const uint64_t test = slots ? CB_test_slot_limit() : CB_test_container_limit();
  if (l.limit < test) m += " (" + std::to_string(test) + " in coax test)";
  return m;
}

// A failed evaluation's exception as an E0400, with the capacity it exhausted (#12926).
static void limit_failure(Evaluator &evaluator, std::exception_ptr failure, bool check, std::shared_ptr<CapacityRefusal> *capacity) {
  try { std::rethrow_exception(failure); }
  catch (const CtTraversalLimit &) {
    *capacity = std::make_shared<CapacityRefusal>(evaluator.traversal_capacity(check));
    evaluator.err(evaluator.current_loc, "E0400", "compile-time aggregate traversal limit exceeded: " + std::to_string((*capacity)->used) +
                                                    " value and field visits needed, the limit is " + std::to_string((*capacity)->limit));
  }
  catch (const CtStringLimit &) { evaluator.err(evaluator.current_loc, "E0400", "compile-time string storage or work limit exceeded"); }
  catch (const CtLimit &l) {
    *capacity = std::make_shared<CapacityRefusal>(evaluator.storage_capacity(l, check));
    evaluator.err(evaluator.current_loc, "E0400", storage_limit_message(l));
  }
  catch (const std::bad_alloc &) { evaluator.err(evaluator.current_loc, "E0400", "compile-time allocation failed"); }
}

// A check's use of its steps and traversal budgets, for the near-limit report.
static void check_near(Evaluator &evaluator, CheckOutcome *out, bool own) {
  try {
    if (capacity_near(evaluator.steps, evaluator.max_steps))
      out->near.push_back({"compile-time steps", evaluator.steps_capacity(true, own)});
    if (capacity_near(evaluator.traversal_work, evaluator.traversal_limit))
      out->near.push_back({"compile-time traversal work", evaluator.traversal_capacity(true)});
  } catch (const std::bad_alloc &) {}
}

CapacityDetail check_slot_holders(File *file, const std::vector<CtMemory::Holder> &holders) {
  Evaluator evaluator{file};
  CapacityDetail d = evaluator.storage_detail(holders, true);
  d.fix = Evaluator::storage_fix(d, true, true);
  return d;
}

CheckOutcome run_check(File *file, CheckDecl *check, uint64_t max_steps, bool profile) {
  Evaluator evaluator{file};
  evaluator.quiet = true;
  evaluator.profile = profile;
  evaluator.max_steps = max_steps;
  if (check->traversal) evaluator.traversal_limit = check->traversal;
  std::exception_ptr failure;
  try {
    CheckOutcome out = evaluator.check(check);
    if (out.over_budget) out.capacity = std::make_shared<CapacityRefusal>(evaluator.steps_capacity(true, check->steps != 0));
    if (out.passed) check_near(evaluator, &out, check->steps != 0);
    return out;
  } catch (...) { failure = std::current_exception(); }
  CheckOutcome out; out.passed = false;
  limit_failure(evaluator, failure, true, &out.capacity);
  out.loc = evaluator.fail_loc; out.steps = evaluator.steps; out.budget = evaluator.max_steps;
  out.code = evaluator.fail_code; out.message = evaluator.fail_message; evaluator.profile_rows(&out); return out;
}

bool prepare_native_check_domains(File *file, NativeCheckCPlan *plan, uint64_t steps) {
  file->ct_memory->slot_limit = CB_test_slot_limit();
  file->ct_memory->container_limit = CB_test_container_limit();
  for (auto &entry : plan->checks) {
    if (entry.declaration_index < 0 || entry.declaration_index >= file->checks.count ||
        file->checks[(int32_t)entry.declaration_index] != entry.check) {
      error_at(file->src, {}, "E0413", "native check plan does not name the original checked declaration");
      return false;
    }
    CheckDecl *check = file->checks[(int32_t)entry.declaration_index];
    Evaluator evaluator{file};
    evaluator.max_steps = check->steps ? check->steps : steps;
    if (check->traversal) evaluator.traversal_limit = check->traversal;
    std::vector<Evaluator::Domain> domains;
    uint64_t total = 1;
    evaluator.quiet = true;  // printed below, with any capacity it exhausted
    std::exception_ptr failure;
    bool admitted = false;
    try {
      admitted = evaluator.domains(check, &domains, &total);
    } catch (...) { failure = std::current_exception(); }
    if (!failure && !admitted && !evaluator.failed) return false;
    std::shared_ptr<CapacityRefusal> capacity;
    if (failure) limit_failure(evaluator, failure, true, &capacity);
    else if (evaluator.steps_exceeded) capacity = std::make_shared<CapacityRefusal>(evaluator.steps_capacity(true, check->steps != 0));
    if (evaluator.failed) {
      if (capacity) capacity_refusal(*capacity);
      evaluator.print_failure(evaluator.calls);
      return false;
    }
    entry.domains.clear();
    entry.case_count = total;
    for (auto &domain : domains) {
      NativeCheckDomain observed;
      observed.kind = domain.fact.kind;
      observed.type = domain.type;
      observed.first = domain.fact.first;
      observed.count = domain.fact.count;
      observed.values = std::move(domain.enum_values);
      if (domain.type->kind == TypeKind::enum_)
        for (const EnumMember &member : domain.type->enum_decl->members) observed.labels.push_back(member.name);
      entry.domains.push_back(std::move(observed));
    }
  }
  return true;
}

bool evaluate(File *file, uint64_t max_steps, uint64_t *used, uint64_t *traversed) {
  Evaluator evaluator{file};
  evaluator.max_steps = max_steps;
  // The compilation's use of each compile-time limit, for the near-limit
  // report, with the consumers of a use near its limit (#12926).
  struct Uses {
    Evaluator &e;
    ~Uses() {
      try {
        capacity_use("compile-time call depth", "E0403", e.peak_calls, Evaluator::MAX_DEPTH);
        capacity_use("compile-time dependency depth", "E0403", e.peak_dependencies, Evaluator::MAX_DEPTH);
        capacity_use("compile-time steps", "E0400", e.steps, e.max_steps,
                     capacity_near(e.steps, e.max_steps) ? e.steps_capacity(false, false).detail : CapacityDetail{});
        capacity_use("compile-time traversal work", "E0400", e.traversal_work, e.traversal_limit,
                     capacity_near(e.traversal_work, e.traversal_limit) ? e.traversal_capacity(false).detail : CapacityDetail{});
        if (const auto &m = e.file->ct_memory) {
          CapacityDetail slots;
          if (capacity_near(m->peak_slots, m->slot_limit) && m->at_peak_slots == m->peak_slots) {
            slots = e.storage_detail(m->at_peak, true);
            slots.fix = Evaluator::storage_fix(slots, true, false);
          }
          capacity_use("compile-time value slots", "E0400", m->peak_slots, m->slot_limit, slots);
          capacity_use("compile-time arrays and structs", "E0400", m->peak_containers, m->container_limit);
          capacity_use("compile-time string bytes", "E0400", m->string_peak_bytes, CB_string_live_limit());
        }
      } catch (const std::bad_alloc &) {}
    }
  } uses{evaluator};
  std::exception_ptr failure;
  bool ok = false;
  try { ok = evaluator.run(); } catch (...) { failure = std::current_exception(); }
  if (failure) {
    evaluator.quiet = true;  // printed here, with the capacity it exhausted
    std::shared_ptr<CapacityRefusal> capacity;
    limit_failure(evaluator, failure, false, &capacity);
    if (capacity) capacity_refusal(*capacity);
    evaluator.print_failure(evaluator.calls);
    ok = false;
  } else if (evaluator.held_failure) {
    capacity_refusal(evaluator.steps_capacity(false, false));
    evaluator.print_failure(evaluator.held_trace);
  }
  if (used) *used = evaluator.steps;
  if (traversed) *traversed = evaluator.traversal_work;
  return ok && !evaluator.failed;
}

}  // namespace coax

#undef CT_EVAL
#undef CT_STMT_EVAL
