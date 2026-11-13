// Independent contracts around the actual Coax producer. Numeric vertex parity
// with the separately repaired C++ oracle is checked by mesh-check.c.
#include "IceCream.h"
#include <inttypes.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define REQUIRE(q) do {if(!(q)){fprintf(stderr,"band check failed at%d: %s\n",__LINE__,#q);return 1;}}while(0)
static uint32_t bits(float x){uint32_t n;memcpy(&n,&x,4);return n;}
static int same(coax_t_Vtx a,coax_t_Vtx b){return !memcmp(&a,&b,sizeof a);}
int main(void){
 _Static_assert(sizeof(coax_t_Vtx)==44,"eleven recorded four-byte vertex words");
 const coax_t_Vtx marker={.p={91,92,93},.n={94,95,96},.hull={97,98,99},.colour=0xabcdef,.weight=100};
 uint64_t cases=0,words=0;
 for(int orientation=0;orientation<4;orientation++)for(int base=0;base<8;base++)for(int mode=0;mode<3;mode++){
  coax_t_Vtx full[128],short_view[128],trim[72],middle[36],original[128],post[36];
  for(int j=0;j<128;j++)full[j]=marker;
  REQUIRE(icecream_band_native((coax_v_t_Vtx){full,128},base,orientation,-2.25f,4.75f)==base+108);
  memcpy(original,full,sizeof full);
  if(mode){
   REQUIRE(icecream_open_band_native((coax_v_t_Vtx){full,128},base,orientation,-2.25f,4.75f,mode==2)==base+108);
   REQUIRE(icecream_piece_native((coax_v_t_Vtx){post,36},0,orientation,mode==2?4.75f-.12f:-2.25f,mode==2?4.75f:-2.25f+.12f,0,2.1f,0x778899,.06f)==36);
   float donor_top=-INFINITY;for(int j=0;j<36;j++)donor_top=fmaxf(donor_top,post[j].p.y);
   for(int j=0;j<108;j++){
    coax_t_Vtx before=original[base+j],after=full[base+j];float normal=orientation%2==0?before.n.x:before.n.z;int opening=normal*(mode==2?1:-1)>.5f;
    // Only the opening-facing wall/lower-trim positions can change.
    coax_t_Vtx attributes=after;attributes.p=before.p;REQUIRE(same(attributes,before));
    if(!opening||j>=72)REQUIRE(same(before,after));
    if(opening&&j<36){REQUIRE(after.p.y>=donor_top);REQUIRE(bits(after.p.x)==bits(before.p.x)&&bits(after.p.z)==bits(before.p.z));}
   }
   for(int j=36;j<72;j+=3){float normal=orientation%2==0?original[base+j].n.x:original[base+j].n.z;
    if(normal*(mode==2?1:-1)>.5f){REQUIRE(!memcmp(&full[base+j].p,&full[base+j+1].p,sizeof full[base+j].p));REQUIRE(!memcmp(&full[base+j].p,&full[base+j+2].p,sizeof full[base+j].p));}}
  }
  REQUIRE(icecream_piece_native((coax_v_t_Vtx){trim,72},0,orientation,-2.25f,4.75f,0,.28f,0x445566,.05f)==36);
  REQUIRE(icecream_piece_native((coax_v_t_Vtx){trim,72},36,orientation,-2.25f,4.75f,2.6f-.22f,2.6f,0x445566,.05f)==72);
  REQUIRE(icecream_piece_native((coax_v_t_Vtx){middle,36},0,orientation,-2.25f,4.75f,.28f,2.6f-.22f,0x112233,0)==36);
  float bottom=-INFINITY,top=INFINITY;
  for(int j=0;j<36;j++){bottom=fmaxf(bottom,trim[j].p.y);top=fminf(top,trim[j+36].p.y);}
  REQUIRE(bottom>.27f&&bottom<.29f&&top>2.37f&&top<2.39f);
  for(int j=0;j<36;j++){
   REQUIRE(bits(original[base+j].p.y)==bits(bottom)||bits(original[base+j].p.y)==bits(top));
   middle[j].p.y=original[base+j].p.y;REQUIRE(same(original[base+j],middle[j]));
   REQUIRE(same(original[base+36+j],trim[j]));REQUIRE(same(original[base+72+j],trim[36+j]));
  }
  for(int cap=0;cap<=base+111;cap++){
   for(int j=0;j<128;j++)short_view[j]=marker;
   REQUIRE((mode?icecream_open_band_native((coax_v_t_Vtx){short_view,cap},base,orientation,-2.25f,4.75f,mode==2):icecream_band_native((coax_v_t_Vtx){short_view,cap},base,orientation,-2.25f,4.75f))==base+108);
   for(int j=0;j<128;j++){
    int written=j>=base&&j<base+108&&base+((j-base)/3)*3+3<=cap;
    REQUIRE(same(short_view[j],written?full[j]:marker));words+=11;
   }
   cases++;
  }
 }
 printf("bands: %" PRIu64 " short-view cases, %" PRIu64 " vertex words, exact seams/visible surfaces/canaries pass\n",cases,words);
 return 0;
}
