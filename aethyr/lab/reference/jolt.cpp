// Firm reason: 6 (lab-only Jolt C++ SDK adapter; Coax supplies scenes, metrics and orchestration).
#include <Jolt/Jolt.h>
#include <Jolt/RegisterTypes.h>
#include <Jolt/Core/Factory.h>
#include <Jolt/Core/TempAllocator.h>
#include <Jolt/Core/JobSystemSingleThreaded.h>
#include <Jolt/Physics/PhysicsSystem.h>
#include <Jolt/Physics/Body/BodyCreationSettings.h>
#include <Jolt/Physics/Collision/Shape/BoxShape.h>
#include <Jolt/Physics/Collision/Shape/PlaneShape.h>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <vector>
#include <stdexcept>
using namespace JPH;
static uint64_t u(const std::vector<uint8_t>& b,size_t at){uint64_t v=0;for(int i=0;i<8;i++)v|=uint64_t(b.at(at+i))<<(8*i);return v;}
static double f(const std::vector<uint8_t>& b,size_t at){auto bits=u(b,at);double v;std::memcpy(&v,&bits,8);return v;}
static void put(std::vector<uint8_t>& b,size_t at,uint64_t v){for(int i=0;i<8;i++)b.at(at+i)=uint8_t(v>>(8*i));}
static void putf(std::vector<uint8_t>& b,size_t at,double v){uint64_t bits;std::memcpy(&bits,&v,8);put(b,at,bits);}
class Layers final:public BroadPhaseLayerInterface {public:uint GetNumBroadPhaseLayers()const override{return 2;}BroadPhaseLayer GetBroadPhaseLayer(ObjectLayer l)const override{return BroadPhaseLayer(uint8(l));}};
class Pairs final:public ObjectLayerPairFilter {public:bool ShouldCollide(ObjectLayer a,ObjectLayer b)const override{return a||b;}};
class Vs final:public ObjectVsBroadPhaseLayerFilter {public:bool ShouldCollide(ObjectLayer a,BroadPhaseLayer b)const override{return a||uint8(b);}};
static void snapshot(std::vector<uint8_t>& out,size_t at,int step,BodyInterface& bi,const std::vector<BodyID>& ids){
 put(out,at,step);for(size_t i=0;i<ids.size();i++){RVec3 p;Quat q;bi.GetPositionAndRotation(ids[i],p,q);Vec3 v,w;bi.GetLinearAndAngularVelocity(ids[i],v,w);
 double values[]={p.GetX(),p.GetY(),p.GetZ(),q.GetX(),q.GetY(),q.GetZ(),q.GetW(),v.GetX(),v.GetY(),v.GetZ(),w.GetX(),w.GetY(),w.GetZ()};for(int j=0;j<13;j++)putf(out,at+8+104*i+8*j,values[j]);}}
int main(int argc,char** argv){try{
 if(argc!=3)throw std::runtime_error("usage: lab-jolt INPUT OUTPUT");std::ifstream file(argv[1],std::ios::binary);std::vector<uint8_t>b((std::istreambuf_iterator<char>(file)),{});
 if(b.size()<160||std::memcmp(b.data(),"LABIN001",8))throw std::runtime_error("input");size_t count=u(b,8),steps=u(b,16),every=u(b,24);
 if(count<1||count>30000||steps!=180||every!=30||b.size()!=160+104*count||f(b,104)!=0)throw std::runtime_error("shape");
 RegisterDefaultAllocator();Factory::sInstance=new Factory();RegisterTypes();
 {TempAllocatorImpl allocator(256*1024*1024);JobSystemSingleThreaded jobs(65536);Layers layers;Pairs pairs;Vs vs;PhysicsSystem world;world.Init(32768,0,262144,131072,layers,vs,pairs);
 PhysicsSettings settings;settings.mNumVelocitySteps=uint(u(b,40));settings.mNumPositionSteps=uint(u(b,40));settings.mMinVelocityForRestitution=0.4f;settings.mPenetrationSlop=0.002f;settings.mDeterministicSimulation=true;world.SetPhysicsSettings(settings);
 world.SetGravity(Vec3(float(f(b,56)),float(f(b,64)),float(f(b,72))));auto& bi=world.GetBodyInterface();
 Vec3 normal(float(f(b,80)),float(f(b,88)),float(f(b,96)));RefConst<Shape> ground=new PlaneShape(JPH::Plane(normal.Normalized(),0));
 BodyCreationSettings floor(ground,RVec3::sZero(),Quat::sIdentity(),EMotionType::Static,0);floor.mFriction=float(f(b,144));floor.mRestitution=float(f(b,152));auto floorid=bi.CreateAndAddBody(floor,EActivation::DontActivate);
 RefConst<Shape> box=new BoxShape(Vec3(float(f(b,112)),float(f(b,120)),float(f(b,128))),0);std::vector<BodyID>ids;ids.reserve(count);
 for(size_t i=0;i<count;i++){size_t at=160+104*i;BodyCreationSettings c(box,RVec3(f(b,at),f(b,at+8),f(b,at+16)),Quat(float(f(b,at+24)),float(f(b,at+32)),float(f(b,at+40)),float(f(b,at+48))),EMotionType::Dynamic,1);
 c.mAllowSleeping=false;c.mLinearDamping=0;c.mAngularDamping=0;c.mFriction=float(f(b,144));c.mRestitution=float(f(b,152));c.mOverrideMassProperties=EOverrideMassProperties::CalculateInertia;c.mMassPropertiesOverride.mMass=float(f(b,136));
 c.mLinearVelocity=Vec3(float(f(b,at+56)),float(f(b,at+64)),float(f(b,at+72)));c.mAngularVelocity=Vec3(float(f(b,at+80)),float(f(b,at+88)),float(f(b,at+96)));ids.push_back(bi.CreateAndAddBody(c,EActivation::Activate));if(ids.back().IsInvalid())throw std::runtime_error("body capacity");}
 world.OptimizeBroadPhase();std::vector<uint8_t>out(32+8*steps+(steps/every+1)*(8+104*count));std::memcpy(out.data(),"LABOUT01",8);put(out,8,count);put(out,16,steps);put(out,24,every);
 size_t at=32+8*steps;snapshot(out,at,0,bi,ids);at+=8+104*count;
 for(size_t step=1;step<=steps;step++){auto start=std::chrono::steady_clock::now();auto error=world.Update(float(f(b,48)),1,&allocator,&jobs);auto ns=std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now()-start).count();if(error!=EPhysicsUpdateError::None)throw std::runtime_error("update capacity");put(out,32+8*(step-1),uint64_t(ns));if(step%every==0){snapshot(out,at,int(step),bi,ids);at+=8+104*count;}}
 std::ofstream result(argv[2],std::ios::binary);result.write(reinterpret_cast<const char*>(out.data()),std::streamsize(out.size()));if(!result)throw std::runtime_error("write");
 for(auto id:ids){bi.RemoveBody(id);bi.DestroyBody(id);}bi.RemoveBody(floorid);bi.DestroyBody(floorid);}
 UnregisterTypes();delete Factory::sInstance;Factory::sInstance=nullptr;return 0;
 }catch(const std::exception&e){fprintf(stderr,"lab-jolt: %s\n",e.what());return 1;}}
