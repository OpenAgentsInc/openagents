// Semantic decisions are Coax. This file transports the indexed syntax graph,
// allocates native representations and copies checked results for the evaluator.
#include "coax.h"
#ifdef COAX_BOOTSTRAP_IMPORTER
#include "checker.h"
#include "parser.h"
#else
#include "proto_importer.h"
#endif
namespace coax {
static_assert((uint8_t)TypeKind::invalid == COAX_CK_Kind_invalid, "checker type ABI: invalid");
static_assert((uint8_t)TypeKind::void_ == COAX_CK_Kind_void_, "checker type ABI: void_");
static_assert((uint8_t)TypeKind::integer == COAX_CK_Kind_integer, "checker type ABI: integer");
static_assert((uint8_t)TypeKind::floating == COAX_CK_Kind_floating, "checker type ABI: floating");
static_assert((uint8_t)TypeKind::bool_ == COAX_CK_Kind_bool_, "checker type ABI: bool_");
static_assert((uint8_t)TypeKind::string_ == COAX_CK_Kind_string_, "checker type ABI: string_");
static_assert((uint8_t)TypeKind::untyped_int == COAX_CK_Kind_untyped_int, "checker type ABI: untyped_int");
static_assert((uint8_t)TypeKind::untyped_float == COAX_CK_Kind_untyped_float, "checker type ABI: untyped_float");
static_assert((uint8_t)TypeKind::struct_ == COAX_CK_Kind_struct_, "checker type ABI: struct_");
static_assert((uint8_t)TypeKind::enum_ == COAX_CK_Kind_enum_, "checker type ABI: enum_");
static_assert((uint8_t)TypeKind::array == COAX_CK_Kind_array, "checker type ABI: array");
static_assert((uint8_t)TypeKind::view == COAX_CK_Kind_view, "checker type ABI: view");
static_assert((uint8_t)TypeKind::param == COAX_CK_Kind_param, "checker type ABI: param");
static_assert(COAX_CK_Tag_type_param == COAX_PS_Tag_type_param, "checker/parser type ABI: param");
static_assert((uint8_t)ExprKind::int_lit == COAX_CK_Tag_expr_int-16, "checker expression ABI: int_lit");
static_assert((uint8_t)ExprKind::float_lit == COAX_CK_Tag_expr_float-16, "checker expression ABI: float_lit");
static_assert((uint8_t)ExprKind::bool_lit == COAX_CK_Tag_expr_bool-16, "checker expression ABI: bool_lit");
static_assert((uint8_t)ExprKind::string_lit == COAX_CK_Tag_expr_string-16, "checker expression ABI: string_lit");
static_assert((uint8_t)ExprKind::ident == COAX_CK_Tag_expr_ident-16, "checker expression ABI: ident");
static_assert((uint8_t)ExprKind::call == COAX_CK_Tag_expr_call-16, "checker expression ABI: call");
static_assert((uint8_t)ExprKind::unary == COAX_CK_Tag_expr_unary-16, "checker expression ABI: unary");
static_assert((uint8_t)ExprKind::binary == COAX_CK_Tag_expr_binary-16, "checker expression ABI: binary");
static_assert((uint8_t)ExprKind::run == COAX_CK_Tag_expr_run-16, "checker expression ABI: run");
static_assert((uint8_t)ExprKind::field == COAX_CK_Tag_expr_field-16, "checker expression ABI: field");
static_assert((uint8_t)ExprKind::index == COAX_CK_Tag_expr_index-16, "checker expression ABI: index");
static_assert((uint8_t)ExprKind::slice == COAX_CK_Tag_expr_slice-16, "checker expression ABI: slice");
static_assert((uint8_t)ExprKind::struct_lit == COAX_CK_Tag_expr_struct-16, "checker expression ABI: struct_lit");
static_assert((uint8_t)ExprKind::array_lit == COAX_CK_Tag_expr_array-16, "checker expression ABI: array_lit");
static_assert((uint8_t)ExprKind::cast == COAX_CK_Tag_expr_cast-16, "checker expression ABI: cast");
static_assert((uint8_t)ExprKind::ifx == COAX_CK_Tag_expr_ifx-16, "checker expression ABI: ifx");
static_assert((uint8_t)ExprKind::enum_value == COAX_CK_Tag_expr_enum-16, "checker expression ABI: enum_value");
static_assert((uint8_t)ExprKind::ct_value == COAX_CK_Tag_expr_ct-16, "checker expression ABI: ct_value");

static_assert(COAX_CK_Tag_stmt_block == COAX_PS_Tag_stmt_block, "checker/parser statement ABI: block");
static_assert(COAX_CK_Tag_stmt_expr == COAX_PS_Tag_stmt_expr, "checker/parser statement ABI: expr");
static_assert(COAX_CK_Tag_stmt_var == COAX_PS_Tag_stmt_var, "checker/parser statement ABI: var");
static_assert(COAX_CK_Tag_stmt_assign == COAX_PS_Tag_stmt_assign, "checker/parser statement ABI: assign");
static_assert(COAX_CK_Tag_stmt_return == COAX_PS_Tag_stmt_ret, "checker/parser statement ABI: return");
static_assert(COAX_CK_Tag_stmt_if == COAX_PS_Tag_stmt_if_, "checker/parser statement ABI: if");
static_assert(COAX_CK_Tag_stmt_while == COAX_PS_Tag_stmt_while_, "checker/parser statement ABI: while");
static_assert(COAX_CK_Tag_stmt_range == COAX_PS_Tag_stmt_for_range, "checker/parser statement ABI: range");
static_assert(COAX_CK_Tag_stmt_each == COAX_PS_Tag_stmt_for_each, "checker/parser statement ABI: each");
static_assert(COAX_CK_Tag_stmt_break == COAX_PS_Tag_stmt_break_, "checker/parser statement ABI: break");
static_assert(COAX_CK_Tag_stmt_continue == COAX_PS_Tag_stmt_continue_, "checker/parser statement ABI: continue");
static_assert(COAX_CK_Tag_stmt_defer == COAX_PS_Tag_stmt_defer, "checker/parser statement ABI: defer");
static_assert(COAX_CK_Tag_stmt_assert == COAX_PS_Tag_stmt_assert_, "checker/parser statement ABI: assert");
static_assert(COAX_CK_Tag_proc == COAX_PS_Tag_decl_proc, "checker/parser declaration ABI: proc");
static_assert(COAX_CK_Tag_constant == COAX_PS_Tag_decl_constant, "checker/parser declaration ABI: constant");
static_assert(COAX_CK_Tag_struct_ == COAX_PS_Tag_decl_struct_, "checker/parser declaration ABI: struct_");
static_assert(COAX_CK_Tag_enum_ == COAX_PS_Tag_decl_enum_, "checker/parser declaration ABI: enum_");
static_assert(COAX_CK_Tag_param == COAX_PS_Tag_decl_param, "checker/parser declaration ABI: param");
static_assert(COAX_CK_Tag_field == COAX_PS_Tag_decl_field, "checker/parser declaration ABI: field");
static_assert(COAX_CK_Tag_member == COAX_PS_Tag_decl_enum_member, "checker/parser declaration ABI: member");
static_assert(COAX_CK_Tag_field_init == COAX_PS_Tag_decl_field_init, "checker/parser declaration ABI: field_init");
static_assert(COAX_CK_Tag_import_ == COAX_PS_Tag_decl_import_, "checker/parser declaration ABI: import_");
static_assert(COAX_CK_Tag_check == COAX_PS_Tag_decl_check, "checker/parser declaration ABI: check");
static std::string checked_text(coax_string s) {return std::string(s.data?s.data:"",(size_t)s.count);}
static TypeInfo checked_scalar(int64_t id) {
  const auto v=CK_builtin(id);TypeInfo t;t.kind=(TypeKind)v.kind;t.name=checked_text(v.name);t.size=(int32_t)v.size;t.align=(int32_t)v.alignment;t.bits=(int32_t)v.bits;t.is_signed=v.is_signed;return t;
}
TypeInfo T_INVALID=checked_scalar(0),T_VOID=checked_scalar(1),T_BOOL=checked_scalar(2),T_STRING=checked_scalar(3);
TypeInfo T_UNTYPED_INT=checked_scalar(4),T_UNTYPED_FLOAT=checked_scalar(5),T_F32=checked_scalar(6),T_F64=checked_scalar(7);
TypeInfo T_S8=checked_scalar(8),T_S16=checked_scalar(9),T_S32=checked_scalar(10),T_S64=checked_scalar(11);
TypeInfo T_U8=checked_scalar(12),T_U16=checked_scalar(13),T_U32=checked_scalar(14),T_U64=checked_scalar(15);
static std::vector<Type> g_composite_list;
const std::vector<Type> &all_composite_types(){return g_composite_list;}
const char *type_name(Type t){return t->name.c_str();}
Intrinsic find_intrinsic(Str name){return (Intrinsic)CK_intrinsic({name.data,name.count});}
ByteIntrinsic find_byte_intrinsic(Str name){return (ByteIntrinsic)CK_byte_intrinsic({name.data,name.count});}
namespace {
bool is_c_keyword(Str name){return CK_c_keyword({name.data,name.count});}
Loc checked_loc(coax_t_CK_Loc l){return {(int32_t)l.line,(int32_t)l.column,(int32_t)l.offset};}
coax_t_CK_Loc checked_loc(coax_t_PS_Loc l){return {l.line,l.column,l.offset};}
Str checked_string(Arena *arena,coax_string s){if(!s.count)return {};char *data=(char*)arena_alloc(arena,(size_t)s.count+1);memcpy(data,s.data,(size_t)s.count);data[s.count]=0;return {data,(int32_t)s.count};}
template<class T> T *checked_ref(const std::vector<void*> &objects,int64_t id){return (T*)objects[(size_t)id];}
template<class T> Slice<T> checked_values(Arena *arena,const std::vector<coax_t_CK_Node> &nodes,const std::vector<void*> &objects,int64_t head){std::vector<T> values;for(auto id=head;id;id=nodes[(size_t)id].next)values.push_back(*checked_ref<T>(objects,id));return to_slice(arena,values);}
template<class T> Slice<T*> checked_pointers(Arena *arena,const std::vector<coax_t_CK_Node> &nodes,const std::vector<void*> &objects,int64_t head){std::vector<T*> values;for(auto id=head;id;id=nodes[(size_t)id].next)values.push_back(checked_ref<T>(objects,id));return to_slice(arena,values);}
}
struct CheckInput {
  std::vector<coax_t_CK_Node> nodes;
  std::vector<void*> objects;
  int64_t used=0;
  // Logical module scopes (0 is the unowned program) and the aliases each scope loads.
  std::vector<std::string> modules;
  std::vector<coax_string> module_names;
  std::vector<std::string> alias_names;
  std::vector<coax_t_CK_Alias> aliases;
  std::vector<std::string> export_names;
  std::vector<coax_t_CK_Export> exports;
};
static int64_t module_scope(CheckInput *input,const SourceUnit *unit){
  if(!unit || unit->module.empty())return 0;
  for(size_t i=1;i<input->modules.size();i++)if(input->modules[i]==unit->module)return (int64_t)i;
  input->modules.push_back(unit->module);return (int64_t)input->modules.size()-1;
}
static void attach_check_input(File *file,const std::vector<coax_t_PS_Node> &nodes,const std::vector<void*> &objects,int64_t used){
  auto input=std::make_shared<CheckInput>();input->used=used;input->objects=objects;input->nodes.resize((size_t)used+1);
  input->modules.push_back("");
  for(const auto &u:file->src->units)module_scope(input.get(),u.get());
  for(const auto &u:file->src->units)for(const auto &a:u->aliases)input->alias_names.push_back(a.name);
  size_t alias_at=0;
  for(const auto &u:file->src->units)for(const auto &a:u->aliases){
    const auto &name=input->alias_names[alias_at++];
    input->aliases.push_back({module_scope(input.get(),u.get()),{name.data(),(int64_t)name.size()},module_scope(input.get(),file->src->units[(size_t)a.target].get())});
  }
  for(const auto &u:file->src->units)for(const auto &e:u->exports)input->export_names.push_back(e.name);
  size_t export_at=0;
  for(const auto &u:file->src->units)for(const auto &e:u->exports){
    const auto &name=input->export_names[export_at++];
    input->exports.push_back({module_scope(input.get(),u.get()),{name.data(),(int64_t)name.size()},{u->first_line+e.loc.line-1,e.loc.col,u->begin+e.loc.offset}});
  }
  for(const auto &m:input->modules)input->module_names.push_back({m.data(),(int64_t)m.size()});
  for(int64_t id=0;id<=used;id++){
    const auto &v=nodes[(size_t)id];auto &n=input->nodes[(size_t)id];n.kind=v.kind;n.loc=checked_loc(v.loc);n.name=v.name;n.name2=v.name2;n.loc2=checked_loc(v.loc2);
    n.a=v.a;n.b=v.b;n.c=v.c;n.d=v.d;n.head=v.head;n.tail=v.tail;n.next=v.next;n.op=v.op;n.flag=v.flag;n.flag2=v.flag2;n.value=v.value;memcpy(&n.floating,&v.floating,8);n.field_index=-1;n.skip_authority=!file->src->units.empty() && !source_authority(file->src,Loc{(int32_t)v.loc.line,(int32_t)v.loc.column,(int32_t)v.loc.offset});
    n.scope=module_scope(input.get(),source_unit_at(file->src,Loc{(int32_t)v.loc.line,(int32_t)v.loc.column,(int32_t)v.loc.offset}));
    if(v.kind==17 && v.width)n.type=v.width==32?6:7;if(v.kind==69)n.field_index=0;
  }
  file->check_input=std::move(input);
}
bool check(Arena *arena,File *file){
  if(!file->check_input){error_at(file->src,{1,1,0},"E0381","type checker has no indexed syntax input");return false;}
  auto input=file->check_input;const auto caps=CK_caps(input->used+1,(int64_t)file->src->text.size());const auto extra=CK_meta_capacity({input->nodes.data(),(int64_t)input->nodes.size()},input->used);input->nodes.resize((size_t)(caps.nodes+extra));
  std::vector<coax_t_CK_Type> records((size_t)caps.types);std::vector<coax_t_CK_Local> locals((size_t)caps.locals);
  std::vector<coax_t_CK_Error> errors((size_t)caps.errors);std::vector<uint8_t> bytes((size_t)caps.bytes);coax_t_CK_State state{};
  coax_t_CK_Context context{{input->nodes.data(),(int64_t)input->nodes.size()},{records.data(),(int64_t)records.size()},{locals.data(),(int64_t)locals.size()},{errors.data(),(int64_t)errors.size()},{bytes.data(),(int64_t)bytes.size()},{&state,1},
    {input->module_names.data(),(int64_t)input->module_names.size()},{input->aliases.data(),(int64_t)input->aliases.size()},{input->exports.data(),(int64_t)input->exports.size()}};
  const bool ok=CK_run(context,input->used,{file->library,file->metal,file->authority});
  if(ok)CK_lower_effects(context);
  for(int64_t i=0;i<state.error_count;i++){const auto &d=errors[(size_t)i];error_at(file->src,checked_loc(d.loc),checked_text(d.code).c_str(),checked_text(d.message),checked_text(d.fix));}
  if(!ok){if(!state.error_count)error_at(file->src,{1,1,0},"E0381","type checker storage budget rejected");return false;}
  auto &nodes=input->nodes;auto &objects=input->objects;const size_t old_size=objects.size();objects.resize((size_t)state.used+1,nullptr);
  for(size_t id=old_size;id<objects.size();id++){const auto k=nodes[id].kind;if(k>=1 && k<=5)objects[id]=make<TypeExpr>(arena);else if(k>=16 && k<=33)objects[id]=make<Expr>(arena);else if(k>=40 && k<=52)objects[id]=make<Stmt>(arena);else if(k==COAX_CK_Tag_proc)objects[id]=make<ProcDecl>(arena);else if(k==COAX_CK_Tag_param)objects[id]=make<Param>(arena);else if(k==COAX_CK_Tag_field_init)objects[id]=make<FieldInit>(arena);}
  std::vector<TypeInfo*> types={&T_INVALID,&T_VOID,&T_BOOL,&T_STRING,&T_UNTYPED_INT,&T_UNTYPED_FLOAT,&T_F32,&T_F64,&T_S8,&T_S16,&T_S32,&T_S64,&T_U8,&T_U16,&T_U32,&T_U64};
  for(int64_t i=16;i<state.type_count;i++)types.push_back(make<TypeInfo>(arena));
  g_composite_list.clear();
  for(int64_t i=16;i<state.type_count;i++){
    const auto &r=records[(size_t)i];auto *t=types[(size_t)i];t->kind=(TypeKind)r.kind;t->name=checked_text(r.name);t->size=(int32_t)r.size;t->align=(int32_t)r.alignment;t->bits=(int32_t)r.bits;t->is_signed=r.is_signed;t->length=r.length;t->id=(int32_t)r.id;t->scope=(int32_t)r.scope;t->elem=types[(size_t)r.elem];
    if(r.kind==COAX_CK_Kind_struct_)t->struct_decl=checked_ref<StructDecl>(objects,r.decl);
    if(r.kind==COAX_CK_Kind_enum_)t->enum_decl=checked_ref<EnumDecl>(objects,r.decl);
  }
  // A composite over a type parameter ([] $T) belongs to a generic procedure's check only.
  for(int64_t i=16;i<state.type_count;i++){const auto &r=records[(size_t)i];if(r.kind!=COAX_CK_Kind_array && r.kind!=COAX_CK_Kind_view)continue;
    // ... and so does a fixed array whose length is a value parameter ([$N] T).
    bool placeholder=r.length<0;auto at=r.elem;while(records[(size_t)at].kind==COAX_CK_Kind_array || records[(size_t)at].kind==COAX_CK_Kind_view){placeholder=placeholder || records[(size_t)at].length<0;at=records[(size_t)at].elem;}
    if(!placeholder && records[(size_t)at].kind!=COAX_CK_Kind_param)g_composite_list.push_back(types[(size_t)i]);}
  // Populate all scalar/reference fields before copying aggregate value lists.
  for(int64_t id=1;id<=state.used;id++){
    const auto &v=nodes[(size_t)id];const auto loc=checked_loc(v.loc);const auto name=checked_string(arena,v.name);const auto ty=types[(size_t)v.type];
    if(v.kind>=1 && v.kind<=5){auto *p=checked_ref<TypeExpr>(objects,id);p->kind=(TypeExpr::Kind)(v.kind-1);p->loc=loc;p->name=name;p->length=v.a && nodes[(size_t)v.a].kind==COAX_CK_Tag_type_param?nullptr:checked_ref<Expr>(objects,v.a);p->elem=checked_ref<TypeExpr>(objects,v.b);}
    else if(v.kind>=16 && v.kind<=33){auto *p=checked_ref<Expr>(objects,id);p->kind=(ExprKind)(v.kind-16);p->loc=loc;p->text=name;p->type=ty;p->int_value=v.value;memcpy(&p->float_value,&v.floating,8);p->op=(Tok)v.op;
      p->effects=v.effects;p->own_effects=v.own_effects;p->lhs=checked_ref<Expr>(objects,v.a);p->rhs=checked_ref<Expr>(objects,v.b);p->third=checked_ref<Expr>(objects,v.c);p->type_expr=checked_ref<TypeExpr>(objects,v.d);p->trunc=v.flag;p->negative=v.negative;p->folded=v.folded;p->compiler_generated=CK_generated(v.mark);p->field_index=(int32_t)v.field_index;p->proc=checked_ref<ProcDecl>(objects,v.proc);p->constant=checked_ref<ConstDecl>(objects,v.constant);p->builtin_print=v.builtin_print;p->byte_intrinsic=(ByteIntrinsic)v.byte_intrinsic;p->intrinsic=(Intrinsic)v.intrinsic;p->args={};p->fields={};}
    else if(v.kind>=40 && v.kind<=52){auto *p=checked_ref<Stmt>(objects,id);p->kind=(StmtKind)(v.kind-40);p->own_effects=v.own_effects;p->loc=loc;p->name=name;p->name2=checked_string(arena,v.name2);p->name2_loc=checked_loc(v.loc2);p->type=ty;p->expr=checked_ref<Expr>(objects,v.a);p->expr2=checked_ref<Expr>(objects,v.b);p->target=nullptr;p->type_expr=nullptr;p->then_=nullptr;p->else_=nullptr;p->uninit=v.flag;
      if(v.kind==COAX_CK_Tag_stmt_var)p->type_expr=checked_ref<TypeExpr>(objects,v.d);
      else if(v.kind==COAX_CK_Tag_stmt_assign){p->target=checked_ref<Expr>(objects,v.c);p->op=(Tok)v.op;}
      else {p->then_=checked_ref<Stmt>(objects,v.c);p->else_=checked_ref<Stmt>(objects,v.d);}}
    else switch(v.kind){
      case COAX_CK_Tag_proc:{auto *p=checked_ref<ProcDecl>(objects,id);p->loc=loc;p->name=name;p->ret_expr=checked_ref<TypeExpr>(objects,v.a);p->body=checked_ref<Stmt>(objects,v.b);p->ret=ty;p->export_c=v.flag;p->foreign=v.flag2;p->generic=v.generic;p->scope=v.collides?(int32_t)v.scope:0;p->key=checked_string(arena,v.key);p->origin=checked_ref<ProcDecl>(objects,v.origin);break;}
      case COAX_CK_Tag_constant:{auto *p=checked_ref<ConstDecl>(objects,id);p->loc=loc;p->name=name;p->expr=checked_ref<Expr>(objects,v.a);p->type=ty;p->checked=(uint8_t)v.mark;break;}
      case COAX_CK_Tag_struct_:{auto *p=checked_ref<StructDecl>(objects,id);p->loc=loc;p->name=name;p->type=types[(size_t)v.type];p->state=(uint8_t)v.mark;break;}
      case COAX_CK_Tag_enum_:{auto *p=checked_ref<EnumDecl>(objects,id);p->loc=loc;p->name=name;p->type=types[(size_t)v.type];p->base_expr=checked_ref<TypeExpr>(objects,v.a);break;}
      case COAX_CK_Tag_param:{auto *p=checked_ref<Param>(objects,id);p->loc=loc;p->name=name;p->type_expr=checked_ref<TypeExpr>(objects,v.a);p->type=ty;break;}
      case COAX_CK_Tag_field:{auto *p=checked_ref<Field>(objects,id);p->loc=loc;p->name=name;p->type_expr=checked_ref<TypeExpr>(objects,v.a);p->init=checked_ref<Expr>(objects,v.b);p->type=ty;p->offset=(int32_t)v.field_index;break;}
      case COAX_CK_Tag_member:{auto *p=checked_ref<EnumMember>(objects,id);p->loc=loc;p->name=name;p->value=checked_ref<Expr>(objects,v.a);p->bits=v.value;break;}
      case COAX_CK_Tag_field_init:{auto *p=checked_ref<FieldInit>(objects,id);p->loc=loc;p->name=name;p->value=checked_ref<Expr>(objects,v.a);p->field=(int32_t)v.field_index;break;}
      case COAX_CK_Tag_import_:{auto *p=checked_ref<Import>(objects,id);p->loc=loc;p->module=name;break;}
      case COAX_CK_Tag_check: {
        auto *p=checked_ref<CheckDecl>(objects,id);
        p->loc=loc;p->name=name;p->forall=v.flag;p->proc=checked_ref<ProcDecl>(objects,v.a);
        std::vector<Binding> domains;
        for(auto b=nodes[(size_t)v.a].head;b;b=nodes[(size_t)b].next) {
          domains.push_back({checked_ref<Expr>(objects,nodes[(size_t)b].b),checked_ref<Expr>(objects,nodes[(size_t)b].c)});
        }
        p->domains=to_slice(arena,domains);break;
      }
    }
  }
  for(int64_t id=1;id<=state.used;id++){const auto &v=nodes[(size_t)id];switch(v.kind){
    case COAX_CK_Tag_expr_call:case COAX_CK_Tag_expr_array:checked_ref<Expr>(objects,id)->args=checked_pointers<Expr>(arena,nodes,objects,v.head);break;
    case COAX_CK_Tag_expr_struct:checked_ref<Expr>(objects,id)->fields=checked_values<FieldInit>(arena,nodes,objects,v.head);break;
    case COAX_CK_Tag_stmt_block:checked_ref<Stmt>(objects,id)->body=checked_pointers<Stmt>(arena,nodes,objects,v.head);break;
    case COAX_CK_Tag_proc:checked_ref<ProcDecl>(objects,id)->params=checked_values<Param>(arena,nodes,objects,v.head);break;
    case COAX_CK_Tag_struct_:checked_ref<StructDecl>(objects,id)->fields=checked_values<Field>(arena,nodes,objects,v.head);break;
    case COAX_CK_Tag_enum_:checked_ref<EnumDecl>(objects,id)->members=checked_values<EnumMember>(arena,nodes,objects,v.head);break;
  }}
  // Generic procedures are templates: only declaration listings see them.
  // Their instances (new proc nodes the checker made) are the procedures that
  // emitters, the evaluator and checks see, after the authored ones.
  std::vector<ProcDecl*> concrete,generic;for(auto *p:file->procs)(p->generic?generic:concrete).push_back(p);
  for(int64_t id=state.instances;id;id=nodes[(size_t)id].chain){auto *p=checked_ref<ProcDecl>(objects,id);const auto *o=p->origin;p->end=o->end;p->authority=o->authority;p->root_source=o->root_source;concrete.push_back(p);}
  file->procs=to_slice(arena,concrete);file->generics=to_slice(arena,generic);
  file->check_input.reset();return true;
}
} // namespace coax
