// Offline content migration, never linked into the app. Link against the
// read-only source world's compiler and foliage placement implementation.
#include "haunt_world.h"
#include "foliage/foliage.h"
#include <fstream>
#include <iostream>
#include <map>
#include <cstring>
#include <cmath>
using J=ecs::json::Value;
struct Capture:haunt::Builder {
 std::vector<std::string> names;std::vector<std::array<double,6>> boxes;
 J calls=J::array(), trees=J::array(); int object=0,light=0,actor=0;
 Capture(const char* path){std::ifstream f(path,std::ios::binary);std::vector<char>b((std::istreambuf_iterator<char>(f)),{});uint32_t n,v;memcpy(&n,b.data()+4,4);memcpy(&v,b.data()+8,4);size_t vb=16+n*100,ib=vb+v*44;
  for(uint32_t k=0;k<n;k++){size_t a=16+k*100;names.emplace_back(b.data()+a);uint32_t first,count;memcpy(&first,b.data()+a+68,4);memcpy(&count,b.data()+a+72,4);std::array<double,6> box={1e9,1e9,1e9,-1e9,-1e9,-1e9};for(uint32_t i=0;i<count;i++){uint32_t ix;memcpy(&ix,b.data()+ib+(first+i)*4,4);float p[3];memcpy(p,b.data()+vb+ix*44,12);for(int j=0;j<3;j++)box[j]=std::min(box[j],double(p[j])),box[j+3]=std::max(box[j+3],double(p[j]));}boxes.push_back(box);}
  for(int w:{4,6,8})for(auto s:{"Plain_Roof_","Plain_Gable_"}){names.push_back(std::string(s)+std::to_string(w));boxes.push_back({-w/2.-.3,-.35,-1,w/2.+.3,.55*w+.3,1});}
 }
 int mesh(const std::string& n)override{for(size_t i=0;i<names.size();i++)if(names[i]==n)return int(i);return -1;}
 int clip(const std::string&)override{return 0;}
 bool bounds(int m,double*out)override{if(m<0||m>=int(boxes.size()))return false;std::copy(boxes[m].begin(),boxes[m].end(),out);return true;}
 J command(const std::string& n,const J&a)override{J row=J::array();row.items={J::string(n),a};J ans=J::object();ans.set("ok",J::boolean(true));if(n=="object-add")ans.set("id",J::number(object++));if(n=="light-add")ans.set("id",J::number(light++));if(n=="actor-add")ans.set("id",J::number(actor++));calls.items.push_back(row);
  if(n=="foliage-layer"){foliage::Layer l;foliage::fromJson(*a.find("layer"),l);foliage::World w;w.species=[](const std::string&){foliage::SpeciesInfo i;i.known=true;i.trunk=0;return i;};for(auto t:foliage::place(l,w)){J o=J::object();o.set("mesh",J::string(l.species[t.species].model));o.set("x",J::number(t.x));o.set("y",J::number(t.y));o.set("z",J::number(t.z));o.set("yaw",J::number(t.yaw));o.set("scale",J::number(t.scale));J tint=J::array();for(float c:t.tint)tint.items.push_back(J::number(c));o.set("tint",tint);o.set("collide",J::boolean(l.collide));trees.items.push_back(o);}}
  return ans;}
};
int main(int argc,char**argv){if(argc!=4)return 2;Capture c(argv[1]);std::ifstream f(argv[2]);std::string text((std::istreambuf_iterator<char>(f)),{});J doc;if(!ecs::json::parse(text,doc))return 3;J handles=haunt::buildWorld(doc,c),out=J::object(),names=J::array();for(auto&n:c.names)names.items.push_back(J::string(n));out.set("meshes",names);out.set("calls",c.calls);out.set("handles",handles);out.set("trees",c.trees);std::ofstream(argv[3])<<ecs::json::write(out)<<'\n';std::cout<<c.object<<" objects; "<<c.trees.items.size()<<" trees\n";}
