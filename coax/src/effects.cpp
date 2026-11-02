// Resolved checked-tree observations and JSON transport. Effect classification,
// reachability, cycle detection and witness selection belong to effects.coax.
#include "coax.h"
#include <map>
#include <set>
#include <algorithm>

namespace coax {
#ifndef COAX_BOOTSTRAP_IMPORTER
namespace {
struct EffectFacts {
  const File *file;
  std::vector<const ProcDecl *> procedures;
  std::map<const ProcDecl *,int64_t> ids;
  std::vector<coax_t_EF_Edge> edges;
  std::vector<Loc> edge_locations;
  std::vector<coax_t_EF_Site> sites;
  std::vector<Loc> site_locations;
  std::set<const Expr *> seen;
  std::set<Type> defaults_seen;
  int64_t current = 0;
  void site(int64_t kind,Loc loc) { sites.push_back({current,kind});site_locations.push_back(loc); }
  static bool borrowed_place(const Expr *e) {
    if(!e)return false;
    if(e->kind==ExprKind::index && e->lhs && e->lhs->type &&
       (e->lhs->type->kind==TypeKind::view || e->lhs->type->kind==TypeKind::string_))return true;
    return (e->kind==ExprKind::field || e->kind==ExprKind::index) && borrowed_place(e->lhs);
  }
  static bool borrowed_type(Type type,std::set<Type> &seen_types) {
    if(!type || !seen_types.insert(type).second)return false;
    if(type->kind==TypeKind::view || type->kind==TypeKind::string_)return true;
    if(type->kind==TypeKind::array)return borrowed_type(type->elem,seen_types);
    if(type->kind==TypeKind::struct_ && type->struct_decl)for(const auto &f:type->struct_decl->fields)if(borrowed_type(f.type,seen_types))return true;
    if(type->kind==TypeKind::result && borrowed_type(type->sum_error,seen_types))return true;
    if((type->kind==TypeKind::result || type->kind==TypeKind::optional) && borrowed_type(type->elem,seen_types))return true;
    if(type->kind==TypeKind::union_ && type->union_decl)for(const auto &v:type->union_decl->variants)if(borrowed_type(v.type,seen_types))return true;
    return false;
  }
  void defaults(Type type) {
    if(!type || !defaults_seen.insert(type).second)return;
    if(type->kind==TypeKind::array)defaults(type->elem);
    if(type->kind==TypeKind::struct_ && type->struct_decl)
      for(const auto &field:type->struct_decl->fields){expression(field.init);defaults(field.type);}
  }
  void expression(const Expr *e,bool place=false) {
    if(!e)return;
    // Places may also be read later; do not let a write suppress that fact.
    if(!place && !seen.insert(e).second)return;
    if(!e->type || e->type->kind==TypeKind::invalid){site(3,e->loc);return;}
    if(e->kind==ExprKind::ct_value)return; // Former compile-time calls are not runtime edges.
    if(e->own_effects&2)site(4,e->loc);
    if(e->own_effects&4)site(10,e->loc); // Conditional overflow; Coax applies arithmetic profile.
    switch(e->kind) {
      case ExprKind::int_lit:case ExprKind::float_lit:case ExprKind::bool_lit:
      case ExprKind::string_lit:case ExprKind::enum_value:return;
      case ExprKind::ident:if(e->constant)expression(e->constant->expr);return;
      case ExprKind::run:site(3,e->loc);return;
      case ExprKind::call:
        if(e->builtin_print)site(1,e->loc);
        if(e->byte_intrinsic==ByteIntrinsic::bytes_overlap)site(9,e->loc);
        if(e->byte_intrinsic!=ByteIntrinsic::none)site(12,e->loc);
        if(e->intrinsic!=Intrinsic::none)site(7,e->loc);
        if(e->proc){auto it=ids.find(e->proc);edges.push_back({current,it==ids.end()?-1:it->second});edge_locations.push_back(e->loc);}
        else if(!e->builtin_print && e->byte_intrinsic==ByteIntrinsic::none && e->intrinsic==Intrinsic::none)site(3,e->loc);
        for(const Expr *arg:e->args)expression(arg);return;
      case ExprKind::index:
        if(!place && borrowed_place(e))site(5,e->loc);
        expression(e->lhs,place);expression(e->rhs);return;
      case ExprKind::field:expression(e->lhs,place);return;
      case ExprKind::struct_lit:
        defaults(e->type);for(const auto &field:e->fields)expression(field.value);return;
      case ExprKind::array_lit:for(const Expr *arg:e->args)expression(arg);return;
      case ExprKind::cast:case ExprKind::unary:expression(e->lhs);return;
      case ExprKind::binary:expression(e->lhs);expression(e->rhs);return;
      case ExprKind::slice:case ExprKind::ifx:expression(e->lhs);expression(e->rhs);expression(e->third);return;
      case ExprKind::ct_value:return;
      case ExprKind::sum:case ExprKind::some:case ExprKind::try_:
        // v1 has no recoverable-flow/active-variant contract. Preserve all
        // actual child effects and report its unsupported semantics as unknown.
        site(3,e->loc);expression(e->lhs);return;
      case ExprKind::null_:site(3,e->loc);return;
      case ExprKind::or_else:
        site(3,e->loc);expression(e->lhs);expression(e->rhs);return;
    }
    site(3,e->loc);
  }
  void statement(const Stmt *s) {
    if(!s)return;
    switch(s->kind){case StmtKind::block:case StmtKind::expr:case StmtKind::var:case StmtKind::assign:case StmtKind::ret:case StmtKind::if_:case StmtKind::while_:case StmtKind::for_range:case StmtKind::for_each:case StmtKind::break_:case StmtKind::continue_:case StmtKind::defer:case StmtKind::assert_:break;default:site(3,s->loc);}
    if(s->kind==StmtKind::assert_)site(4,s->loc);
    if(s->kind==StmtKind::for_each && s->expr && s->expr->type && (s->expr->type->kind==TypeKind::view || s->expr->type->kind==TypeKind::string_))site(5,s->loc);
    if(s->own_effects&2)site(4,s->loc);
    if(s->own_effects&4)site(10,s->loc);
    if(s->kind==StmtKind::var){defaults(s->type);if(s->uninit)site(8,s->loc);}
    if(s->kind==StmtKind::assign && borrowed_place(s->target))site(6,s->loc);
    expression(s->expr);expression(s->expr2);
    expression(s->target,s->kind==StmtKind::assign && s->op==Tok::equal);
    for(const Stmt *child:s->body)statement(child);
    for(const SwitchCase *c:s->cases){expression(c->enum_pattern);statement(c->body);}
    statement(s->then_);statement(s->else_); // Includes defer bodies and every syntactic branch.
  }
  void collect() {
    for(const ProcDecl *p:file->procs){ids.emplace(p,(int64_t)procedures.size());procedures.push_back(p);}
    for(const CheckDecl *c:file->checks)if(c->proc && !ids.count(c->proc)){ids.emplace(c->proc,(int64_t)procedures.size());procedures.push_back(c->proc);}
    for(size_t i=0;i<procedures.size();i++){
      current=(int64_t)i;const auto *p=procedures[i];seen.clear();defaults_seen.clear();
      std::set<Type> return_types;if(borrowed_type(p->ret,return_types))site(11,p->loc);
      if(p->foreign)site(2,p->loc);else if(!p->body)site(3,p->loc);else statement(p->body);
    }
  }
  std::string location(Loc loc) const {
    auto p=source_point(file->src,loc);const auto *u=source_unit_at(file->src,loc);
    return "{\"unit\":"+(u?json_string(u->canonical):std::string("null"))+",\"line\":"+std::to_string(p.loc.line)+",\"column\":"+std::to_string(p.loc.col)+"}";
  }
  static bool overlap(const void *a,size_t an,const void *b,size_t bn) {
    if(!an||!bn)return false;auto x=(uintptr_t)a,y=(uintptr_t)b;return x<=y?y-x<an:x-y<bn;
  }
  bool aliases(const std::vector<int64_t> &p,const std::vector<int64_t> &v,const std::vector<int64_t> &q,const std::vector<int64_t> &d,const std::vector<int64_t> &w)const {
    std::pair<const void *,size_t> spans[]={{p.data(),p.size()*8},{v.data(),v.size()*8},{q.data(),q.size()*8},{d.data(),d.size()*8},{w.data(),w.size()*8},{edges.data(),edges.size()*sizeof(coax_t_EF_Edge)},{sites.data(),sites.size()*sizeof(coax_t_EF_Site)}};
    for(size_t i=0;i<5;i++)for(size_t j=i+1;j<7;j++)if(overlap(spans[i].first,spans[i].second,spans[j].first,spans[j].second))return true;
    return false;
  }
};
} // namespace
#endif

static bool effects_report(const Source *,const File *file,const EmitOptions &opts,const std::string &selected,std::string *text) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)opts;(void)selected;(void)text;
  fprintf(stderr,"coax: effects inventory is unavailable in the bootstrap compiler\n");return false;
