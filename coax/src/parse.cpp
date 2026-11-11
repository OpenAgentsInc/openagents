// Representation bridge only. Grammar, tree construction, budgets and syntax
// diagnostics live in lib/compiler/parser.coax. No native parser fallback.
#include "coax.h"
#include <functional>
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
static_assert((uint32_t)TypeExpr::param == COAX_PS_Tag_type_param-1, "type AST ABI: param");
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
static_assert((uint32_t)TypeExpr::optional == COAX_PS_Tag_type_optional-1, "type AST ABI: optional");
static_assert((uint32_t)ExprKind::sum == COAX_PS_Tag_expr_sum-16, "expression AST ABI: sum");
static_assert((uint32_t)ExprKind::null_ == COAX_PS_Tag_expr_null-16, "expression AST ABI: null");
static_assert((uint32_t)ExprKind::some == COAX_PS_Tag_expr_some-16, "expression AST ABI: some");
static_assert((uint32_t)ExprKind::try_ == COAX_PS_Tag_expr_try-16, "expression AST ABI: try");
static_assert((uint32_t)ExprKind::or_else == COAX_PS_Tag_expr_or_else-16, "expression AST ABI: or_else");
static_assert((uint32_t)StmtKind::switch_ == COAX_PS_Tag_stmt_switch-40, "statement AST ABI: switch");
bool parser_type_tag(uint32_t kind) { switch(kind) {
  case COAX_PS_Tag_type_named:case COAX_PS_Tag_type_array:case COAX_PS_Tag_type_view:case COAX_PS_Tag_type_dynamic:case COAX_PS_Tag_type_param:case COAX_PS_Tag_type_optional:return true;
  default:return false;
}}
bool parser_expr_tag(uint32_t kind) { switch(kind) {
  case COAX_PS_Tag_expr_int_lit:case COAX_PS_Tag_expr_float_lit:case COAX_PS_Tag_expr_bool_lit:case COAX_PS_Tag_expr_string_lit:case COAX_PS_Tag_expr_ident:case COAX_PS_Tag_expr_call:case COAX_PS_Tag_expr_unary:case COAX_PS_Tag_expr_binary:case COAX_PS_Tag_expr_run:case COAX_PS_Tag_expr_field:case COAX_PS_Tag_expr_index:case COAX_PS_Tag_expr_slice:case COAX_PS_Tag_expr_struct_lit:case COAX_PS_Tag_expr_array_lit:case COAX_PS_Tag_expr_cast:case COAX_PS_Tag_expr_ifx:case COAX_PS_Tag_expr_sum:case COAX_PS_Tag_expr_null:case COAX_PS_Tag_expr_some:case COAX_PS_Tag_expr_try:case COAX_PS_Tag_expr_or_else:return true;
  default:return false;
}}
bool parser_stmt_tag(uint32_t kind) { switch(kind) {
  case COAX_PS_Tag_stmt_block:case COAX_PS_Tag_stmt_expr:case COAX_PS_Tag_stmt_var:case COAX_PS_Tag_stmt_assign:case COAX_PS_Tag_stmt_ret:case COAX_PS_Tag_stmt_if_:case COAX_PS_Tag_stmt_while_:case COAX_PS_Tag_stmt_for_range:case COAX_PS_Tag_stmt_for_each:case COAX_PS_Tag_stmt_break_:case COAX_PS_Tag_stmt_continue_:case COAX_PS_Tag_stmt_defer:case COAX_PS_Tag_stmt_assert_:case COAX_PS_Tag_stmt_switch:case COAX_PS_Tag_stmt_transaction:return true;
  default:return false;
}}
Loc parser_loc(coax_t_PS_Loc l) {return {(int32_t)l.line,(int32_t)l.column,(int32_t)l.offset};}
Str parser_str(coax_string s) {return {s.data,(int32_t)s.count};}
template<class T> T *parser_ref(const std::vector<void*> &objects,int64_t id) {return (T*)objects[(size_t)id];}
template<class T> Slice<T> parser_values(Arena *arena,const zvec<coax_t_PS_Node> &nodes,const std::vector<void*> &objects,int64_t head) {
  std::vector<T> values;for(auto id=head;id;id=nodes[(size_t)id].next) values.push_back(*parser_ref<T>(objects,id));return to_slice(arena,values);
}
template<class T> Slice<T*> parser_pointers(Arena *arena,const zvec<coax_t_PS_Node> &nodes,const std::vector<void*> &objects,int64_t head) {
  std::vector<T*> values;for(auto id=head;id;id=nodes[(size_t)id].next) values.push_back(parser_ref<T>(objects,id));return to_slice(arena,values);
}
}
namespace {
std::vector<coax_t_PS_Token> parser_tokens(const std::vector<Token> &tokens) {
  std::vector<coax_t_PS_Token> input(tokens.size());
  burst(tokens.size(),16384,[&](size_t begin,size_t end,unsigned) {
    for(size_t i=begin;i<end;i++) {
      const auto &t=tokens[i];
      const auto text=t.kind==Tok::string_lit?t.string_value:t.text;
      input[i]={(uint32_t)t.kind,{text.data,text.count},{t.loc.line,t.loc.col,t.loc.offset},t.int_value,t.float_value,t.float_bits,t.text.count};
    }
  });
  return input;
}
}
// Reports a failed parse: its first error exactly as before, then one error
// per later attempt, each parsed again from the next top-level declaration
// (PS_recover owns where, and which errors are likely cascades), until the
// tokens run out or error_limit. lexed: where this unit's lexer errors are.
static void parse_report(const Source *src,const std::vector<coax_t_PS_Token> &input,const coax_t_PS_State &first,
                         const std::string &first_message,const std::vector<Loc> &lexed,bool development) {
  std::vector<int64_t> offsets;for(const auto &l:lexed) offsets.push_back(l.offset);
  const coax_v_t_PS_Token tokens{(coax_t_PS_Token*)input.data(),(int64_t)input.size()};
  const coax_v_s64 causes{offsets.data(),(int64_t)offsets.size()};
  const auto caps=PS_caps((int64_t)input.size(),(int64_t)src->text.size());
  zvec<coax_t_PS_Node> nodes((size_t)caps.nodes);zvec<uint8_t> message((size_t)caps.message);
  coax_t_PS_State state=first;std::string text=first_message;
  auto at=PS_recover(tokens,0,first.pos,causes,false);Loc previous{};
  for(int64_t from=0;;) {
    const std::string code(state.code.data,(size_t)state.code.count),fix(state.fix.data?state.fix.data:"",(size_t)state.fix.count);
    if(at.cause>=0) error_after(src,parser_loc(state.loc),code.c_str(),text,fix,src,lexed[(size_t)at.cause]);
    else if(at.follows) error_after(src,parser_loc(state.loc),code.c_str(),text,fix,src,previous);
    else error_at(src,parser_loc(state.loc),code.c_str(),text,fix);
    previous=parser_loc(state.loc);
    if(at.next>=tokens.count-1 || error_limit()) return;
    from=at.next;state={};std::fill(nodes.begin(),nodes.end(),coax_t_PS_Node{});
    const auto result=PS_parse_build({src->text.data(),(int64_t)src->text.size()},{tokens.data+from,tokens.count-from},
        {nodes.data(),(int64_t)nodes.size()},{&state,1},{message.data(),(int64_t)message.size()},development);
    if(result.ok) return;
    text=std::string((char*)message.data(),(size_t)state.length);
    at=PS_recover(tokens,from,from+state.pos,causes,!at.balanced);
  }
}
// #run_steps N; is the budget of the compile whose entry file declares it. A
// loaded file's well-formed declaration is dropped here, before the parser
// sees it, so a library declares what it needs when it is checked alone and
// never raises the budget of a program that loads it. A malformed one still
// reaches the parser (E0224), and so does a second one in the same file.
// False with E0224 reported when a loaded file declares it twice.
// *same: nothing was dropped, and kept is left empty: the caller reads tokens.
bool drop_loaded_run_steps(const Source *src,const std::vector<Token> &tokens,std::vector<Token> *kept,bool *same) {
  kept->clear();*same=true;
  if(src->units.size()<2) return true;
  const std::string &entry=src->units.front()->path;
  std::vector<std::string> declared;
  for(size_t i=0;i<tokens.size();i++) {
    const auto &t=tokens[i];
    // N from 1 through parser.coax's PS_RUN_STEPS_MAX, as the parser requires.
    if(t.kind==Tok::directive && str_eq(t.text,"run_steps") && i+2<tokens.size() && tokens[i+1].kind==Tok::int_lit &&
       tokens[i+1].int_value>=1 && tokens[i+1].int_value<=250000000 && tokens[i+2].kind==Tok::semicolon) {
      const auto at=source_point(src,t.loc);
      if(at.path!=entry) {
        for(const auto &path:declared) if(path==at.path) {
          error_at(src,t.loc,"E0224","#run_steps is declared more than once","declare one #run_steps N; in a file");return false;
        }
        declared.push_back(at.path);
        if(*same) {*same=false;kept->reserve(tokens.size());kept->assign(tokens.begin(),tokens.begin()+(std::ptrdiff_t)i);}
        i+=2;continue;
      }
    }
    if(!*same) kept->push_back(t);
  }
  return true;
}
// Called once the tokens parsed, before the tree is made: true when the
// compile already has its errors (the changed bodies' check, skeleton.cpp)
// and they are reported, so there is nothing to build a tree for.
static std::function<bool()> g_parsed_far_enough;
bool parse(Arena *arena,const Source *src,const std::vector<Token> &all_tokens,File *out,bool development) {
  std::vector<Token> kept;bool same=true;
  if(!drop_loaded_run_steps(src,all_tokens,&kept,&same)) return false;
  const std::vector<Token> &tokens=same?all_tokens:kept;
  TimedLaps laps;  // --timings: parse.tokens, parse.build (the Coax parser), parse.tree, parse.input (the checker's copy)
  auto input=parser_tokens(tokens);
  const auto caps=PS_caps((int64_t)input.size(),(int64_t)src->text.size());
  zvec<coax_t_PS_Node> nodes((size_t)caps.nodes);zvec<uint8_t> message((size_t)caps.message);
  coax_t_PS_State state{};
  laps.lap("parse.tokens");
  const auto result=PS_parse_build({src->text.data(),(int64_t)src->text.size()},{input.data(),(int64_t)input.size()},
      {nodes.data(),(int64_t)nodes.size()},{&state,1},{message.data(),(int64_t)message.size()},development);
  capacity_use("parser nesting","E0221",(uint64_t)state.peak,(uint64_t)PS_depth_limit());
  laps.lap("parse.build");
  if(!result.ok) {parse_report(src,input,state,std::string((char*)message.data(),(size_t)state.length),{},development);return false;}
  if(g_parsed_far_enough && g_parsed_far_enough()) return false;
  std::vector<void*> objects((size_t)state.used+1,nullptr);
  // Allocate first so all subsequent reference copies are independent of order.
  // One block holds every node's object, in node order: each node's size is
  // found, then its place, then the objects are made, on threads (burst, main.cpp).
  auto object=[&](uint32_t k,void *at)->size_t {
#define COAX_OBJECT(T) do{if(at)new(at)T{};return (sizeof(T)+15)&~(size_t)15;}while(0)
    if(parser_type_tag(k)) COAX_OBJECT(TypeExpr);
    if(parser_expr_tag(k)) COAX_OBJECT(Expr);
    if(parser_stmt_tag(k)) COAX_OBJECT(Stmt);
    switch(k) {
      case COAX_PS_Tag_decl_proc:COAX_OBJECT(ProcDecl);case COAX_PS_Tag_decl_constant:COAX_OBJECT(ConstDecl);
      case COAX_PS_Tag_decl_struct_:COAX_OBJECT(StructDecl);case COAX_PS_Tag_decl_enum_:COAX_OBJECT(EnumDecl);
      case COAX_PS_Tag_decl_param:COAX_OBJECT(Param);case COAX_PS_Tag_decl_field:COAX_OBJECT(Field);
      case COAX_PS_Tag_decl_enum_member:COAX_OBJECT(EnumMember);case COAX_PS_Tag_decl_field_init:COAX_OBJECT(FieldInit);
      case COAX_PS_Tag_decl_import_:COAX_OBJECT(Import);case COAX_PS_Tag_decl_check:COAX_OBJECT(CheckDecl);
      case COAX_PS_Tag_decl_union:COAX_OBJECT(UnionDecl);case COAX_PS_Tag_decl_variant:COAX_OBJECT(UnionVariant);
      case COAX_PS_Tag_switch_case:COAX_OBJECT(SwitchCase);case COAX_PS_Tag_case_binding:COAX_OBJECT(CaseBinding);
      case COAX_PS_Tag_effect_name:COAX_OBJECT(EffectName);
    }
#undef COAX_OBJECT
    return 0;
  };
  // places[id]: where node id's bytes begin in the block (places[used+1]: the total), from each node's size.
  auto place=[&](std::vector<size_t> &places)->uint8_t* {
    size_t total=0;
    for(int64_t id=1;id<=state.used;id++) {const size_t size=places[(size_t)id];places[(size_t)id]=total;total+=size;}
    places[(size_t)state.used+1]=total;
    return total?(uint8_t*)arena_alloc(arena,total):nullptr;
  };
  std::vector<size_t> places((size_t)state.used+2,0);
  burst((size_t)state.used,16384,[&](size_t begin,size_t end,unsigned) {
    for(int64_t id=(int64_t)begin+1;id<=(int64_t)end;id++) places[(size_t)id]=object(nodes[(size_t)id].kind,nullptr);
  });
  uint8_t *block=place(places);
  burst((size_t)state.used,16384,[&](size_t begin,size_t end,unsigned) {
    for(int64_t id=(int64_t)begin+1;id<=(int64_t)end;id++) if(places[(size_t)id+1]!=places[(size_t)id]) {
      objects[(size_t)id]=block+places[(size_t)id];object(nodes[(size_t)id].kind,objects[(size_t)id]);
    }
  });
  // Scalar and reference fields; list value copies happen after this pass.
  // Each node fills its own object from the parser's record: slices of nodes
  // on threads (burst, main.cpp). A check's domains are a list: made after.
  burst((size_t)state.used,16384,[&](size_t begin,size_t end,unsigned) {
  for(int64_t id=(int64_t)begin+1;id<=(int64_t)end;id++) {
    const auto &v=nodes[(size_t)id];const auto loc=parser_loc(v.loc);const auto name=parser_str(v.name);
    if(parser_type_tag(v.kind)) {
      auto *p=parser_ref<TypeExpr>(objects,id);p->kind=(TypeExpr::Kind)(v.kind-1);p->loc=loc;p->name=name;p->read_only=v.flag;
      p->length=v.a && nodes[(size_t)v.a].kind==COAX_PS_Tag_type_param?nullptr:parser_ref<Expr>(objects,v.a);p->elem=parser_ref<TypeExpr>(objects,v.b);
    }else if(parser_expr_tag(v.kind)) {
      auto *p=parser_ref<Expr>(objects,id);p->kind=(ExprKind)(v.kind-16);p->loc=loc;p->text=name;p->int_value=v.value;p->float_value=v.floating;
      if(v.width) p->type=v.width==32?&T_F32:&T_F64;
      p->op=(Tok)v.op;p->lhs=parser_ref<Expr>(objects,v.a);p->rhs=parser_ref<Expr>(objects,v.b);p->third=parser_ref<Expr>(objects,v.c);
      p->type_expr=parser_ref<TypeExpr>(objects,v.d);p->trunc=v.flag;
    }else if(parser_stmt_tag(v.kind)) {
      auto *p=parser_ref<Stmt>(objects,id);p->kind=(StmtKind)(v.kind-40);p->loc=loc;p->name=name;p->name2=parser_str(v.name2);p->name2_loc=parser_loc(v.loc2);
      p->expr=parser_ref<Expr>(objects,v.a);p->expr2=parser_ref<Expr>(objects,v.b);p->uninit=v.flag;
      if(v.kind==42) p->type_expr=parser_ref<TypeExpr>(objects,v.d);
      else {p->then_=parser_ref<Stmt>(objects,v.c);p->else_=parser_ref<Stmt>(objects,v.d);}
      if(v.kind==43) {p->target=parser_ref<Expr>(objects,v.c);p->then_=nullptr;p->op=(Tok)v.op;}
    }else switch(v.kind) {
      case COAX_PS_Tag_decl_proc:{auto *p=parser_ref<ProcDecl>(objects,id);p->loc=loc;p->end=(int32_t)v.end;p->name=name;p->ret_expr=parser_ref<TypeExpr>(objects,v.a);p->body=parser_ref<Stmt>(objects,v.b);p->export_c=v.flag;p->foreign=v.flag2;p->effect_present=v.effect_present;p->pure_decl=v.pure_decl;p->effect_loc=parser_loc(v.effect_loc);break;}
      case COAX_PS_Tag_decl_constant:{auto *p=parser_ref<ConstDecl>(objects,id);p->loc=loc;p->end=(int32_t)v.end;p->name=name;p->expr=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_struct_:{auto *p=parser_ref<StructDecl>(objects,id);p->persisted=v.flag;p->loc=loc;p->end=(int32_t)v.end;p->name=name;break;}
      case COAX_PS_Tag_decl_enum_:{auto *p=parser_ref<EnumDecl>(objects,id);p->loc=loc;p->end=(int32_t)v.end;p->name=name;p->base_expr=parser_ref<TypeExpr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_param:{auto *p=parser_ref<Param>(objects,id);p->loc=loc;p->name=name;p->type_expr=parser_ref<TypeExpr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_field:{auto *p=parser_ref<Field>(objects,id);p->loc=loc;p->name=name;p->type_expr=parser_ref<TypeExpr>(objects,v.a);p->init=parser_ref<Expr>(objects,v.b);p->flat=v.flag;p->depth=v.flag2;break;}
      case COAX_PS_Tag_decl_enum_member:{auto *p=parser_ref<EnumMember>(objects,id);p->loc=loc;p->name=name;p->value=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_field_init:{auto *p=parser_ref<FieldInit>(objects,id);p->loc=loc;p->name=name;p->value=parser_ref<Expr>(objects,v.a);break;}
      case COAX_PS_Tag_decl_union:{auto *p=parser_ref<UnionDecl>(objects,id);p->loc=loc;p->end=(int32_t)v.end;p->name=name;break;}
      case COAX_PS_Tag_decl_variant:{auto *p=parser_ref<UnionVariant>(objects,id);p->loc=loc;p->name=name;p->type_expr=parser_ref<TypeExpr>(objects,v.a);p->ordinal=(uint32_t)v.value;break;}
      case COAX_PS_Tag_switch_case:{auto *p=parser_ref<SwitchCase>(objects,id);p->loc=loc;p->name=name;p->enum_pattern=parser_ref<Expr>(objects,v.d);p->binding=parser_ref<CaseBinding>(objects,v.a);p->body=parser_ref<Stmt>(objects,v.b);break;}
      case COAX_PS_Tag_case_binding:{auto *p=parser_ref<CaseBinding>(objects,id);p->name=name;p->loc=loc;break;}
      case COAX_PS_Tag_effect_name:{auto *p=parser_ref<EffectName>(objects,id);p->name=name;p->loc=loc;break;}
      case COAX_PS_Tag_decl_import_:{auto *p=parser_ref<Import>(objects,id);p->loc=loc;p->module=name;break;}
      case COAX_PS_Tag_decl_check:{auto *p=parser_ref<CheckDecl>(objects,id);p->loc=loc;p->name=name;p->forall=v.flag;p->steps=v.value;p->traversal=(uint64_t)v.width;p->proc=parser_ref<ProcDecl>(objects,v.a);break;}
    }
  }
  });
  for(int64_t id=1;id<=state.used;id++) if(nodes[(size_t)id].kind==COAX_PS_Tag_decl_check) {
    const auto &v=nodes[(size_t)id];auto *p=parser_ref<CheckDecl>(objects,id);
    std::vector<Binding> domains;for(auto b=nodes[(size_t)v.a].head;b;b=nodes[(size_t)b].next) domains.push_back({parser_ref<Expr>(objects,nodes[(size_t)b].b),parser_ref<Expr>(objects,nodes[(size_t)b].c)});
    p->domains=to_slice(arena,domains);
  }
  // Each node's lists, the same way: their sizes, one block in node order, then the copies.
  auto lists=[&](int64_t id,uint8_t *at)->size_t {
    const auto &v=nodes[(size_t)id];
    // A list of the nodes from head: each one's value(node) in turn. Without a place, only its size.
    auto list=[&](int64_t head,auto *out,auto &&value)->size_t {
      using T=std::remove_reference_t<decltype(*out->data)>;
      int32_t n=0;
      for(auto at_node=head;at_node;at_node=nodes[(size_t)at_node].next) {if(at)new((void*)((T*)at+n))T(value(at_node));n++;}
      if(at) {out->count=n;out->data=n?(T*)at:nullptr;}
      return ((size_t)n*sizeof(T)+15)&~(size_t)15;
    };
    auto values=[&](int64_t head,auto *out) {return list(head,out,[&](int64_t node) {return *parser_ref<std::remove_reference_t<decltype(*out->data)>>(objects,node);});};
    auto pointers=[&](int64_t head,auto *out) {return list(head,out,[&](int64_t node) {return parser_ref<std::remove_pointer_t<std::remove_reference_t<decltype(*out->data)>>>(objects,node);});};
    switch(v.kind) {
      case COAX_PS_Tag_expr_call:case COAX_PS_Tag_expr_array_lit:return pointers(v.head,&parser_ref<Expr>(objects,id)->args);
      case COAX_PS_Tag_expr_struct_lit:return values(v.head,&parser_ref<Expr>(objects,id)->fields);
      case COAX_PS_Tag_stmt_block:return pointers(v.head,&parser_ref<Stmt>(objects,id)->body);
      case COAX_PS_Tag_decl_proc:{
        auto *p=parser_ref<ProcDecl>(objects,id);const size_t params=values(v.head,&p->params);
        uint8_t *first=at;if(at)at+=params;const size_t names=values(v.effect_head,&p->effect_names);at=first;return params+names;}
      case COAX_PS_Tag_stmt_switch:return pointers(v.head,&parser_ref<Stmt>(objects,id)->cases);
      case COAX_PS_Tag_decl_union:return values(v.head,&parser_ref<UnionDecl>(objects,id)->variants);
      case COAX_PS_Tag_type_named:return list(v.head,&parser_ref<TypeExpr>(objects,id)->args,[&](int64_t node) {
        TypeArgument a;if(parser_type_tag(nodes[(size_t)node].kind))a.type=parser_ref<TypeExpr>(objects,node);else a.length=parser_ref<Expr>(objects,node);return a;});
      case COAX_PS_Tag_decl_struct_:return values(v.head,&parser_ref<StructDecl>(objects,id)->fields);
      case COAX_PS_Tag_decl_enum_:return values(v.head,&parser_ref<EnumDecl>(objects,id)->members);
    }
    return 0;
  };
  burst((size_t)state.used,16384,[&](size_t begin,size_t end,unsigned) {
    for(int64_t id=(int64_t)begin+1;id<=(int64_t)end;id++) places[(size_t)id]=lists(id,nullptr);
  });
  block=place(places);
  burst((size_t)state.used,16384,[&](size_t begin,size_t end,unsigned) {
    for(int64_t id=(int64_t)begin+1;id<=(int64_t)end;id++) if(places[(size_t)id+1]!=places[(size_t)id]) lists(id,block+places[(size_t)id]);
  });
  std::vector<Import> imports;std::vector<ProcDecl*> procs;std::vector<ConstDecl*> constants;
  std::vector<StructDecl*> structs;std::vector<EnumDecl*> enums;std::vector<UnionDecl*> unions;std::vector<Expr*> assertions;std::vector<CheckDecl*> checks;
  for(auto id=nodes[(size_t)result.root].head;id;id=nodes[(size_t)id].next) switch(nodes[(size_t)id].kind) {
    case COAX_PS_Tag_decl_import_:imports.push_back(*parser_ref<Import>(objects,id));break;case COAX_PS_Tag_decl_proc:procs.push_back(parser_ref<ProcDecl>(objects,id));break;
    case COAX_PS_Tag_decl_constant:constants.push_back(parser_ref<ConstDecl>(objects,id));break;case COAX_PS_Tag_decl_struct_:structs.push_back(parser_ref<StructDecl>(objects,id));break;
    case COAX_PS_Tag_decl_enum_:enums.push_back(parser_ref<EnumDecl>(objects,id));break;case COAX_PS_Tag_decl_check:checks.push_back(parser_ref<CheckDecl>(objects,id));break;
    case COAX_PS_Tag_decl_union:unions.push_back(parser_ref<UnionDecl>(objects,id));break;
    case COAX_PS_Tag_decl_assertion:assertions.push_back(parser_ref<Expr>(objects,nodes[(size_t)id].a));break;
  }
  for (auto *p:procs) {
    p->authority=src->units.empty()?nodes[(size_t)result.root].flag:source_authority(src,p->loc);
    p->root_source=src->units.empty() || source_point(src,p->loc).path==src->units.front()->path;
  }
  // The entry file's #run_steps N; (a loaded file's never reaches the parser).
  if(nodes[(size_t)result.root].value) out->run_steps=nodes[(size_t)result.root].value;
  out->src=src;out->authority=nodes[(size_t)result.root].flag;out->imports=to_slice(arena,imports);out->procs=to_slice(arena,procs);
  out->constants=to_slice(arena,constants);out->structs=to_slice(arena,structs);out->enums=to_slice(arena,enums);out->unions=to_slice(arena,unions);out->assertions=to_slice(arena,assertions);out->checks=to_slice(arena,checks);
  for(int64_t id=1;id<=state.used;id++)if(nodes[(size_t)id].kind==COAX_PS_Tag_stmt_transaction)out->has_transactions=true;
  laps.lap("parse.tree");
  attach_check_input(out,std::move(nodes),objects,state.used);
  laps.lap("parse.input");
  return true;
}
} // namespace coax
