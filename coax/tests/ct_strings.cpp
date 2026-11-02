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
  // The same ownership rule reaches only the active payload through sums.
  // These are the checked LP64 layouts: tag u32, aligned payload at8.
  // Source cannot return a local borrowed string past A1; exercise the real
  // owner directly rather than weakening that source admission rule.
  {
    TypeInfo optional_text; optional_text.kind = TypeKind::optional;
    optional_text.name = "string?"; optional_text.elem = &T_STRING;
    optional_text.size = 24; optional_text.align = 8;
    optional_text.sum_payload_offset = 8; optional_text.sum_default = 0;
    UnionVariant text_variants[2];
    text_variants[0].name = {"empty", 5}; text_variants[0].type = &T_VOID; text_variants[0].ordinal = 0;
    text_variants[1].name = {"text", 4}; text_variants[1].type = &T_STRING; text_variants[1].ordinal = 1;
    UnionDecl text_decl; text_decl.name = {"TextChoice", 10}; text_decl.variants = {text_variants, 2};
    TypeInfo text_choice; text_choice.kind = TypeKind::union_; text_choice.name = "TextChoice";
    text_choice.union_decl = &text_decl; text_decl.type = &text_choice;
    text_choice.size = 24; text_choice.align = 8; text_choice.sum_payload_offset = 8;
    TypeInfo optional_choice; optional_choice.kind = TypeKind::optional;
    optional_choice.name = "TextChoice?"; optional_choice.elem = &text_choice;
    optional_choice.size = 32; optional_choice.align = 8;
    optional_choice.sum_payload_offset = 8; optional_choice.sum_default = 0;
    UnionVariant optional_variants[2];
    optional_variants[0].name = {"empty", 5}; optional_variants[0].type = &T_VOID; optional_variants[0].ordinal = 0;
    optional_variants[1].name = {"text", 4}; optional_variants[1].type = &optional_text; optional_variants[1].ordinal = 1;
    UnionDecl optional_decl; optional_decl.name = {"OptionalChoice", 14}; optional_decl.variants = {optional_variants, 2};
    TypeInfo optional_union; optional_union.kind = TypeKind::union_; optional_union.name = "OptionalChoice";
    optional_union.union_decl = &optional_decl; optional_decl.type = &optional_union;
    optional_union.size = 32; optional_union.align = 8; optional_union.sum_payload_offset = 8;
    TypeInfo nested_optional; nested_optional.kind = TypeKind::optional;
    nested_optional.name = "string??"; nested_optional.elem = &optional_text;
    nested_optional.size = 32; nested_optional.align = 8;
    nested_optional.sum_payload_offset = 8; nested_optional.sum_default = 0;
    for (int mode = 0; mode < 5; ++mode) {
      File sum_file; Evaluator owner{&sum_file};
      TypeInfo bytes_type; bytes_type.kind = TypeKind::view; bytes_type.elem = &T_U8;
      bytes_type.size = 16; bytes_type.align = 8;
      Value bytes; bytes.type = &bytes_type; bytes.agg = owner.storage(); bytes.count = 4;
      for (uint8_t ch : {'A', 'B', 'C', 'D'}) bytes.agg->push_back(Evaluator::scalar(&T_U8, ch));
      Value text = owner.do_cast(&cast, bytes); text.offset = 1; text.count = 2;
      Value active;
      if (mode == 0) active = owner.sum_value(&optional_text, 1, text, {});
      if (mode == 1) active = owner.sum_value(&text_choice, 1, text, {});
      if (mode == 2) active = owner.sum_value(&optional_choice, 1, owner.sum_value(&text_choice, 1, text, {}), {});
      if (mode == 3) active = owner.sum_value(&optional_union, 1, owner.sum_value(&optional_text, 1, text, {}), {});
      if (mode == 4) active = owner.sum_value(&nested_optional, 1, owner.sum_value(&optional_text, 1, text, {}), {});
      assert(!owner.failed && owner.valid_sum(active, {}));
      Expr saved; owner.freeze(&saved, active, {});
      assert(!owner.failed && saved.kind == ExprKind::ct_value && saved.ct && saved.ct->type == active.type);
      (*bytes.agg)[1].bits = 'X'; (*bytes.agg)[2].bits = 'Y';
      assert(Evaluator::text_byte(text, 0) == 'X' && Evaluator::text_byte(text, 1) == 'Y');
      active = {}; text = {}; bytes = {};  // retire every caller-side owner
      const Value *held = saved.ct;
      for (int depth = 0; depth < (mode < 2 ? 1 : 2); ++depth) {
        assert(owner.valid_sum(*held, {}) && held->sum_tag == 1 && held->agg && held->agg->size() == 1);
        held = &(*held->agg)[0];
      }
      assert(held->type == &T_STRING && !held->agg && held->string_owner);
      assert(str_eq(held->s, "BC") && sum_file.ct_memory->string_bytes == 3 && owner.string_work == 3);
    }
    Type unit_types[2] = {&optional_text, &text_choice};
    for (Type t : unit_types) {
      File unit_file; Evaluator owner{&unit_file};
      Value empty = owner.sum_value(t, 0, {}, {}); Expr saved; owner.freeze(&saved, empty, {});
      assert(!owner.failed && saved.ct && saved.ct->sum_tag == 0 && !saved.ct->agg);
      assert(unit_file.ct_memory->string_bytes == 0 && owner.string_work == 0);
    }
    // Capture the expected diagnostic without printing or using a source
    // escape to construct an invalid value. Failure publishes no expression.
    for (Type t : unit_types) {
      File invalid_file; Evaluator owner{&invalid_file}; owner.quiet = true;
      owner.current_loc = {7, 9, 31};
      Value invalid; invalid.type = t; invalid.sum_tag = 2;
      Expr marker; Expr original; original.kind = ExprKind::binary; original.type = T_INT;
      original.lhs = original.rhs = original.third = &marker; original.int_value = 73;
      owner.freeze(&original, invalid, owner.current_loc);
      assert(owner.failed && owner.fail_code == "E0401" && owner.fail_message.find("invalid active sum tag") != std::string::npos);
      assert(owner.fail_loc.line == 7 && owner.fail_loc.col == 9 && owner.fail_loc.offset == 31);
      assert(original.kind == ExprKind::binary && original.type == T_INT && original.int_value == 73);
      assert(original.lhs == &marker && original.rhs == &marker && original.third == &marker && !original.ct);
      assert(invalid_file.frozen_values.empty() && invalid_file.ct_memory->string_bytes == 0 && owner.string_work == 0);
    }
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
