// Test-only unity access to the actual frontend and Mutator. No alternate
// spatial arithmetic lives here: Coax prepares helpers and C executes them.
#define main coax_driver_entry_for_spatial_native_test
#include "../src/main.cpp"
#undef main
#include <filesystem>
#include <set>
#include <array>
#include <type_traits>

using namespace coax;
namespace {
void require(bool condition, const char *message) {
  if (!condition) { fprintf(stderr,"spatial operator native: %s\n",message); abort(); }
}
struct SavedExpr {
  Expr *node;
  ExprKind kind;
  Type type;
  Tok op, mutation_op;
  ProcDecl *proc, *mutation_proc;
  uint8_t lowering, mutation_status;
  bool generated;
  Expr *lhs, *rhs, *third;
  std::vector<Expr*> args, fields;
  std::array<unsigned char,sizeof(Expr)> bytes;
  std::vector<std::array<unsigned char,sizeof(FieldInit)>> initializer_bytes;
};
static_assert(std::is_trivially_copyable_v<Expr> && std::is_trivially_copyable_v<FieldInit>);
void expressions(Expr *e, std::set<Expr*> &seen, std::vector<SavedExpr> &out) {
  if (!e || !seen.insert(e).second) return;
  SavedExpr s{e,e->kind,e->type,e->op,e->mutation_op,e->proc,e->mutation_proc,
              e->lowering,e->mutation_status,e->compiler_generated,e->lhs,e->rhs,e->third,{},{}};
  for (Expr *a:e->args) s.args.push_back(a);
  for (const FieldInit &f:e->fields) s.fields.push_back(f.value);
  memcpy(s.bytes.data(),e,sizeof(Expr));
  for (const FieldInit &f:e->fields) {
    std::array<unsigned char,sizeof(FieldInit)> bytes;memcpy(bytes.data(),&f,sizeof f);
    s.initializer_bytes.push_back(bytes);
  }
  out.push_back(s);
  expressions(e->lhs,seen,out);expressions(e->rhs,seen,out);expressions(e->third,seen,out);
  for (Expr *a:e->args) expressions(a,seen,out);
  for (const FieldInit &f:e->fields) expressions(f.value,seen,out);
}
void expressions(Stmt *s, std::set<Expr*> &seen, std::vector<SavedExpr> &out) {
  if (!s) return;
  expressions(s->expr,seen,out);expressions(s->expr2,seen,out);expressions(s->target,seen,out);
  for (Stmt *c:s->body) expressions(c,seen,out);
  expressions(s->then_,seen,out);expressions(s->else_,seen,out);
}
void unchanged_except(const std::vector<SavedExpr> &saved, Expr *changed) {
  for (const SavedExpr &s:saved) {
    Expr *e=s.node;
    Expr expected;memcpy(&expected,s.bytes.data(),sizeof expected);
    expected.proc=e==changed?s.mutation_proc:s.proc;
    require(memcmp(e,&expected,sizeof expected)==0,"another expression payload/location/effect/provenance field changed");
    require(e->kind==s.kind && e->type==s.type && e->op==s.op,"source kind/type/token changed");
    require(e->mutation_op==s.mutation_op && e->mutation_proc==s.mutation_proc &&
            e->lowering==s.lowering && e->mutation_status==s.mutation_status &&
            e->compiler_generated==s.generated,"lowering provenance changed");
    require(e->lhs==s.lhs && e->rhs==s.rhs && e->third==s.third,"operand links changed");
    require(e->args.count==(int64_t)s.args.size() && e->fields.count==(int64_t)s.fields.size(),"operand extent changed");
    for (int64_t i=0;i<e->args.count;i++) require(e->args[i]==s.args[(size_t)i],"argument order changed");
    for (int64_t i=0;i<e->fields.count;i++) {
      require(e->fields[i].value==s.fields[(size_t)i],"field expression changed");
      require(memcmp(&e->fields[i],s.initializer_bytes[(size_t)i].data(),sizeof(FieldInit))==0,"resolved initializer metadata changed");
    }
    require(e->proc==(e==changed?s.mutation_proc:s.proc),"another call/shared helper changed");
  }
}
int segment(const std::string &name) {
  const char *labels[]={"pv_add","pv_sub","vv_add","vv_sub","scale","unscale"};
  if (name.size()<2) return -1;
  std::string stem=name.substr(0,name.size()-2);
  for (int i=0;i<6;i++) if (stem==labels[i]) return i;
  return -1;
}
void artifact(const std::filesystem::path &path, const std::string &text) {
  require(write_file(path.string(),text),"could not create emitted artifact");
}
}
int main(int argc,char **argv) {
  require(argc==3,"usage: probe FIXTURE NEW_OUTPUT_DIRECTORY");
  std::filesystem::path output(argv[2]);
  require(!std::filesystem::exists(output),"output directory already exists");
  require(std::filesystem::create_directories(output),"output directory creation failed");
  Source source;Arena arena;File file;EmitOptions opts;opts.library=true;
  require(front_end(argv[1],&source,&arena,&file,opts,1,false,true),"mutation frontend refused fixture");
  std::set<Expr*> seen;std::vector<SavedExpr> saved;
  for (ProcDecl *p:file.procs) expressions(p->body,seen,saved);
  for (const CheckDecl *c:file.checks) expressions(c->proc->body,seen,saved);
  std::vector<std::pair<Type,TypeInfo>> types;
  for (const SavedExpr &s:saved) if (s.type && std::none_of(types.begin(),types.end(),[&](const auto &t){return t.first==s.type;})) types.emplace_back(s.type,*s.type);
  auto types_unchanged=[&] {
    for (const auto &[pointer,before]:types) {
      const TypeInfo &after=*pointer;
      require(after.kind==before.kind && after.name==before.name && after.size==before.size && after.align==before.align && after.bits==before.bits && after.is_signed==before.is_signed && after.elem==before.elem && after.scope==before.scope && after.length==before.length && after.struct_decl==before.struct_decl && after.enum_decl==before.enum_decl && after.physical==before.physical && after.id==before.id,"referenced type description changed");
    }
  };
  size_t eligible=0,no_alternate=0,unsupported=0;
  for (const SavedExpr &s:saved) {
    if (!s.lowering) continue;
    require((s.lowering==1 && (s.mutation_status==1 || s.mutation_status==2)) ||
            (s.lowering==2 && s.mutation_status==3),"source kind/status inventory is inconsistent");
    if (s.mutation_status==1) eligible++;
    else if (s.mutation_status==2) no_alternate++;
    else if (s.mutation_status==3) unsupported++;
    else require(false,"a source operator was silently unprepared");
  }
  require(eligible==12 && no_alternate==6 && unsupported==2,"source operator inventory mismatch");
  Mutator mutator{&arena};
  for (const CheckDecl *c:file.checks) mutator.calls(c->proc->body);
  for (ProcDecl *p:file.procs) {
    if (!p->root_source || std::find(mutator.reached.begin(),mutator.reached.end(),p)==mutator.reached.end()) continue;
    size_t first=mutator.mutants.size();mutator.stmt(p->body);
    for (size_t i=first;i<mutator.mutants.size();i++) mutator.mutants[i].owner=p;
  }
  require(mutator.mutants.size()==12,"expected only twelve source token mutants");
  require(!mutator.invalid_spatial && mutator.spatial_eligible==12 &&
          mutator.spatial_no_alternative==6 && mutator.spatial_unsupported==2,"actual Mutator metadata inventory differs");
  std::string original_c=emit_c(&file,opts),original_h=emit_header(&file);
  auto baseline=output/"baseline";std::filesystem::create_directory(baseline);
  artifact(baseline/"operators.c",original_c);artifact(baseline/"operators.h",original_h);
  std::string manifest="{\"schema\":\"coax.spatial-operator-native-emission.v1\",\"eligible\":12,\"no_alternate\":6,\"unsupported\":2,\"variants\":[";
  std::set<std::pair<int,int>> observed;
  for (size_t i=0;i<mutator.mutants.size();i++) {
    Mutant &m=mutator.mutants[i];require(m.spatial && m.owner && m.owner->root_source,"mutant lost spatial source owner");
    std::string owner=to_std(m.owner->name);int part=segment(owner);int width=owner.ends_with("32")?32:owner.ends_with("64")?64:0;
    require(part>=0 && width && observed.insert({width,part}).second,"mutant owner/width duplicated");
    const Tok from[]={Tok::plus,Tok::minus,Tok::plus,Tok::minus,Tok::star,Tok::slash};
    const Tok to[]={Tok::minus,Tok::plus,Tok::minus,Tok::plus,Tok::slash,Tok::star};
    require(m.from==from[part] && m.to==to[part],"source token pair differs from the independent owner table");
    m.apply();Expr *target=nullptr;size_t changed=0;
    for (const SavedExpr &s:saved) if (s.node->proc!=s.proc) {target=s.node;changed++;}
    require(changed==1 && target && target->lowering==1 && target->mutation_status==1,"apply did not change exactly one source binary callee");
    require(target->proc==target->mutation_proc,"callee does not match Coax-selected alternate");
    require(target->proc->ret==target->type,"alternate result type differs");
    auto old=std::find_if(saved.begin(),saved.end(),[&](const SavedExpr &s){return s.node==target;});
    require(old!=saved.end() && target->proc!=old->proc,"alternate is original helper");
    require(m.from==old->op && m.to==target->mutation_op,"mutant token identity differs from Coax preparation");
    require(target->proc->params.count==old->proc->params.count,"alternate arity differs");
    for (int64_t p=0;p<target->proc->params.count;p++) require(target->proc->params[p].type==old->proc->params[p].type,"alternate parameter type differs");
    unchanged_except(saved,target);
    types_unchanged();
    auto directory=output/("variant-"+std::to_string(i));std::filesystem::create_directory(directory);
    std::string changed_c=emit_c(&file,opts);require(changed_c!=original_c,"applied mutant emitted original C");
    artifact(directory/"operators.c",changed_c);artifact(directory/"operators.h",emit_header(&file));
    require(emit_header(&file)==original_h,"mutation changed public header");
    m.undo();unchanged_except(saved,nullptr);
    types_unchanged();
    require(emit_c(&file,opts)==original_c && emit_header(&file)==original_h,"undo did not restore exact emission");
    if (i) manifest+=",";
    manifest+="{\"index\":"+std::to_string(i)+",\"owner\":"+json_string(owner)+",\"width\":"+std::to_string(width)+",\"segment\":"+std::to_string(part)+",\"line\":"+std::to_string(m.loc.line)+",\"column\":"+std::to_string(m.loc.col)+",\"site_local_callee_only\":true,\"undo_exact\":true}";
  }
  manifest+="]}";artifact(output/"emission.json",manifest+"\n");
  puts("spatial operators: 12 source-local apply/emit/undo witnesses; 6 no-alternate and 2 unsupported inventoried");
}
