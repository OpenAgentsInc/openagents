// Exercise real compile-time string ownership and accounting under sanitizers.
#define main coax_driver_main
#include "../src/main.cpp"
#undef main
#include <cassert>
static bool fail_next_string_allocation = false;
void *operator new[](size_t n) {
  if (fail_next_string_allocation) { fail_next_string_allocation = false; throw std::bad_alloc(); }
  return ::operator new(n);
}
void operator delete[](void *p) noexcept { ::operator delete(p); }
void operator delete[](void *p, size_t) noexcept { ::operator delete(p); }

int main() {
  using namespace coax;
  File file;
  Evaluator evaluator{&file};
  auto memory = file.ct_memory;
  TypeInfo view_type; view_type.kind = TypeKind::view;
  Value input; input.type = &view_type; input.agg = evaluator.storage();
  input.agg->resize(1024); input.count = 1024;
  for (size_t i=0; i<1024; ++i) (*input.agg)[i].bits = i & 255;
  Expr cast; cast.type = &T_STRING;
  const auto start = memory->string_bytes;
  for (int i=0; i<5000; ++i) {
    auto s = evaluator.do_cast(&cast, input);
    assert(memory->string_bytes == start + 1025);
    assert((unsigned char)s.s.data[255] == 255);
    auto alias = s;
    s = {};
    assert(memory->string_bytes == start + 1025);
    alias = {};
    assert(memory->string_bytes == start);
  }
  assert(memory->string_peak_bytes >= 1025);
  input.count = 0;
  { auto empty = evaluator.do_cast(&cast, input);
    assert(empty.s.count == 0 && empty.s.data[0] == 0 && memory->string_bytes == start + 1); }
  assert(memory->string_bytes == start);
  input.count = 1024;
  // A frozen scalar and a sliced alias retain the snapshot until file teardown.
  auto first = evaluator.do_cast(&cast, input);
  Value slice = first; slice.s.data += 1; --slice.s.count;
  (*input.agg)[1].bits = 42;
  assert((unsigned char)slice.s.data[0] == 1);
  Expr frozen; evaluator.freeze(&frozen, slice, {});
  first = {}; slice = {};
  assert(memory->string_bytes == start + 1025);
  assert((unsigned char)frozen.text.data[0] == 1);
  // Admission refusals occur before allocation and preserve live accounting.
  // Charge actual copying up to the precise work boundary, without carrying
  // temporary strings over iterations.
  while (CB_admit(evaluator.string_work, 1025, CB_string_work_limit())) {
    auto temporary = evaluator.do_cast(&cast, input);
  }
  const auto remaining = CB_string_work_limit() - evaluator.string_work;
  if (remaining) {
    input.count = (int64_t)remaining - 1;
    auto temporary = evaluator.do_cast(&cast, input);
  }
  assert(evaluator.string_work == CB_string_work_limit());
  input.count = 1024;
  try { (void)evaluator.do_cast(&cast, input); assert(false); }
  catch (const CtStringLimit &) {}
  assert(memory->string_bytes == start + 1025);
  evaluator.string_work = 0;
  fail_next_string_allocation = true;
  try { (void)evaluator.do_cast(&cast, input); assert(false); }
  catch (const std::bad_alloc &) {}
  assert(memory->string_bytes == start + 1025 && evaluator.string_work == 0);
  const auto live = memory->string_bytes;
  {
    auto full = std::make_shared<CtString>(memory, CB_string_live_limit() - live);
    assert(memory->string_bytes == CB_string_live_limit());
    try { (void)evaluator.do_cast(&cast, input); assert(false); }
    catch (const CtStringLimit &) {}
    assert(memory->string_bytes == CB_string_live_limit());
  }
  assert(memory->string_bytes == live);
  // A failed freeze must not partially rewrite its source expression.
  {
    TypeInfo array; array.kind = TypeKind::array;
    Value aggregate; aggregate.type = &array; aggregate.agg = evaluator.storage();
    aggregate.agg->resize(1);
    auto full = evaluator.storage();
    full->reserve(CtMemory::max_slots() - memory->slots);
    Expr original; original.kind = ExprKind::binary; original.type = T_INT; original.lhs = &cast;
    try { evaluator.freeze(&original, aggregate, {}); assert(false); }
    catch (const CtLimit &) {}
    assert(original.kind == ExprKind::binary && original.type == T_INT && original.lhs == &cast);
    Expr init; init.kind = ExprKind::ct_value; init.type = &array; init.ct = &aggregate;
    Field field; field.init = &init; StructDecl decl; decl.fields = {&field, 1};
    try { evaluator.field_defaults(&decl); assert(false); }
    catch (const CtLimit &) {}
    assert(decl.defaults == 0 && init.kind == ExprKind::ct_value && init.ct == &aggregate);
  }
  // Frozen scalar owners remain live during emission and are freed with File.
  std::shared_ptr<CtMemory> retained;
  {
    File isolated; retained = isolated.ct_memory;
    Evaluator inner{&isolated};
    Value scalar; scalar.type = &T_STRING;
    scalar.string_owner = std::make_shared<CtString>(retained, 4);
    scalar.string_owner->bytes[0] = 'a'; scalar.string_owner->bytes[1] = 'b';
    scalar.string_owner->bytes[2] = 'c'; scalar.string_owner->bytes[3] = 0;
    scalar.s = {scalar.string_owner->bytes.get(), 3};
    Expr saved; inner.freeze(&saved, scalar, {});
    scalar = {};
    assert(retained->string_bytes == 4 && str_eq(saved.text, "abc"));
  }
  assert(retained->string_bytes == 0);
  return 0;
}
