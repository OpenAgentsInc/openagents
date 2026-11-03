// Coax compiler (coax0): shared declarations.
// The language is specified in coax/spec.md.
// Stage 2 slice (§20): sized scalars, structs, enums, fixed arrays, views,
// for/defer/ifx and the C export boundary, transpiled to C11. Syntax nodes
// are plain structs allocated in a compile arena.
#pragma once
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <new>
#include <string>
#include <vector>

namespace coax {
// Storage for the parser's and checker's caller-owned arrays, which are sized
// to a worst-case capacity and mostly left unused. calloc returns zeroed pages
// that the system maps only when they are first written, so capacity that is
// never used costs no resident memory. Elements are plain C records, for which
// all-zero bytes are their value-initialized state, so default construction
// writes nothing. Use it only for vectors that grow from empty and never shrink
// and grow again (ax-library-check-cache).
template <class T> struct ZeroAlloc {
  using value_type = T;
  ZeroAlloc() = default;
  template <class U> ZeroAlloc(const ZeroAlloc<U> &) {}
  T *allocate(size_t n) {
    void *p = calloc(n ? n : 1, sizeof(T));
    if (!p) throw std::bad_alloc();
    return (T *)p;
  }
  void deallocate(T *p, size_t) { free(p); }
  template <class U> void construct(U *) {}
  template <class U, class A, class... More> void construct(U *p, A &&a, More &&...more) {
    ::new ((void *)p) U(static_cast<A &&>(a), static_cast<More &&>(more)...);
  }
  template <class U> bool operator==(const ZeroAlloc<U> &) const { return true; }
  template <class U> bool operator!=(const ZeroAlloc<U> &) const { return false; }
};
template <class T> using zvec = std::vector<T, ZeroAlloc<T>>;
}  // namespace coax

extern "C" uint64_t CB_default_steps();
extern "C" uint64_t CB_slot_limit();
extern "C" uint64_t CB_container_limit();
extern "C" uint64_t CB_test_slot_limit();
extern "C" uint64_t CB_test_container_limit();

