// Test-only unity seam to the actual immutable frontend/Mutator implementation.
#define main coax_driver_entry_for_panda_spatial_token
#include "../src/main.cpp"
#undef main
#include <filesystem>
#include <array>
#include <set>
#include <type_traits>
using namespace coax;
namespace {
void need(bool ok,const char*message){if(!ok){fprintf(stderr,"Panda spatial token: %s\n",message);abort();}}
template<class T>struct Saved {T*pointer;std::array<unsigned char,sizeof(T)>bytes;Saved(T*p):pointer(p){static_assert(std::is_trivially_copyable_v<T>);memcpy(bytes.data(),p,sizeof(T));}};
struct Tree {
 std::set<Expr*>seen_expr;std::set<Stmt*>seen_stmt;std::vector<Saved<Expr>>exprs;std::vector<Saved<Stmt>>stmts;std::vector<Saved<ProcDecl>>procs;std::vector<Saved<FieldInit>>fields;
 void visit(Expr*e){if(!e||!seen_expr.insert(e).second)return;exprs.emplace_back(e);visit(e->lhs);visit(e->rhs);visit(e->third);for(Expr*a:e->args)visit(a);for(FieldInit&f:e->fields){fields.emplace_back(&f);visit(f.value);}}
 void visit(Stmt*s){if(!s||!seen_stmt.insert(s).second)return;stmts.emplace_back(s);visit(s->expr);visit(s->expr2);visit(s->target);for(Stmt*c:s->body)visit(c);visit(s->then_);visit(s->else_);}
 void verify(Expr*changed=nullptr){
  for(auto&s:exprs){Expr expected;memcpy(&expected,s.bytes.data(),sizeof expected);if(s.pointer==changed)expected.proc=expected.mutation_proc;need(!memcmp(&expected,s.pointer,sizeof expected),"non-target expression/argument/effect/provenance changed");}
  for(auto&s:stmts)need(!memcmp(s.bytes.data(),s.pointer,sizeof(Stmt)),"statement/helper body changed");
  for(auto&s:procs)need(!memcmp(s.bytes.data(),s.pointer,sizeof(ProcDecl)),"procedure/shared helper changed");
  for(auto&s:fields)need(!memcmp(s.bytes.data(),s.pointer,sizeof(FieldInit)),"initializer metadata changed");
 }
};
void artifact(const std::filesystem::path&p,const std::string&s){need(write_file(p.string(),s),"artifact write failed");}
}
int main(int argc,char**argv){
 need(argc==3,"usage: probe FIXTURE NEW_DIRECTORY");std::filesystem::path out(argv[2]);need(!std::filesystem::exists(out),"output exists");need(std::filesystem::create_directories(out),"mkdir failed");
 Source source;Arena arena;File file;EmitOptions opts;opts.library=true;need(front_end(argv[1],&source,&arena,&file,opts,1,false,true),"mutation frontend refused");
 CheckDecl*selected=nullptr;for(CheckDecl*c:file.checks)if(to_std(c->name)=="Panda source spatial token keeps independently known zero-angle geometry"){need(!selected,"duplicate new check");selected=c;}need(selected,"missing new check");
 CheckOutcome baseline=run_check(&file,selected,1000000);need(baseline.passed&&baseline.cases==1,"new baseline check failed");
 ProcDecl*owner=nullptr;for(ProcDecl*p:file.procs)if(to_std(p->name)=="pfk_shift"){need(!owner,"duplicate target procedure");owner=p;}need(owner&&!owner->root_source,"target must be unchanged source dependency");
 Mutator mutator{&arena};mutator.calls(selected->proc->body);need(std::find(mutator.reached.begin(),mutator.reached.end(),owner)!=mutator.reached.end(),"new test does not reach target");
 mutator.stmt(owner->body);std::vector<Mutant*>chosen;size_t skipped=0;for(Mutant&m:mutator.mutants){m.owner=owner;if(m.spatial)chosen.push_back(&m);else skipped++;}
 need(chosen.size()==1&&mutator.spatial_eligible==1&&!mutator.invalid_spatial,"exactly one target spatial-token mutant required");Mutant&m=*chosen[0];need(m.owner==owner&&m.from==Tok::plus&&m.to==Tok::minus,"wrong owner/token pair");
 std::string original=emit_c(&file,opts),header=emit_header(&file);Tree tree;for(ProcDecl*p:file.procs){tree.procs.emplace_back(p);tree.visit(p->body);}for(CheckDecl*c:file.checks)tree.visit(c->proc->body);
 auto base=out/"baseline",variant=out/"mutant";std::filesystem::create_directory(base);std::filesystem::create_directory(variant);artifact(base/"panda-token.c",original);artifact(base/"panda-token.h",header);
 m.apply();Expr*target=nullptr;size_t changed=0;for(auto&s:tree.exprs){Expr before;memcpy(&before,s.bytes.data(),sizeof before);if(s.pointer->proc!=before.proc){target=s.pointer;changed++;}}
 need(changed==1&&target&&target->proc==target->mutation_proc&&target->lowering==1&&target->mutation_status==1,"callee swap was not source-local");
 tree.verify(target);need(target->proc->ret==target->type,"alternate result type");for(int i=0;i<target->args.count;i++)need(target->proc->params[i].type==target->args[i]->type,"alternate operand type");
 std::string mutated=emit_c(&file,opts);need(mutated!=original&&emit_header(&file)==header,"mutation C/header identity");artifact(variant/"panda-token.c",mutated);artifact(variant/"panda-token.h",emit_header(&file));
 CheckOutcome killed=run_check(&file,selected,1000000);need(!killed.passed,"new consumer did not kill actual token mutant");tree.verify(target);
 m.undo();tree.verify();need(emit_c(&file,opts)==original&&emit_header(&file)==header,"undo did not restore byte-exact C/header");
 SourcePoint point=source_point(&source,m.loc);need(std::filesystem::path(point.path).filename()=="panda-fk-math.coax","wrong authored source location");
 std::string record="{\"schema\":\"coax.panda-spatial-token.emission.v1\",\"passed\":true,\"owner\":\"pfk_shift\",\"selected_spatial_mutants\":1,\"killed\":1,\"survived\":0,\"unexecuted_nonspatial_target_sites\":"+std::to_string(skipped)+",\"old_dependency_checks_executed\":0,\"new_check_cases\":1,\"new_check_baseline_steps\":"+std::to_string(baseline.steps)+",\"new_check_mutant_steps\":"+std::to_string(killed.steps)+",\"source_line\":"+std::to_string(point.loc.line)+",\"source_column\":"+std::to_string(point.loc.col)+",\"snapshot_expressions\":"+std::to_string(tree.exprs.size())+",\"snapshot_statements\":"+std::to_string(tree.stmts.size())+",\"snapshot_procedures\":"+std::to_string(tree.procs.size())+",\"site_local_callee_only\":true,\"shared_helpers_unchanged\":true,\"undo_c_header_exact\":true}\n";artifact(out/"emission.json",record);
 puts("Panda spatial token: one actual pfk_shift source mutant killed; unchanged helpers and byte-exact undo");
}
