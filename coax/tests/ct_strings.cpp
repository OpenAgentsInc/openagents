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
  // cast(string) borrows the view's bytes: no string storage, and a write
  // through the array shows in the string and in its slices.
  for (int i=0; i<5000; ++i) {
    auto s = evaluator.do_cast(&cast, input);
    assert(memory->string_bytes == start && evaluator.string_work == 0);
    assert(s.agg == input.agg && s.count == 1024 && Evaluator::text_byte(s, 255) == 255);
  }
  input.count = 0;
  { auto empty = evaluator.do_cast(&cast, input);
    assert(Evaluator::text_count(empty) == 0 && memory->string_bytes == start); }
  input.count = 1024;
  auto first = evaluator.do_cast(&cast, input);
  Value slice = first; slice.offset += 1; --slice.count;
  (*input.agg)[1].bits = 42;
  assert(Evaluator::text_byte(first, 1) == 42 && Evaluator::text_byte(slice, 0) == 42);
  // A frozen string outlives the bytes: freezing takes one snapshot, which
  // later writes don't change and which lives until file teardown.
  Expr frozen; evaluator.freeze(&frozen, slice, {});
  (*input.agg)[1].bits = 1;
  first = {}; slice = {};
  assert(memory->string_bytes == start + 1024 && evaluator.string_work == 1024);
  assert(frozen.kind == ExprKind::string_lit && frozen.text.count == 1023 && (unsigned char)frozen.text.data[0] == 42);
  evaluator.string_work = 0;
  // Snapshot admission refuses before allocation and preserves live
  // accounting. Charge actual copying up to the precise work boundary.
  auto borrowed = evaluator.do_cast(&cast, input);
  while (CB_admit(evaluator.string_work, 1025, CB_string_work_limit())) {
    auto temporary = evaluator.snapshot(borrowed);
  }
  const auto remaining = CB_string_work_limit() - evaluator.string_work;
  if (remaining) {
    Value part = borrowed; part.count = (int64_t)remaining - 1;
    auto temporary = evaluator.snapshot(part);
  }
  assert(evaluator.string_work == CB_string_work_limit());
  try { (void)evaluator.snapshot(borrowed); assert(false); }
  catch (const CtStringLimit &) {}
  assert(memory->string_bytes == start + 1024);
  evaluator.string_work = 0;
  fail_next_string_allocation = true;
  try { (void)evaluator.snapshot(borrowed); assert(false); }
  catch (const std::bad_alloc &) {}
  assert(memory->string_bytes == start + 1024 && evaluator.string_work == 0);
  const auto live = memory->string_bytes;
  {
    auto full = std::make_shared<CtString>(memory, CB_string_live_limit() - live);
    assert(memory->string_bytes == CB_string_live_limit());
    try { (void)evaluator.snapshot(borrowed); assert(false); }
    catch (const CtStringLimit &) {}
    assert(memory->string_bytes == CB_string_live_limit());
  }
  assert(memory->string_bytes == live);
  // A frozen record owns snapshots of the strings it holds.
  {
    TypeInfo record; record.kind = TypeKind::array; record.length = 1; record.elem = &T_STRING;
    Value boxed; boxed.type = &record; boxed.agg = evaluator.storage();
    boxed.agg->push_back(evaluator.do_cast(&cast, input));
    (*boxed.agg)[0].count = 3;
    Expr saved; evaluator.freeze(&saved, boxed, {});
    (*input.agg)[0].bits = 'z';
    assert(saved.kind == ExprKind::ct_value && !(*saved.ct->agg)[0].agg && str_eq((*saved.ct->agg)[0].s, Str{"\0\1\2", 3}));
  }
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