namespace coax {

// ---- Memory: one arena for the whole compile; nothing is freed. ----

struct Arena {
  uint8_t *block = nullptr;
  size_t used = 0, cap = 0;
};

void *arena_alloc(Arena *a, size_t size);

template <class T> T *make(Arena *a) {
  T *p = (T *)arena_alloc(a, sizeof(T));
  new (p) T{};
  return p;
}

template <class T> struct Slice {
  T *data = nullptr;
  int32_t count = 0;
  T &operator[](int32_t i) const { return data[i]; }
  T *begin() const { return data; }
  T *end() const { return data + count; }
};

template <class T> Slice<T> to_slice(Arena *a, const std::vector<T> &v) {
  Slice<T> s;
  s.count = (int32_t)v.size();
  if (s.count) {
    s.data = (T *)arena_alloc(a, sizeof(T) * v.size());
    memcpy((void *)s.data, (const void *)v.data(), sizeof(T) * v.size());
  }
  return s;
}

struct Str {  // a view into source text or the arena; not NUL-terminated
  const char *data = nullptr;
  int32_t count = 0;
};

inline bool str_eq(Str a, const char *b) {
  size_t n = strlen(b);
  return (size_t)a.count == n && memcmp(a.data, b, n) == 0;
}
inline bool str_eq(Str a, Str b) {
  return a.count == b.count && (a.count == 0 || memcmp(a.data, b.data, (size_t)a.count) == 0);
}
inline std::string to_std(Str s) { return std::string(s.data, (size_t)s.count); }

// ---- Source and diagnostics ----

struct Loc {
  int32_t line = 0, col = 0, offset = 0;
};

struct GeneratedRegion {
  int32_t start = 0, end = 0;
  Loc directive;
  std::string prefix;
};
struct SourceUnit {
  std::string path, canonical, text;
  int32_t begin = 0, end = 0, first_line = 1;
  bool authority = false;
  // Logical module (schema 1): `claims` when this unit declares #module;
  // `module` is its owner's ID, or empty when no owner is available (legacy).
  bool claims = false;
  std::string module;
  std::vector<int32_t> loads; // indices in Source::units this unit #loads, in written order
  // `#load "FILE.coax" as NAME;`: the alias, the loaded unit, the alias offset in this unit.
  struct Alias { std::string name; int32_t target = 0; int32_t offset = 0; };
  std::vector<Alias> aliases;
  // `#export a, b;`: each name and its location within this unit.
  struct Export { std::string name; Loc loc; };
  std::vector<Export> exports;
};
struct SourcePoint { std::string path; Loc loc; };
struct Source {
  std::string path;
  std::string text;
  std::string original_text;
  std::vector<GeneratedRegion> generated;
  std::vector<std::string> import_provenance;
  std::vector<std::shared_ptr<SourceUnit>> units;
  std::vector<std::string> descriptor_inputs;
};
Loc diagnostic_loc(const Source *src, Loc loc);
SourcePoint source_point(const Source *src, Loc loc);
bool source_authority(const Source *src, Loc loc);
const SourceUnit *source_unit_at(const Source *src, Loc loc);
// The nominal declaration ID "coax.decl.v1:MODULE:KIND:NAME" (kind 0 proc,
// 1 struct, 2 enum, 3 const), or "" when the declaring unit has no owner.
std::string declaration_id(const Source *src, uint8_t kind, Str name, Loc loc);

// Prints "path:line:col: error[Exxxx]: message", the source line, a caret,
// and "fix: ..." when there is one; or one JSON object per line with
// --json. Counts errors.
void error_at(const Source *src, Loc loc, const char *code, const std::string &message,
              const std::string &fix = "");
extern int32_t g_error_count;
extern bool g_json_errors;
// --diagnostics json: one coax.diagnostic.v1 object per line on stderr, with
// spans, notes and fixes (spec 19.4). The text format is unchanged without it.
extern bool g_diagnostics_json;
// A note for the next error_at's JSON diagnostic: where (when has_loc) and what.
void diagnostic_note(const Source *src, Loc loc, bool has_loc, const std::string &message);

// Explanations for every error code, shared by `coax explain` and the book.
struct ErrorInfo {
  const char *code;
  const char *title;
  const char *explanation;
};
extern const ErrorInfo ERRORS[];
extern const int32_t ERROR_COUNT;

// ---- Lexer ----

enum class Tok : uint8_t {
  eof, ident, keyword, int_lit, float_lit, string_lit, directive, annotation,
  lparen, rparen, lbrace, rbrace, lbracket, rbracket,
  comma, semicolon, colon, coloncolon, colonequal, arrow, dot, dotdot, dotdotlt,
  equal, plus_equal, minus_equal, star_equal, slash_equal, percent_equal,
  amp_equal, pipe_equal, caret_equal, shl_equal, shr_equal,
  plus, minus, star, slash, percent, amp, pipe, caret, shl, shr,
  eqeq, noteq, lt, le, gt, ge, andand, oror, bang, dollar, question,
};

struct Token {
  Tok kind = Tok::eof;
  Loc loc;
  Str text;            // identifier, keyword, directive name (without '#'), or raw literal
  uint64_t int_value = 0;
  double float_value = 0;
  uint8_t float_bits = 0;  // float_lit: 32 or 64 for a 1.5f32 / 1.5f64 suffix, 0 for untyped
  Str string_value;    // string literal after escapes
};

bool lex(Arena *arena, const Source *src, std::vector<Token> *out);
const char *tok_name(Tok t);

// ---- Types ----
// Types are interned: two types are the same type exactly when their
// TypeInfo pointers are equal.

enum class TypeKind : uint8_t {
  invalid, void_, integer, floating, bool_, string_, untyped_int, untyped_float,
  struct_, enum_, array, view,
  param,  // a type parameter's placeholder ($T): checked, never laid out or emitted
  union_, result, optional,
};

struct StructDecl;
struct EnumDecl;
struct UnionDecl;

struct TypeInfo {
  TypeKind kind = TypeKind::invalid;
  std::string name;          // as Coax spells it: "s32", "Vec2", "[4] int", "[] u8"
  int32_t size = 0, align = 1;  // host C layout (LP64)
  int32_t bits = 0;          // integer and floating widths
  bool is_signed = false;
  const TypeInfo *elem = nullptr;  // array, view; enum: the underlying integer type
  int32_t scope = 0;         // struct/enum declared under one name by two modules: its C spelling's module scope
  int64_t length = 0;        // array
  StructDecl *struct_decl = nullptr;
  EnumDecl *enum_decl = nullptr;
  const TypeInfo *physical = nullptr; // compiler-selected transparent quantity carrier
  int32_t id = 0;            // generated C names for arrays and views
  UnionDecl *union_decl = nullptr;
  const TypeInfo *sum_error = nullptr;
  int32_t sum_payload_offset = 0;
  int32_t sum_default = -1;  // checked default tag; -1 means no implicit construction
};
using Type = const TypeInfo *;

extern TypeInfo T_INVALID, T_VOID, T_BOOL, T_STRING, T_UNTYPED_INT, T_UNTYPED_FLOAT, T_F32, T_F64;
extern TypeInfo T_S8, T_S16, T_S32, T_S64, T_U8, T_U16, T_U32, T_U64;
#define T_INT (&T_S64)  // int is s64: one type, spelled "int"

const char *type_name(Type t);
inline bool is_int(Type t) { return t->kind == TypeKind::integer; }
inline bool is_float(Type t) { return t->kind == TypeKind::floating; }
inline bool is_number(Type t) { return is_int(t) || is_float(t); }
inline bool is_untyped(Type t) { return t->kind == TypeKind::untyped_int || t->kind == TypeKind::untyped_float; }
inline bool is_aggregate(Type t) { return t->kind == TypeKind::struct_ || t->kind == TypeKind::array; }
inline bool is_sum(Type t) { return t->kind == TypeKind::union_ || t->kind == TypeKind::result || t->kind == TypeKind::optional; }
const std::vector<Type> &all_composite_types();  // arrays and views, in creation order

// ---- Compile-time values ----
// A value holds scalars, byte strings, and aggregates (struct fields or array
// elements). Views share their storage with the aggregate they borrow. A
// value never holds a host address, so nothing of the compiler's memory can
// reach a target executable.

// Which aggregate limit a program passed: what it asked for and the limit.
struct CtLimit { const char *what = "value slots"; uint64_t needed = 0, limit = 0; };
struct CtStringLimit {};
struct CtTraversalLimit {};
struct CtStorage;
struct CtString;
struct CtMemory {
  // Counts vector capacity slots and aggregate containers, not total RSS.
  // A compile's limits (budget.coax); coax test and mutate raise them to the test limits.
  uint64_t slot_limit = CB_slot_limit(), container_limit = CB_container_limit();
  static uint64_t max_slots() { return CB_slot_limit(); }
  static uint64_t max_containers() { return CB_container_limit(); }
  uint64_t slots = 0, containers = 0, peak_slots = 0, peak_containers = 0;
  uint64_t string_bytes = 0, string_peak_bytes = 0;
  // Every live aggregate storage, newest first, linked through the storage
  // itself (so a short-lived array costs no registry entry), for clear_cycles.
  CtStorage *live = nullptr;
  uint64_t live_count = 0;
  CtStorage *pending = nullptr;
  bool draining = false;
  void dispose(CtStorage *storage) noexcept;
  void remember(const std::shared_ptr<CtStorage> &storage);
  void forget(CtStorage *storage) noexcept;
  void clear_cycles() noexcept;
  void take_slots(size_t n);
  void release_slots(size_t n) noexcept { slots -= n; }
  void take_container();
  void take_string(size_t n);
  void release_string(size_t n) noexcept { string_bytes -= n; }
  void release_container() noexcept { --containers; }
};
template<class T> struct CtAllocator {
  using value_type = T;
  std::shared_ptr<CtMemory> memory;
  CtAllocator() = default;
  explicit CtAllocator(std::shared_ptr<CtMemory> m) : memory(std::move(m)) {}
  template<class U> CtAllocator(const CtAllocator<U> &other) : memory(other.memory) {}
  T *allocate(size_t n) {
    memory->take_slots(n);
    try { return std::allocator<T>{}.allocate(n); }
    catch (...) { memory->release_slots(n); throw; }
  }
  void deallocate(T *p, size_t n) noexcept {
    std::allocator<T>{}.deallocate(p,n);
    memory->release_slots(n);
  }
  template<class U> bool operator==(const CtAllocator<U> &o) const noexcept { return memory == o.memory; }
  template<class U> bool operator!=(const CtAllocator<U> &o) const noexcept { return !(*this == o); }
};
struct CtString {
  std::shared_ptr<CtMemory> owner;
  std::unique_ptr<char[]> bytes;
  size_t size;
  CtString(std::shared_ptr<CtMemory> memory, size_t n);
  ~CtString();
};
struct Value;
struct CtStorage : std::vector<Value, CtAllocator<Value>>, std::enable_shared_from_this<CtStorage> {
  std::shared_ptr<CtMemory> owner;
  CtStorage *pending_next = nullptr;
  CtStorage *live_prev = nullptr, *live_next = nullptr;  // CtMemory::live
  bool linked = false;
  explicit CtStorage(std::shared_ptr<CtMemory> memory);
  ~CtStorage();
};

struct Value {
  Type type = &T_VOID;
  uint64_t bits = 0;  // integers (two's complement bit pattern), bool, enum
  double f = 0;       // f32 values are stored rounded to float
  Str s;
  std::shared_ptr<CtString> string_owner;  // owned cast snapshot, shared by slices
  std::shared_ptr<CtStorage> agg;  // struct fields, array elements, view storage
  int64_t offset = 0, count = 0;            // view: window into agg
  uint32_t sum_tag = 0;  // sum: agg contains only the active payload (zero slots for unit)
};

// ---- Syntax tree ----

struct Expr;
struct TypeExpr;
struct TypeArgument {
  TypeExpr *type = nullptr;
  Expr *length = nullptr;
};

struct TypeExpr {
  enum Kind : uint8_t { named, array, view, dynamic, param, optional } kind = named;
  Loc loc;
  Str name;                  // named
  Expr *length = nullptr;    // array
  TypeExpr *elem = nullptr;  // array, view, dynamic
  Slice<TypeArgument> args;
};

enum class ExprKind : uint8_t {
  int_lit, float_lit, bool_lit, string_lit, ident, call, unary, binary, run,
  field, index, slice, struct_lit, array_lit, cast, ifx, enum_value, ct_value,
  sum, null_, some, try_, or_else,
};

// Built-in math, the same in C, Metal and compile-time evaluation.
enum class Intrinsic : uint8_t { none, sin, cos, tan, exp, log, sqrt, floor, abs, pow, fma };
Intrinsic find_intrinsic(Str name);

enum class ByteIntrinsic : uint8_t { none, bytes_overlap, f32_from_bits, f64_from_bits, f32_bits, f64_bits, value_barrier, add_wrap, sub_wrap, mul_wrap, rotl, rotr, mul_wide };
ByteIntrinsic find_byte_intrinsic(Str name);

struct FieldInit {
  Str name;
  Loc loc;
  Expr *value = nullptr;
  int32_t field = -1;  // resolved field index
};

struct Expr {
  ExprKind kind;
  uint8_t effects = 0;  // CK_effects: 1 call, 2 trap, 4 overflow, 8 current-frame propagation
  uint8_t own_effects = 0;  // the same bits for this operation alone, without its operands
  Loc loc;
  Type type = &T_INVALID;     // set by the checker
  uint64_t int_value = 0;     // int_lit (bit pattern), bool_lit, enum_value
  double float_value = 0;     // float_lit
  Str text;                   // ident name, string literal value, call name, field name
  Tok op = Tok::eof;          // unary, binary
  Expr *lhs = nullptr, *rhs = nullptr, *third = nullptr;
  // unary/run/cast/field: lhs. binary: lhs, rhs. index: lhs[rhs].
  // slice: lhs[rhs ..< third] (rhs/third null for `[..]`). ifx: lhs then rhs else third.
  Slice<Expr *> args;         // call, array_lit
  Slice<FieldInit> fields;    // struct_lit
  TypeExpr *type_expr = nullptr;  // cast, struct_lit, array_lit
  bool trunc = false;         // cast,trunc
  bool negative = false;      // untyped int_lit: int_value holds a negative s64
  bool compiler_generated = false; // checked Coax provenance; never an authored mutation site
  bool folded = false;        // untyped int_lit computed from an operation, not written
  int32_t field_index = -1;   // field
  struct ProcDecl *proc = nullptr;      // call: resolved target (null for builtins)
  uint8_t lowering = 0, mutation_status = 0; // checked lowering origin and preparation status
  Tok mutation_op = Tok::eof;
  struct ProcDecl *mutation_proc = nullptr; // site-local admitted alternate callee
  struct ConstDecl *constant = nullptr; // ident: resolved global constant
  bool builtin_print = false;
  ByteIntrinsic byte_intrinsic = ByteIntrinsic::none;
  Intrinsic intrinsic = Intrinsic::none;  // call: built-in math (proc is null)
  Value *ct = nullptr;        // ct_value: an aggregate computed at compile time
  int32_t ct_id = -1;         // ct_value: emitted as a static constant
  int32_t sum_variant = -1;
  Type propagate_type = nullptr; // exact admitted enclosing return type on try
};

struct SwitchCase;
enum class StmtKind : uint8_t { block, expr, var, assign, ret, if_, while_, for_range, for_each, break_, continue_, defer, assert_, switch_ };

struct Stmt {
  StmtKind kind;
  Loc loc;
  Slice<Stmt *> body;          // block
  Expr *expr = nullptr;        // expr, var init, assign value, return value, condition,
                               // for_range lower bound, for_each collection
  Expr *expr2 = nullptr;       // for_range upper bound
  Expr *target = nullptr;      // assign: the place
  Str name, name2;             // var; for: element/counter and optional index
  Loc name2_loc;
  TypeExpr *type_expr = nullptr;  // var: declared type, null when inferred
  Type type = &T_INVALID;      // var/for: resolved type of `name`
  bool uninit = false;         // var: `= ---`
  Tok op = Tok::equal;         // assign: = += -= ...
  uint8_t own_effects = 0;     // assign: the compound update's CK_effects bits
  Stmt *then_ = nullptr, *else_ = nullptr;  // if (then_ is a block), loops and defer (then_)
  Slice<SwitchCase *> cases;
};

struct Param {
  Str name;
  Loc loc;
  TypeExpr *type_expr = nullptr;
  Type type = &T_INVALID;
};

struct CaseBinding {
  Str name;
  Loc loc;
  Type type = &T_INVALID;
};
struct SwitchCase {
  Loc loc;
  Str name;
  Expr *enum_pattern = nullptr;
  CaseBinding *binding = nullptr;
  Stmt *body = nullptr;
  uint64_t tag_value = 0;
};
struct EffectName { Str name; Loc loc; };

struct ProcDecl {
  uint64_t ct_executed_mark = 0;  // the evaluator that last recorded entering it (Evaluator::executed)
  Str name;
  int32_t scope = 0;  // a name two modules declare: its C spelling's module scope (0: the plain name)
  Loc loc;
  int32_t end = 0;  // offset just past the item's last authored token (0: unknown)
  Slice<Param> params;
  TypeExpr *ret_expr = nullptr;
  Type ret = &T_VOID;
  Stmt *body = nullptr;
  bool root_source = true;
  bool authority = false; // obligation belongs to the authored source unit
  bool export_c = false;
  bool foreign = false;  // #foreign: a C symbol the host provides; no body
  bool generic = false;  // has $T type parameters: checked; only its instances are emitted
  Str key;  // an instance: its bindings, length-prefixed, appended to the template's C name
  const ProcDecl *origin = nullptr;  // an instance: the generic procedure it was made from
  bool effect_present = false, pure_decl = false;
  Loc effect_loc;
  Slice<EffectName> effect_names;
};

struct Field {
  Str name;
  Loc loc;
  TypeExpr *type_expr = nullptr;
  Type type = &T_INVALID;
  Expr *init = nullptr;  // a constant default, frozen to a literal by evaluation
  int32_t offset = 0;
  bool flat = false;     // #flat: a shader varying taken from the provoking vertex, not interpolated
  bool depth = false;    // #depth: a fragment output that is its depth, not a color target
};

struct StructDecl {
  Str name;
  Loc loc;
  int32_t end = 0;  // offset just past the item's last authored token (0: unknown)
  Slice<Field> fields;
  TypeInfo *type = nullptr;
  uint8_t state = 0;     // layout: 0 unseen, 1 visiting, 2 done
  uint8_t defaults = 0;  // default evaluation: 0 unseen, 1 visiting, 2 done
  bool generic = false;  // a template (Pair :: struct($T)) or an instance over placeholders
  Str key;               // an instance's bindings, for its C name (Pair__3int)
  const StructDecl *origin = nullptr;  // an instance's template
};

struct EnumMember {
  Str name;
  Loc loc;
  Expr *value = nullptr;  // optional; otherwise previous + 1
  uint64_t bits = 0;
};

struct EnumDecl {
  Str name;
  Loc loc;
  int32_t end = 0;  // offset just past the item's last authored token (0: unknown)
  TypeExpr *base_expr = nullptr;
  Slice<EnumMember> members;
  TypeInfo *type = nullptr;
};

struct UnionVariant {
  Str name;
  Loc loc;
  TypeExpr *type_expr = nullptr;
  Type type = &T_INVALID;
  uint32_t ordinal = 0;
};
struct UnionDecl {
  Str name;
  Loc loc;
  int32_t end = 0;
  Slice<UnionVariant> variants;
  TypeInfo *type = nullptr;
};

struct Import {
  Str module;
  Loc loc;
};

struct ConstDecl {
  Str name;
  Loc loc;
  int32_t end = 0;  // offset just past the item's last authored token (0: unknown)
  Expr *expr = nullptr;  // replaced with a literal after compile-time evaluation
  Type type = &T_INVALID;
  uint8_t checked = 0, evaluated = 0; // 0 unseen, 1 visiting, 2 done
};

// #test "name" { ... } and #forall a: u8, b: 0..<10 { ... }: checks the
// evaluator runs (coax test), excluded from ordinary product emission. A check
// is a procedure with no name in the program's namespace; a #forall's bindings
// are its parameters, enumerated over their whole domain.
struct Binding {
  Expr *lo = nullptr, *hi = nullptr;  // a lo..<hi range of int; null for a type domain
};

struct CheckDecl {
  Str name;               // the #test's string, or the bindings' text for an unnamed #forall
  Loc loc;
  bool forall = false;
  Slice<Binding> domains;  // one per parameter of proc
  ProcDecl *proc = nullptr;
  uint64_t steps = 0;      // #steps N: this check's own step budget (0: the file's, coax test --steps)
};

struct CheckInput;
struct File {
  ~File();
  std::shared_ptr<CtMemory> ct_memory = std::make_shared<CtMemory>();
  std::vector<std::unique_ptr<Value>> frozen_values;
  std::shared_ptr<CheckInput> check_input;
  std::string execution_profile; // resolved semantic selection binds mutation receipts
  const Source *src = nullptr;
  Slice<Import> imports;
  Slice<ProcDecl *> procs;     // procedures with instances: every emitter and evaluator walks these
  Slice<StructDecl *> generic_structs;  // struct templates (declaration spans only)
  Slice<ProcDecl *> generics;  // checked generic procedures (declaration spans only, nothing emitted)
  Slice<ConstDecl *> constants;
  Slice<StructDecl *> structs;
  Slice<EnumDecl *> enums;
  Slice<UnionDecl *> unions;
  Slice<Expr *> assertions;
  Slice<CheckDecl *> checks;
  bool authority = false;  // at least one authored unit requires checked execution
  uint64_t run_steps = 0;  // the entry file's #run_steps N; (0: none declared), then the budget used
  bool library = false;  // set by the driver: no C main, main optional
  bool metal = false;    // checked against the Metal scalar profile
  bool quantity_device = false; // observed device-output profile; Coax owns quantity admission
  bool cuda = false;     // checked against the CUDA compute subset
};

bool parse(Arena *arena, const Source *src, const std::vector<Token> &tokens, File *out, bool development = true);

// ---- Checker, evaluator and C emitter ----

bool check(Arena *arena, File *file, bool mutation_lowering = false);
// Bounded, side-effect-free AST evaluation; no host I/O or foreign calls.
bool evaluate(File *file, uint64_t max_steps, uint64_t *used = nullptr, uint64_t *traversed = nullptr);
// Whether an expression contains a procedure call (whose effects order can observe).
bool has_call(const Expr *e);

std::string json_string(const std::string &s);
std::string emit_debug_map(const File *file);

struct EmitOptions {
  uint64_t run_steps = CB_default_steps(); // shared #run and constant evaluation budget
  bool run_steps_given = false; // --run-steps given: it overrides the entry file's #run_steps
  bool debug_info = false; // opt-in original-source native line tables
  bool wrapping = false; // runtime add/subtract/multiply/negation; evaluator stays checked
  bool development = true; // BUILD_DEBUG source/layout selection only
  int optimization = 0; // native C compiler level, independent of arithmetic/layout
  int profile_identity = -1;
  bool library = false;  // no C main; #export_c procedures are the interface
  bool metal = false;    // Metal Shading Language instead of C
  bool cuda = false;     // CUDA C for NVRTC: the compute subset (lib/compiler/compute.coax)
  bool quantity_device = false; // device output is outside initial quantity qualification
  bool metal_compute = false;  // Metal Shading Language compute kernels: the same subset, f64 as float-float
};
std::string emit_c(const File *file, const EmitOptions &opts);
// Verification emission borrows the original checked graph. Its procedure
// and check lists are a separate admitted plan, never a copied or rewritten File.
struct NativeCheckDomain {
  uint32_t kind = 0; // 1 bool, 2 signed, 3 unsigned, 4 enum, 5 int range
  Type type = nullptr;
  int64_t first = 0;
  uint64_t count = 0;
  std::vector<uint64_t> values; // canonical enum members, in declaration order
  std::vector<Str> labels; // borrowed member names, same order as values
};
struct NativeCheckCEntry {
  int64_t declaration_index = -1;
  const CheckDecl *check = nullptr;
  std::vector<NativeCheckDomain> domains;
  uint64_t case_count = 1;
};
struct NativeCheckCPlan {
  std::vector<const ProcDecl *> procedures;
  std::vector<NativeCheckCEntry> checks;
  uint32_t verification_abi = 1;
};
bool plan_native_checks_c(const File *file, const EmitOptions &opts, const char *only,
                          uint32_t verification_abi, NativeCheckCPlan *plan);
// Mutable evaluation prepares checked range facts before the const emitter.
bool prepare_native_check_domains(File *file, NativeCheckCPlan *plan, uint64_t steps);
std::string emit_native_checks_c_plan(const File *file, const EmitOptions &opts, const NativeCheckCPlan &plan);
// On refusal, emits original-source diagnostics and leaves text unchanged.
// only is null for the complete check selection; authority admission belongs
// to the driver's normal, unfiltered front_end before this entry point.
bool emit_native_checks_c(const File *file, const EmitOptions &opts, const char *only, std::string *text);
extern size_t g_instance_c_bytes;  // C text of generic instances in the last emit_c, for --timings
// A C header for the file's #export_c procedures and the types they use,
// with layout assertions the C compiler checks.
std::string emit_header(const File *file);
// A human-readable layout report for every struct.
std::string layout_report(const File *file);

// ---- Checks: coax test and coax mutate ----

// --profile steps: where a check's steps go. A procedure's own steps are
// those charged while it is the innermost call; its total adds the calls it
// makes (counted once when it recurses). A null procedure is the check's body.
struct StepProfileRow {
  const ProcDecl *proc = nullptr;
  uint64_t own = 0, total = 0, calls = 0;
};
struct CheckOutcome {
  bool passed = false;
  uint64_t cases = 0, steps = 0;
  uint64_t budget = 0;         // the step budget the check ran under
  bool over_budget = false;    // it stopped because it ran out of steps
  std::vector<StepProfileRow> profile;  // with profiling: every procedure, most own steps first
  Loc loc;                     // the failure
  std::string code, message;   // the failure's error code and message
  std::string counterexample;  // #forall: "a = 3, b = true"
  std::vector<const ProcDecl *> executed; // procedures actually entered by this check
};
// Runs one check in the evaluator with a budget of max_steps (the caller picks
// the check's own #steps, the file's budget or a mutant's). Never prints.
CheckOutcome run_check(File *file, CheckDecl *check, uint64_t max_steps, bool profile = false);
// The source line and a caret under loc, as diagnostics print them.
void show_source(FILE *out, const Source *src, Loc loc);

}  // namespace coax
