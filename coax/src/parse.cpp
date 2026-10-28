// Representation bridge only. Grammar, tree construction, budgets and syntax
// diagnostics live in lib/compiler/parser.coax. No native parser fallback.
#include "coax.h"
#ifdef COAX_BOOTSTRAP_IMPORTER
#include "parser.h"
#else
#include "proto_importer.h"
#endif
namespace coax {
namespace {
static_assert((uint32_t)TypeExpr::named == COAX_PS_Tag_type_named-1, "type AST ABI: named");
static_assert((uint32_t)TypeExpr::array == COAX_PS_Tag_type_array-1, "type AST ABI: array");
static_assert((uint32_t)TypeExpr::view == COAX_PS_Tag_type_view-1, "type AST ABI: view");
static_assert((uint32_t)TypeExpr::dynamic == COAX_PS_Tag_type_dynamic-1, "type AST ABI: dynamic");
static_assert((uint32_t)ExprKind::int_lit == COAX_PS_Tag_expr_int_lit-16, "expression AST ABI: int_lit");
static_assert((uint32_t)ExprKind::float_lit == COAX_PS_Tag_expr_float_lit-16, "expression AST ABI: float_lit");
static_assert((uint32_t)ExprKind::bool_lit == COAX_PS_Tag_expr_bool_lit-16, "expression AST ABI: bool_lit");
static_assert((uint32_t)ExprKind::string_lit == COAX_PS_Tag_expr_string_lit-16, "expression AST ABI: string_lit");
static_assert((uint32_t)ExprKind::ident == COAX_PS_Tag_expr_ident-16, "expression AST ABI: ident");
static_assert((uint32_t)ExprKind::call == COAX_PS_Tag_expr_call-16, "expression AST ABI: call");
static_assert((uint32_t)ExprKind::unary == COAX_PS_Tag_expr_unary-16, "expression AST ABI: unary");
static_assert((uint32_t)ExprKind::binary == COAX_PS_Tag_expr_binary-16, "expression AST ABI: binary");
static_assert((uint32_t)ExprKind::run == COAX_PS_Tag_expr_run-16, "expression AST ABI: run");
static_assert((uint32_t)ExprKind::field == COAX_PS_Tag_expr_field-16, "expression AST ABI: field");
static_assert((uint32_t)ExprKind::index == COAX_PS_Tag_expr_index-16, "expression AST ABI: index");
static_assert((uint32_t)ExprKind::slice == COAX_PS_Tag_expr_slice-16, "expression AST ABI: slice");
static_assert((uint32_t)ExprKind::struct_lit == COAX_PS_Tag_expr_struct_lit-16, "expression AST ABI: struct_lit");
static_assert((uint32_t)ExprKind::array_lit == COAX_PS_Tag_expr_array_lit-16, "expression AST ABI: array_lit");
static_assert((uint32_t)ExprKind::cast == COAX_PS_Tag_expr_cast-16, "expression AST ABI: cast");
static_assert((uint32_t)ExprKind::ifx == COAX_PS_Tag_expr_ifx-16, "expression AST ABI: ifx");
static_assert((uint32_t)StmtKind::block == COAX_PS_Tag_stmt_block-40, "statement AST ABI: block");
static_assert((uint32_t)StmtKind::expr == COAX_PS_Tag_stmt_expr-40, "statement AST ABI: expr");
static_assert((uint32_t)StmtKind::var == COAX_PS_Tag_stmt_var-40, "statement AST ABI: var");
static_assert((uint32_t)StmtKind::assign == COAX_PS_Tag_stmt_assign-40, "statement AST ABI: assign");
static_assert((uint32_t)StmtKind::ret == COAX_PS_Tag_stmt_ret-40, "statement AST ABI: ret");
static_assert((uint32_t)StmtKind::if_ == COAX_PS_Tag_stmt_if_-40, "statement AST ABI: if_");
static_assert((uint32_t)StmtKind::while_ == COAX_PS_Tag_stmt_while_-40, "statement AST ABI: while_");
static_assert((uint32_t)StmtKind::for_range == COAX_PS_Tag_stmt_for_range-40, "statement AST ABI: for_range");
static_assert((uint32_t)StmtKind::for_each == COAX_PS_Tag_stmt_for_each-40, "statement AST ABI: for_each");
static_assert((uint32_t)StmtKind::break_ == COAX_PS_Tag_stmt_break_-40, "statement AST ABI: break_");
static_assert((uint32_t)StmtKind::continue_ == COAX_PS_Tag_stmt_continue_-40, "statement AST ABI: continue_");
static_assert((uint32_t)StmtKind::defer == COAX_PS_Tag_stmt_defer-40, "statement AST ABI: defer");
static_assert((uint32_t)StmtKind::assert_ == COAX_PS_Tag_stmt_assert_-40, "statement AST ABI: assert_");
Loc parser_loc(coax_t_PS_Loc l) {return {(int32_t)l.line,(int32_t)l.column,(int32_t)l.offset};}
Str parser_str(coax_string s) {return {s.data,(int32_t)s.count};}
template<class T> T *parser_ref(const std::vector<void*> &objects,int64_t id) {return (T*)objects[(size_t)id];}
template<class T> Slice<T> parser_values(Arena *arena,const std::vector<coax_t_PS_Node> &nodes,const std::vector<void*> &objects,int64_t head) {
  std::vector<T> values;for(auto id=head;id;id=nodes[(size_t)id].next) values.push_back(*parser_ref<T>(objects,id));return to_slice(arena,values);
}
template<class T> Slice<T*> parser_pointers(Arena *arena,const std::vector<coax_t_PS_Node> &nodes,const std::vector<void*> &objects,int64_t head) {
  std::vector<T*> values;for(auto id=head;id;id=nodes[(size_t)id].next) values.push_back(parser_ref<T>(objects,id));return to_slice(arena,values);
}
}
namespace {
std::vector<coax_t_PS_Token> parser_tokens(const std::vector<Token> &tokens) {
  std::vector<coax_t_PS_Token> input;input.reserve(tokens.size());
  for(const auto &t:tokens) {
    const auto text=t.kind==Tok::string_lit?t.string_value:t.text;
    input.push_back({(uint32_t)t.kind,{text.data,text.count},{t.loc.line,t.loc.col,t.loc.offset},t.int_value,t.float_value,t.float_bits,t.text.count});
  }
  return input;
}
}
bool parse(Arena *arena,const Source *src,const std::vector<Token> &tokens,File *out) {
  auto input=parser_tokens(tokens);
  const auto caps=PS_caps((int64_t)input.size(),(int64_t)src->text.size());
  std::vector<coax_t_PS_Node> nodes((size_t)caps.nodes);std::vector<uint8_t> message((size_t)caps.message);
  coax_t_PS_State state{};
  const auto result=PS_parse({src->text.data(),(int64_t)src->text.size()},{input.data(),(int64_t)input.size()},
      {nodes.data(),(int64_t)nodes.size()},{&state,1},{message.data(),(int64_t)message.size()});
  if(!result.ok) {
    error_at(src,parser_loc(state.loc),std::string(state.code.data,(size_t)state.code.count).c_str(),
        std::string((char*)message.data(),(size_t)state.length),std::string(state.fix.data?state.fix.data:"",(size_t)state.fix.count));return false;
  }
  std::vector<void*> objects((size_t)state.used+1,nullptr);
  // Allocate first so all subsequent reference copies are independent of order.
  for(int64_t id=1;id<=state.used;id++) {
    const auto k=nodes[(size_t)id].kind;void *p=nullptr;
    if(k>=1 && k<=4) p=make<TypeExpr>(arena);
    else if(k>=16 && k<=31) p=make<Expr>(arena);
    else if(k>=40 && k<=52) p=make<Stmt>(arena);
    else switch(k) {
      case COAX_PS_Tag_decl_proc:p=make<ProcDecl>(arena);break;case COAX_PS_Tag_decl_constant:p=make<ConstDecl>(arena);break;
      case COAX_PS_Tag_decl_struct_:p=make<StructDecl>(arena);break;case COAX_PS_Tag_decl_enum_:p=make<EnumDecl>(arena);break;
      case COAX_PS_Tag_decl_param:p=make<Param>(arena);break;case COAX_PS_Tag_decl_field:p=make<Field>(arena);break;
      case COAX_PS_Tag_decl_enum_member:p=make<EnumMember>(arena);break;case COAX_PS_Tag_decl_field_init:p=make<FieldInit>(arena);break;
      case COAX_PS_Tag_decl_import_:p=make<Import>(arena);break;case COAX_PS_Tag_decl_check:p=make<CheckDecl>(arena);break;
    }
    objects[(size_t)id]=p;
  }
  // Scalar and reference fields; list value copies happen after this pass.
  for(int64_t id=1;id<=state.used;id++) {
    const auto &v=nodes[(size_t)id];const auto loc=parser_loc(v.loc);const auto name=parser_str(v.name);
    if(v.kind>=1 && v.kind<=4) {
      auto *p=parser_ref<TypeExpr>(objects,id);p->kind=(TypeExpr::Kind)(v.kind-1);p->loc=loc;p->name=name;
      p->length=parser_ref<Expr>(objects,v.a);p->elem=parser_ref<TypeExpr>(objects,v.b);
    }else if(v.kind>=16 && v.kind<=31) {
      auto *p=parser_ref<Expr>(objects,id);p->kind=(ExprKind)(v.kind-16);p->loc=loc;p->text=name;p->int_value=v.value;p->float_value=v.floating;
      if(v.width) p->type=v.width==32?&T_F32:&T_F64;
      p->op=(Tok)v.op;p->lhs=parser_ref<Expr>(objects,v.a);p->rhs=parser_ref<Expr>(objects,v.b);p->third=parser_ref<Expr>(objects,v.c);
      p->type_expr=parser_ref<TypeExpr>(objects,v.d);p->trunc=v.flag;
    }else if(v.kind>=40 && v.kind<=52) {
      auto *p=parser_ref<Stmt>(objects,id);p->kind=(StmtKind)(v.kind-40);p->loc=loc;p->name=name;p->name2=parser_str(v.name2);p->name2_loc=parser_loc(v.loc2);
      p->expr=parser_ref<Expr>(objects,v.a);p->expr2=parser_ref<Expr>(objects,v.b);p->uninit=v.flag;
      if(v.kind==42) p->type_expr=parser_ref<TypeExpr>(objects,v.d);
      else {p->then_=parser_ref<Stmt>(objects,v.c);p->else_=parser_ref<Stmt>(objects,v.d);}
      if(v.kind==43) {p->target=parser_ref<Expr>(objects,v.c);p->then_=nullptr;p->op=(Tok)v.op;}
    }else switch(v.kind) {
      case COAX_PS_Tag_decl_proc:{auto *p=parser_ref<ProcDecl>(objects,id);p->loc=loc;p->name=name;p->ret_expr=parser_ref<TypeExpr>(objects,v.a);p->body=parser_ref<Stmt>(objects,v.b);p->export_c=v.flag;p->foreign=v.flag2;break;}
      case COAX_PS_Tag_decl_constant:{auto *p=parser_ref<ConstDecl>(objects,id);p->loc=loc;p->name=name;p->expr=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_struct_:{auto *p=parser_ref<StructDecl>(objects,id);p->loc=loc;p->name=name;break;}
      case COAX_PS_Tag_decl_enum_:{auto *p=parser_ref<EnumDecl>(objects,id);p->loc=loc;p->name=name;p->base_expr=parser_ref<TypeExpr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_param:{auto *p=parser_ref<Param>(objects,id);p->loc=loc;p->name=name;p->type_expr=parser_ref<TypeExpr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_field:{auto *p=parser_ref<Field>(objects,id);p->loc=loc;p->name=name;p->type_expr=parser_ref<TypeExpr>(objects,v.a);p->init=parser_ref<Expr>(objects,v.b);break;}
      case COAX_PS_Tag_decl_enum_member:{auto *p=parser_ref<EnumMember>(objects,id);p->loc=loc;p->name=name;p->value=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_field_init:{auto *p=parser_ref<FieldInit>(objects,id);p->loc=loc;p->name=name;p->value=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_import_:{auto *p=parser_ref<Import>(objects,id);p->loc=loc;p->module=name;break;}
      case COAX_PS_Tag_decl_check:{auto *p=parser_ref<CheckDecl>(objects,id);p->loc=loc;p->name=name;p->forall=v.flag;p->proc=parser_ref<ProcDecl>(objects,v.a);
        std::vector<Binding> domains;for(auto b=nodes[(size_t)v.a].head;b;b=nodes[(size_t)b].next) domains.push_back({parser_ref<Expr>(objects,nodes[(size_t)b].b),parser_ref<Expr>(objects,nodes[(size_t)b].c)});
        p->domains=to_slice(arena,domains);break;}
    }
  }
  for(int64_t id=1;id<=state.used;id++) {
    const auto &v=nodes[(size_t)id];switch(v.kind) {
      case COAX_PS_Tag_expr_call:case COAX_PS_Tag_expr_array_lit:parser_ref<Expr>(objects,id)->args=parser_pointers<Expr>(arena,nodes,objects,v.head);break;
      case COAX_PS_Tag_expr_struct_lit:parser_ref<Expr>(objects,id)->fields=parser_values<FieldInit>(arena,nodes,objects,v.head);break;
      case COAX_PS_Tag_stmt_block:parser_ref<Stmt>(objects,id)->body=parser_pointers<Stmt>(arena,nodes,objects,v.head);break;
      case COAX_PS_Tag_decl_proc:parser_ref<ProcDecl>(objects,id)->params=parser_values<Param>(arena,nodes,objects,v.head);break;
      case COAX_PS_Tag_decl_struct_:parser_ref<StructDecl>(objects,id)->fields=parser_values<Field>(arena,nodes,objects,v.head);break;
      case COAX_PS_Tag_decl_enum_:parser_ref<EnumDecl>(objects,id)->members=parser_values<EnumMember>(arena,nodes,objects,v.head);break;
    }
  }
  std::vector<Import> imports;std::vector<ProcDecl*> procs;std::vector<ConstDecl*> constants;
  std::vector<StructDecl*> structs;std::vector<EnumDecl*> enums;std::vector<Expr*> assertions;std::vector<CheckDecl*> checks;
  for(auto id=nodes[(size_t)result.root].head;id;id=nodes[(size_t)id].next) switch(nodes[(size_t)id].kind) {
    case COAX_PS_Tag_decl_import_:imports.push_back(*parser_ref<Import>(objects,id));break;case COAX_PS_Tag_decl_proc:procs.push_back(parser_ref<ProcDecl>(objects,id));break;
    case COAX_PS_Tag_decl_constant:constants.push_back(parser_ref<ConstDecl>(objects,id));break;case COAX_PS_Tag_decl_struct_:structs.push_back(parser_ref<StructDecl>(objects,id));break;
    case COAX_PS_Tag_decl_enum_:enums.push_back(parser_ref<EnumDecl>(objects,id));break;case COAX_PS_Tag_decl_check:checks.push_back(parser_ref<CheckDecl>(objects,id));break;
    case COAX_PS_Tag_decl_assertion:assertions.push_back(parser_ref<Expr>(objects,nodes[(size_t)id].a));break;
  }
  for (auto *p:procs) {
    p->authority=src->units.empty()?nodes[(size_t)result.root].flag:source_authority(src,p->loc);
    p->root_source=src->units.empty() || source_point(src,p->loc).path==src->units.front()->path;
  }
  out->src=src;out->authority=nodes[(size_t)result.root].flag;out->imports=to_slice(arena,imports);out->procs=to_slice(arena,procs);
  out->constants=to_slice(arena,constants);out->structs=to_slice(arena,structs);out->enums=to_slice(arena,enums);out->assertions=to_slice(arena,assertions);out->checks=to_slice(arena,checks);
  attach_check_input(out,nodes,objects,state.used);
  return true;
}
} // namespace coax
