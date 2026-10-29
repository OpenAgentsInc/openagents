// Observe real evaluator traversal admission under sanitizers.
#define main coax_driver_main
#include "../src/main.cpp"
#undef main
#include <cassert>

int main() {
  using namespace coax;
  File file;
  Evaluator owner{&file};
  TypeInfo array; array.kind = TypeKind::array; array.elem = &T_U8; array.length = 3;
  Value input; input.type = &array; input.agg = owner.storage(); input.agg->resize(3);
  for (auto &element : *input.agg) element = owner.scalar(&T_U8, 7);
  const auto baseline_slots = file.ct_memory->slots;
  {
    Evaluator scalar_only{&file}; scalar_only.traversal_limit = 0;
    Value a = scalar_only.scalar(&T_U8, 7), b = scalar_only.scalar(&T_U8, 0);
    auto copied = scalar_only.clone(a);
    scalar_only.assign_into(b, copied);
    assert(!scalar_only.contains_view(a) && b.bits == 7 && scalar_only.traversal_work == 0);
  }
  {
    Evaluator exact{&file}; exact.traversal_limit = 4;
    auto copied = exact.clone(input);
    assert(exact.traversal_work == 4 && copied.agg != input.agg);
  }
  assert(file.ct_memory->slots == baseline_slots);
  {
    Evaluator short_limit{&file}; short_limit.traversal_limit = 3;
    try { (void)short_limit.clone(input); assert(false); } catch (const CtTraversalLimit &) {}
    assert(short_limit.traversal_work == 3);
  }
  assert(file.ct_memory->slots == baseline_slots);
  {
    Evaluator exact{&file}; exact.traversal_limit = 4;
    assert(!exact.contains_view(input) && exact.traversal_work == 4);
  }
  {
    Evaluator short_limit{&file}; short_limit.traversal_limit = 3;
    Expr original; original.kind = ExprKind::binary; original.type = T_INT;
    try { short_limit.freeze(&original, input, {}); assert(false); } catch (const CtTraversalLimit &) {}
    assert(original.kind == ExprKind::binary && original.type == T_INT);
  }
  {
    Value output; output.type = &array; output.agg = owner.storage(); output.agg->resize(3);
    Evaluator exact{&file}; exact.traversal_limit = 4;
    exact.assign_into(output,input);
    assert(exact.traversal_work == 4 && (*output.agg)[2].bits == 7);
    Evaluator short_limit{&file}; short_limit.traversal_limit = 3;
    try { short_limit.assign_into(output,input); assert(false); } catch (const CtTraversalLimit &) {}
    assert(short_limit.traversal_work == 3);
  }
  {
    Field fields[2]; fields[0].type = &T_U8; fields[1].type = &T_U8;
    StructDecl decl; decl.fields = {fields,2};
    TypeInfo structure; structure.kind = TypeKind::struct_; structure.struct_decl = &decl;
    Evaluator short_limit{&file}; short_limit.traversal_limit = 1;
    try { (void)short_limit.zero(&structure, {}); assert(false); } catch (const CtTraversalLimit &) {}
    Evaluator exact{&file}; exact.traversal_limit = 2;
    auto result = exact.zero(&structure, {});
    assert(exact.traversal_work == 2 && result.agg->size() == 2);
  }
  {
    Expr defaults[2];
    for (auto &v : defaults) { v.kind = ExprKind::int_lit; v.type = &T_U8; v.int_value = 5; }
    Field fields[2];
    for (int i=0; i<2; ++i) { fields[i].type = &T_U8; fields[i].init = &defaults[i]; }
    StructDecl decl; decl.fields = {fields,2};
    TypeInfo structure; structure.kind = TypeKind::struct_; structure.struct_decl = &decl;
    Evaluator short_limit{&file}; short_limit.traversal_limit = 1;
    try { (void)short_limit.zero(&structure, {}); assert(false); } catch (const CtTraversalLimit &) {}
    assert(decl.defaults == 0);
    Evaluator exact{&file}; exact.traversal_limit = 4;
    auto result = exact.zero(&structure, {});
    assert(exact.traversal_work == 4 && (*result.agg)[0].bits == 5 && decl.defaults == 2);
  }
  {
    Field fields[2]; fields[0].type = &T_U8; fields[1].type = &T_U8;
    StructDecl decl; decl.fields = {fields,2};
    TypeInfo structure; structure.kind = TypeKind::struct_; structure.struct_decl = &decl;
    Expr value; value.kind = ExprKind::int_lit; value.type = &T_U8; value.int_value = 7;
    FieldInit inits[2]; inits[0].field = 0; inits[0].value = &value;
    inits[1].field = 1; inits[1].value = &value;
    Expr literal; literal.kind = ExprKind::struct_lit; literal.type = &structure; literal.fields = {inits,2};
    CtFrame frame;
    Evaluator short_limit{&file}; short_limit.traversal_limit = 3;
    try { (void)short_limit.expr(&literal,frame,true); assert(false); } catch (const CtTraversalLimit &) {}
    Evaluator exact{&file}; exact.traversal_limit = 4;
    auto result = exact.expr(&literal,frame,true);
    assert(exact.traversal_work == 4 && (*result.agg)[1].bits == 7);
  }
  {
    Expr value; value.kind = ExprKind::int_lit; value.type = &T_U8; value.int_value = 9;
    Expr *elements[3] = {&value,&value,&value};
    Expr literal; literal.kind = ExprKind::array_lit; literal.type = &array; literal.args = {elements,3};
    CtFrame frame;
    Evaluator short_limit{&file}; short_limit.traversal_limit = 2;
    try { (void)short_limit.expr(&literal,frame,true); assert(false); } catch (const CtTraversalLimit &) {}
    Evaluator exact{&file}; exact.traversal_limit = 3;
    auto result = exact.expr(&literal,frame,true);
    assert(exact.traversal_work == 3 && (*result.agg)[2].bits == 9);
  }
  return 0;
}
