/* Focused direct-token parser/ABI oracle; no product grammar lives here. */
#include "parser.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t checks, cases;
#define CHECK(c) do { ++checks; if (!(c)) { fprintf(stderr,"frame-parser native failure line %d case %zu\n",__LINE__,cases); exit(1); } } while (0)
static coax_string str(const char *s) { return (coax_string){s,(int64_t)strlen(s)}; }
static bool same(coax_string s,const char *text) { return s.count==(int64_t)strlen(text) && (!s.count || memcmp(s.data,text,(size_t)s.count)==0); }
static coax_t_PS_Token token(uint32_t kind,const char *text,int64_t at) { return (coax_t_PS_Token){.kind=kind,.text=str(text),.loc={.line=1,.column=at+1,.offset=at},.raw_count=(int64_t)strlen(text)}; }
static void fixture(coax_t_PS_Token *t,const char *const words[4],int shift) {
 static const char *names[]={"handedness","convention","continuity","min_precision"};
 memset(t,0,64*sizeof(*t));t[0]=token(1,"Frame",0);t[1]=token(17,"::",1);t[2]=token(2,"struct",2);t[3]=token(7,"frame",3);t[4]=token(8,"(",4);
 for (int i=0;i<4;i++) { int f=(i+shift)%4,at=5+4*i;t[at]=token(1,names[f],at);t[at+1]=token(23,"=",at+1);t[at+2]=token(1,words[f],at+2);if (i!=3)t[at+3]=token(14,",",at+3); }
 t[20]=token(9,")",20);t[21]=token(10,"{",21);t[22]=token(11,"}",22);
}
static void run(coax_t_PS_Token *t,int64_t count,const int64_t *want,const char *code) {
 coax_t_PS_Node n[96];coax_t_PS_State state[1];uint8_t message[512];
 memset(n,0xa5,sizeof(n));memset(state,0xa5,sizeof(state));memset(message,0xa5,sizeof(message));++cases;
 coax_t_PS_Result r=PS_parse(str("direct-token test fixture"),(coax_v_t_PS_Token){t,count},(coax_v_t_PS_Node){n,96},(coax_v_t_PS_State){state,1},(coax_v_u8){message,512});
 CHECK(r.ok==(code==NULL));CHECK(same(state[0].code,code?code:""));
 if (code) { CHECK(state[0].loc.line==1);CHECK(state[0].loc.column>0);return; }
 CHECK(r.root>0 && r.root<96);int64_t id=n[r.root].head;CHECK(id>0 && id<96);CHECK(n[id].kind==COAX_PS_Tag_decl_struct_);CHECK(same(n[id].name,"Frame"));CHECK(n[id].quantity==0);CHECK(n[id].frame_decl);CHECK(n[id].head==0 && n[id].d==0);
 for(int i=0;i<4;i++)CHECK(n[id].frame.e[i]==want[i]);for(int i=0;i<6;i++)CHECK(n[id].unit.e[i]==0);
}
int main(void) {
 static const char *hands[]={"right","left"},*conventions[]={"cartesian","body","optical"},*continuities[]={"continuous","jumping"},*precisions[]={"f32","f64"};
 coax_t_PS_Token t[64];
 for(int h=0;h<2;h++)for(int c=0;c<3;c++)for(int j=0;j<2;j++)for(int p=0;p<2;p++)for(int shift=0;shift<4;shift++) {
  const char *words[]={hands[h],conventions[c],continuities[j],precisions[p]};int64_t want[]={h?-1:1,c+1,j+1,p?64:32};fixture(t,words,shift);run(t,24,want,NULL);
 }
 const char *words[]={"right","cartesian","continuous","f32"};
 for(int field=0;field<4;field++) {
  for(int kind=0;kind<8;kind++) {fixture(t,words,0);static const uint32_t kinds[]={0,2,3,4,5,7,35,53};t[7+4*field].kind=kinds[kind];run(t,24,NULL,"E0832");}
  fixture(t,words,0);t[7+4*field].text=str("not_a_literal");run(t,24,NULL,"E0832");
  fixture(t,words,0);t[5+4*field].text=str("unknown_metadata");run(t,24,NULL,"E0832");
  fixture(t,words,0);t[20]=token(14,",",20);t[21]=t[5+4*field];t[22]=token(23,"=",22);t[23]=t[7+4*field];t[24]=token(9,")",24);t[25]=token(10,"{",25);t[26]=token(11,"}",26);run(t,28,NULL,"E0832");
 }
 fixture(t,words,0);t[21]=token(7,"frame",21);run(t,24,NULL,"E0832");
 fixture(t,words,0);t[21]=token(7,"unit",21);run(t,24,NULL,"E0832");
 fixture(t,words,0);t[21]=token(7,"builtin_quantity",21);run(t,24,NULL,"E0832");
 fixture(t,words,0);t[4].kind=23;run(t,24,NULL,"E0201");
 fixture(t,words,0);t[6].kind=14;run(t,24,NULL,"E0201");
 fixture(t,words,0);t[20].kind=10;run(t,24,NULL,"E0201");
 printf("{\"schema\":\"coax-frame-parser-native-v1\",\"cases\":%zu,\"assertions\":%zu,\"passed\":true}\n",cases,checks);return 0;
}
