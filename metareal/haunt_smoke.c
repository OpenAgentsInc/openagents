// Actual bundled protobuf/geometry through generated Coax, under sanitizers.
#include "Haunt.h"
#include "HauntBuffer.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <stdbool.h>

#include <math.h>
static coax_v_u8 readfile(const char* dir,const char* name){char path[1024];snprintf(path,sizeof path,"%s/%s",dir,name);FILE*f=fopen(path,"rb");assert(f);fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);uint8_t*b=malloc(n);assert(fread(b,1,n,f)==(size_t)n);fclose(f);return (coax_v_u8){b,n};}
#define SPAN(s,f,T,n) (s).f=(coax_v_t_##T){calloc(n,sizeof(coax_t_##T)),n}
static coax_t_Haunt storage(void){coax_t_Haunt s={0};SPAN(s,meshes,HauntMesh,256);SPAN(s,entities,HauntEntity,4096);SPAN(s,areas,HauntArea,64);SPAN(s,walks,HauntRect,256);SPAN(s,lights,HauntLight,63);SPAN(s,people,HauntPerson,64);SPAN(s,zones,HauntZone,32);SPAN(s,actions,HauntAction,256);SPAN(s,pending,HauntEvent,256);s.talking=-1;s.selected=-1;s.holding=-1;return s;}
static void release(coax_t_Haunt s){free(s.meshes.data);free(s.entities.data);free(s.areas.data);free(s.walks.data);free(s.lights.data);free(s.people.data);free(s.zones.data);free(s.actions.data);free(s.pending.data);}
static int eq(coax_string s,const char*t){return s.count==(int64_t)strlen(t)&&!memcmp(s.data,t,s.count);}
static int find(coax_t_Haunt s,const char*t){for(int i=0;i<s.entity_count;i++)if(eq(s.entities.data[i].id,t))return i;assert(0);return -1;}
static coax_string str(const char*t){return(coax_string){t,strlen(t)};}
int main(int argc,char**argv){assert(argc==2||argc==3);coax_v_u8 b=readfile(argv[1],"Haunt.pb"),v=readfile(argv[1],"Haunt.vertices"),i=readfile(argv[1],"Haunt.indices");assert(metareal_haunt_buffer_ok(v,i,12));uint8_t saved=i.data[3];i.data[3]=255;assert(!metareal_haunt_buffer_ok(v,i,12));i.data[3]=saved;
 coax_t_Haunt s=metareal_haunt_open(b,storage());assert(s.ok&&s.entity_count==1645&&s.mesh_count==87&&s.area_count==16&&s.people_count==4&&s.zone_count==9&&s.light_count==26);int count=s.entity_count;printf("admitted %lld instances, %lld rooms/areas, %lld zones\n",s.entity_count,s.area_count,s.zone_count);
 coax_t_HauntInstance*instances=calloc(4096,sizeof(*instances));coax_t_HauntDraw*draws=calloc(256,sizeof(*draws));coax_t_HauntGPUlight*lights=calloc(64,sizeof(*lights));int n=metareal_haunt_instances(s,0,(coax_v_t_HauntInstance){instances,4096},(coax_v_t_HauntDraw){draws,256});assert(n>50);for(int k=0;k<n;k++){assert(draws[k].first+draws[k].count<=(uint64_t)i.count/4);assert(draws[k].base+draws[k].instances<=4096);}assert(metareal_haunt_lights(s,0,(coax_v_t_HauntGPUlight){lights,64})==28);
 int sheet=find(s,"sheet-1"),ghost=find(s,"ghost-sheet"),brew=find(s,"cauldron-brew");assert(s.entities.data[ghost].hidden);
 for(int z=0;z<s.zone_count;z++){coax_t_HauntPoint p=s.zones.data[z].pos;p.y=0;s=metareal_haunt_advance(s,z*10,p,p,0,0,true);s=metareal_haunt_advance(s,z*10+1,p,p,0,0,true);assert(s.fired&(1u<<z));}
 assert(s.entities.data[sheet].hidden&&!s.entities.data[ghost].hidden);assert(eq(s.meshes.data[s.entities.data[brew].mesh].name,"Custom_Ghost"));assert(s.fired==511);
 s=metareal_haunt_talk(s,0,0);assert(s.reply.count>20);s=metareal_haunt_ask(s,str("Who are you?"));assert(eq(s.reply,"Mortimer"));s=metareal_haunt_teach(s,0,str("The key is in the kitchen."));s=metareal_haunt_ask(s,str("Where is the key?"));assert(eq(s.reply,"The key is in the kitchen."));
 int pumpkin=find(s,"pumpkin-plain");float x=s.entities.data[pumpkin].pos.x;s=metareal_haunt_edit(s,pumpkin,0,.5f);assert(fabs(s.entities.data[pumpkin].pos.x-x-.5f)<.001f);
 s=metareal_haunt_add(s,s.entities.data[pumpkin].mesh,str("placed-test"),str("Pumpkin"),7,10);assert(s.entity_count==count+1);
 uint8_t*person_a=calloc(8192,1),*person_b=calloc(8192,1);
 s=metareal_haunt_add_person(s,str("person-test"),str("Ada"),str("Custom_Fig_Guest0"),(coax_v_u8){person_a,8192},(coax_v_u8){person_b,8192});assert(s.people_count==5&&s.entity_count==count+2);
 s=metareal_haunt_person_action(s,4,0);assert(eq(s.people.data[4].brain,"follower"));
 s=metareal_haunt_zone_edit(s,0,0,0);assert(!s.zones.data[0].once);
 uint8_t*a=calloc(262144,1),*tmp=calloc(262144,1);int64_t bytes=metareal_haunt_save(s,(coax_v_u8){a,262144},(coax_v_u8){tmp,262144});assert(bytes>0&&bytes<262144);printf("saved %lld protobuf bytes\n",bytes);
 coax_t_Haunt copy=metareal_haunt_open(b,storage());copy=metareal_haunt_restore(copy,(coax_v_u8){a,bytes});assert(copy.entity_count==count+2&&copy.fired==511);assert(copy.people_count==5&&eq(copy.people.data[4].name,"Ada")&&eq(copy.people.data[4].brain,"follower"));assert(!copy.zones.data[0].once);assert(fabs(copy.entities.data[find(copy,"pumpkin-plain")].pos.x-x-.5f)<.001f);assert(eq(copy.people.data[0].knowledge,"The key is in the kitchen."));
 FILE*f=fopen("/tmp/haunt-save-oracle.pb","wb");assert(fwrite(a,1,bytes,f)==(size_t)bytes);fclose(f);
 uint8_t bad[]={8,99};coax_t_Haunt refused=metareal_haunt_restore(copy,(coax_v_u8){bad,2});assert(refused.entity_count==copy.entity_count&&refused.fired==copy.fired);
 if(argc==3){FILE*oracle=fopen(argv[2],"rb");assert(oracle);uint8_t*back=calloc(262144,1);size_t size=fread(back,1,262144,oracle);fclose(oracle);assert(metareal_haunt_save_valid(copy,(coax_v_u8){back,size}));copy=metareal_haunt_restore(copy,(coax_v_u8){back,size});assert(copy.entity_count==count+2&&copy.people_count==5&&copy.fired==511);assert(eq(copy.title,"Official protoc")&&eq(copy.feedback,"Independent writer")&&eq(copy.instance_id,"oracle-world"));free(back);}
 free(person_a);free(person_b);free(a);free(tmp);free(instances);free(draws);free(lights);release(s);release(copy);free(b.data);free(v.data);free(i.data);puts("Haunted House gameplay, all nine triggers, rendering packets, teaching, editing and save round trip passed");}
