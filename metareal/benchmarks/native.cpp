// Native benchmark adapter and independent reference reader. Runtime behavior
// belongs to the original pinned evaluator, not a generated-Coax snapshot.
#include "Cast.h"
#include "Player.h"
#include "Benchmarks.h"
#include "reference/anim/anim.h"
#include <algorithm>
#include <cassert>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <new>
#include <vector>

static thread_local bool observe_alloc=false;
static thread_local uint64_t allocations=0;
void* operator new(size_t n){if(observe_alloc)++allocations;if(void*p=std::malloc(n?n:1))return p;throw std::bad_alloc();}
void* operator new[](size_t n){return ::operator new(n);}
void operator delete(void*p)noexcept{std::free(p);}void operator delete[](void*p)noexcept{std::free(p);}
extern "C" void*r0_malloc(size_t n){if(observe_alloc)++allocations;return std::malloc(n);}
extern "C" void*r0_calloc(size_t n,size_t s){if(observe_alloc)++allocations;return std::calloc(n,s);}
extern "C" void*r0_realloc(void*p,size_t n){if(observe_alloc)++allocations;return std::realloc(p,n);}
extern "C" void r0_free(void*p){std::free(p);}

static uint32_t word(const std::vector<uint8_t>&b,size_t at){assert(at+4<=b.size());uint32_t v;std::memcpy(&v,b.data()+at,4);return v;}
static float scalar(const std::vector<uint8_t>&b,size_t at){uint32_t bits=word(b,at);float v;std::memcpy(&v,&bits,4);assert(std::isfinite(v));return v;}
struct Span{size_t offset,count,stride;};
static Span span(const std::vector<uint8_t>&b,const char*name){
 assert(word(b,0)==0x3143524d && word(b,4)==1 && word(b,8)==14);
 for(size_t i=0;i<14;++i){size_t p=16+i*32;assert(p+32<=b.size());if(!std::strncmp((const char*)b.data()+p,name,16)){
  Span s{word(b,p+16),word(b,p+28),word(b,p+24)};assert(s.offset+s.count*s.stride<=b.size());return s;}}
 std::abort();
}
static anim::Data reference(const std::vector<uint8_t>&b){
 auto sk=span(b,"skeleton"),poses=span(b,"poses"),clips=span(b,"clips");
 assert(sk.count==65 && sk.stride==16 && poses.stride==32 && clips.count==7 && clips.stride==8);
 anim::Data d;d.joints=(int)sk.count;d.bodies=1;d.framesPerBody=(uint32_t)(poses.count/sk.count);d.fps=30;
 d.texels.resize(poses.count*16);std::memcpy(d.texels.data(),b.data()+poses.offset,poses.count*32);
 for(size_t i=0;i<sk.count;++i){auto p=(int32_t)word(b,sk.offset+i*16);assert(p<(int)i && p>=-1);d.parent.push_back(p);d.bind.push_back({scalar(b,sk.offset+i*16+4),scalar(b,sk.offset+i*16+8),scalar(b,sk.offset+i*16+12)});}
 for(size_t i=0;i<clips.count;++i){auto first=word(b,clips.offset+i*8),count=word(b,clips.offset+i*8+4);assert(count && first+count<=d.framesPerBody);d.clips.push_back({"",first,count});}
 d.moves.resize(d.joints);d.masks.resize(anim::kMaskCount*d.joints);std::fill_n(d.masks.begin(),d.joints,1.f);return d;
}
static float phase(int move,double start,double now){float rate=move==1?1.6f:(move==2 || move==3)?1.2f:1.f;return (float)(now-start)*30.f*rate;}
static bool loops(int move){return move!=3 && move!=5;}
static anim::Pose pose(coax_t_Anim a,double now){float w=a.fade_start<0?0:std::clamp(1.f-(float)(now-a.fade_start)/.18f,0.f,1.f);return anim::Pose::clip(a.move,phase(a.move,a.start,now),a.from,phase(a.from,a.from_start,now),w,loops(a.move),loops(a.from));}
static void place(const std::vector<anim::Rigid>&in,coax_t_BenchFrame f,std::vector<coax_t_Joint>&out){
 float c=std::cos(f.face),s=std::sin(f.face);
 for(size_t j=0;j<in.size();++j){auto&r=in[j];Vec3 columns[3]={anim::rotate(r.q,{1,0,0}),anim::rotate(r.q,{0,1,0}),anim::rotate(r.q,{0,0,1})};
  for(int k=0;k<3;++k){out[j].m.e[k]=c*columns[k].x+s*columns[k].z;out[j].m.e[4+k]=columns[k].y;out[j].m.e[8+k]=-s*columns[k].x+c*columns[k].z;}
  out[j].m.e[3]=c*r.t.x+s*r.t.z+f.x;out[j].m.e[7]=r.t.y+f.y;out[j].m.e[11]=-s*r.t.x+c*r.t.z+f.z;
 }
}
using Clock=std::chrono::steady_clock;
template<class F>static double measure(F body,uint64_t&count){allocations=0;observe_alloc=true;auto begin=Clock::now();body();auto end=Clock::now();observe_alloc=false;count+=allocations;return std::chrono::duration<double,std::nano>(end-begin).count();}
static coax_t_BenchStats stats(const std::vector<double>&samples){std::vector<double>scratch(samples.size());auto s=metareal_bench_stats({const_cast<double*>(samples.data()),(int64_t)samples.size()},{scratch.data(),(int64_t)scratch.size()});assert(s.ok);return s;}
static void print(const char*name,const std::vector<double>&values,uint64_t allocations){auto s=stats(values);std::printf("\"%s\":{\"count\":%lld,\"median_ns\":%.17g,\"p95_ns\":%.17g,\"mean_ns\":%.17g,\"cv\":%.17g,\"allocations\":%llu,\"samples_ns\":[",name,s.count,s.median,s.p95,s.mean,s.cv,(unsigned long long)allocations);for(size_t i=0;i<values.size();++i)std::printf("%s%.17g",i?",":"",values[i]);std::printf("]}");}
static void native_stats_cases(){
 double values[2]={1,3},scratch[2]={};
 auto call=[&](int64_t count,int64_t capacity){return metareal_bench_stats({values,count},{scratch,capacity});};
 assert(call(2,2).ok && call(2,2).cv==.5 && call(2,2).median==2);
 assert(!call(-1,2).ok && !call(0,2).ok && !call(65537,65537).ok && !call(2,1).ok);
 for(double bad:{NAN,INFINITY,-INFINITY}){values[0]=bad;assert(!call(2,2).ok);}
}
int main(int argc,char**argv){
 native_stats_cases();assert(argc==2 || argc==3);int repeats=argc==3?std::atoi(argv[2]):9;assert(repeats>0 && repeats<=100);
 FILE*f=std::fopen(argv[1],"rb");assert(f);std::fseek(f,0,SEEK_END);long length=std::ftell(f);assert(length>0);std::rewind(f);std::vector<uint8_t>b(length);assert(std::fread(b.data(),1,b.size(),f)==b.size());std::fclose(f);
 auto c=metareal_cast_open({b.data(),(int64_t)b.size()});assert(c.ok && c.joints==65);auto data=reference(b);
 std::vector<coax_t_Joint>palette(metareal_cast_palette(c)),native(c.joints);
 std::vector<float>bodies((size_t)(metareal_cast_robots(c)*c.robot_bodies.count*7));int64_t body_count=0;std::vector<anim::Rigid>rigid;rigid.reserve(c.joints);
 auto player=metareal_player_place(0,0,0,0);auto animation=coax_t_Anim{};animation.fade_start=-1;
 auto plan=metareal_bench_plan();assert(plan.warmup==180 && plan.samples==540);
 std::vector<double>coax,native_time,robots,movement,clock;size_t samples=(size_t)repeats*plan.samples;
 for(auto*v:{&coax,&native_time,&robots,&movement,&clock})v->reserve(samples);
 double max_error=0;uint64_t c_alloc=0,n_alloc=0,r_alloc=0,p_alloc=0,ignored=0;volatile double sink=0;
 for(int repeat=-1;repeat<repeats;++repeat){
  int frames=(int)(repeat<0?plan.warmup:plan.samples);
  for(int i=0;i<frames;++i){int frame=repeat<0?i:(int)plan.warmup+repeat*(int)plan.samples+i;auto fixture=metareal_bench_frame(frame);if(fixture.dance)animation=metareal_cast_dance(animation);
   animation=metareal_cast_update({b.data(),(int64_t)b.size()},c,animation,fixture.act,fixture.rising,fixture.now);auto layers=pose(animation,fixture.now);
   uint64_t ca=0,na=0,ra=0,pa=0;double ct=0,nt=0;
   auto coax_call=[&]{assert(metareal_cast_pose({b.data(),(int64_t)b.size()},c,animation,fixture.now,fixture.x,fixture.y,fixture.z,fixture.face,{palette.data(),(int64_t)palette.size()}));};
   auto native_call=[&]{anim::evaluateAll(data,0,layers,0,rigid);place(rigid,fixture,native);};
   if((i+repeat)&1){ct=measure(coax_call,ca);nt=measure(native_call,na);}else{nt=measure(native_call,na);ct=measure(coax_call,ca);}
   for(int64_t j=0;j<c.joints;++j)for(int k=0;k<12;++k){double error=std::abs((double)palette[j].m.e[k]-native[j].m.e[k]);assert(std::isfinite(error));max_error=std::max(max_error,error);}
   auto rt=measure([&]{assert(metareal_cast_robots_pose({b.data(),(int64_t)b.size()},c,fixture.now,{palette.data(),(int64_t)palette.size()}));body_count=metareal_cast_bodies({b.data(),(int64_t)b.size()},c,{palette.data(),(int64_t)palette.size()},{bodies.data(),(int64_t)bodies.size()});assert(body_count>0);},ra);
   auto pt=measure([&]{player=metareal_player_input(player,0,0,0,fixture.pad_y,fixture.run,fixture.rising,0,0);player=metareal_player_advance_among(player,1.f/60.f,{bodies.data(),body_count*7},{nullptr,0});},pa);
   auto overhead=measure([]{},ignored);sink+=palette[0].m.e[0]+player.pos.x;
   if(repeat>=0){coax.push_back(ct);native_time.push_back(nt);robots.push_back(rt);movement.push_back(pt);clock.push_back(overhead);c_alloc+=ca;n_alloc+=na;r_alloc+=ra;p_alloc+=pa;}
  }
 }
 assert(max_error<=0.0002 && c_alloc==0 && r_alloc==0 && p_alloc==0 && std::isfinite(sink));
 auto cs=stats(coax),ns=stats(native_time);
 std::printf("{\"schema_version\":1,\"joints\":%lld,\"robot_joints\":%lld,\"max_palette_error\":%.17g,\"coax_native_median_ratio\":%.17g,\"proposal_5_percent_met\":%s,\"metrics\":{",c.joints,(int64_t)palette.size()-c.joints,max_error,cs.median/ns.median,cs.median/ns.median<=1.05?"true":"false");
 print("coax_pose",coax,c_alloc);std::printf(",");print("native_pose",native_time,n_alloc);std::printf(",");print("robots_and_bodies",robots,r_alloc);std::printf(",");print("player_collision",movement,p_alloc);std::printf(",");print("clock",clock,0);std::printf("}}\n");
}