#else
  EffectFacts f{file};f.collect();const size_t n=f.procedures.size();
  std::vector<int64_t> parent(n),via(n),queue(n),degree(n),witness(9);
  std::vector<size_t> roots;
  for(size_t i=0;i<n;i++)if(selected.empty()||to_std(f.procedures[i]->name)==selected)roots.push_back(i);
  if(!selected.empty()&&roots.empty()){fprintf(stderr,"coax: effects procedure '%s' was not found\n",selected.c_str());return false;}
  size_t uninstantiated=0;for(const ProcDecl *t:file->generics){bool used=false;for(const ProcDecl *p:file->procs)used=used||p->origin==t;if(!used)uninstantiated++;}
  const char *names[]={"host_io","foreign_call","unknown_effects","may_trap","borrowed_read","borrowed_write","intrinsic_resources_unqualified","uninitialized_storage","address_observation"};
  std::string out="{\"schema\":\"coax.effects.v1\",\"status\":\"inspected\",\"profile\":{\"arithmetic\":"+json_string(opts.wrapping?"wrapping":"checked")+"},\"scope\":\"conservative checked-AST may-effects, not R11 completion or bounded admission\",\"unknown_policy\":\"foreign resources, intrinsic implementations and borrowed regions remain unqualified; absence of a bit is not an allocation or blocking proof\",\"uninstantiated_templates\":"+std::to_string(uninstantiated)+",\"generic_templates\":"+std::to_string(file->generics.count)+",\"procedure_count\":"+std::to_string(n)+",\"edge_count\":"+std::to_string(f.edges.size())+",\"site_count\":"+std::to_string(f.sites.size());
  out+=",\"sources\":[";bool source_comma=false;for(const auto &u:file->src->units){out+=(source_comma?",":"")+std::string("{\"unit\":")+json_string(u->canonical)+",\"sha256\":"+json_string(debug_source_hash(u->text))+"}";source_comma=true;}out+="],\"procedures\":[";
  bool comma=false;
  for(size_t root:roots){
    auto r=EF_trace((int64_t)n,{f.edges.data(),(int64_t)f.edges.size()},{f.sites.data(),(int64_t)f.sites.size()},(int64_t)root,
      {parent.data(),(int64_t)n},{via.data(),(int64_t)n},{queue.data(),(int64_t)n},{degree.data(),(int64_t)n},{witness.data(),9},opts.wrapping,f.aliases(parent,via,queue,degree,witness));
    if(r.status){fprintf(stderr,"coax: effects inventory refused graph/capacity status %u\n",r.status);return false;}
    const auto *p=f.procedures[root];out+=(comma?",":"")+std::string("{\"id\":")+std::to_string(root)+",\"name\":"+json_string(to_std(p->name))+",\"instance_key\":"+json_string(to_std(p->key))+",\"declaration\":"+f.location(p->loc)+",\"generic_instance\":"+(p->origin?"true":"false")+",\"effects_mask\":"+std::to_string(r.effects)+",\"reachable_procedures\":"+std::to_string(r.reached)+",\"reachable_recursion\":"+(r.recursive?"true":"false")+",\"acyclic_removed\":"+std::to_string(r.acyclic_removed)+",\"effects\":[";comma=true;
    bool bit_comma=false;
    for(int bit=0;bit<9;bit++)if(r.effects&EF_bit(bit)){
      size_t at=(size_t)witness[(size_t)bit];const auto &site=f.sites.at(at);std::vector<int64_t> path;
      for(int64_t node=site.procedure;node!=-1;node=parent.at((size_t)node))path.push_back(node);
      std::reverse(path.begin(),path.end());out+=(bit_comma?",":"")+std::string("{\"kind\":")+json_string(names[bit])+",\"site_kind\":"+std::to_string(site.kind)+",\"site\":"+f.location(f.site_locations[at])+",\"path\":[";bit_comma=true;
      bool path_comma=false;for(int64_t node:path){const auto *callee=f.procedures[(size_t)node];out+=(path_comma?",":"")+std::string("{\"procedure\":")+std::to_string(node)+",\"name\":"+json_string(to_std(callee->name))+",\"instance_key\":"+json_string(to_std(callee->key))+",\"declaration\":"+f.location(callee->loc);if(via[(size_t)node]>=0)out+=",\"call\":"+f.location(f.edge_locations[(size_t)via[(size_t)node]]);out+="}";path_comma=true;}out+="]}";
    }
    out+="]}";
  }
  out+="]}\n";*text=std::move(out);return true;
#endif
}
} // namespace coax
