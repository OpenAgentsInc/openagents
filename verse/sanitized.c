#include "host.h"
#include "Home.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char**argv){
  assert(argc==2);FILE*f=fopen(argv[1],"rb");assert(f);
  unsigned char bytes[1025];size_t n=fread(bytes,1,sizeof(bytes),f);fclose(f);
  int64_t h=mr_create(bytes,n,(size_t)verse_home_capacity());assert(h&&verse_home_init(h));
  for(int i=0;i<1200;i++){
    mr_clear_frame(h);verse_home_frame(h,i/60.,1206,2622,3,102,false);assert(mr_ok(h));
    if(i%30==0){const MRBar*b=mr_bar(h);double x=b->x+b->width/2,y=b->y+b->height/2;
      assert(!verse_home_touch(h,0,x,y));assert(verse_home_touch(h,2,x,y));
      assert(mr_output_size(h)>0&&mr_output_size(h)<512);
    }
  }
  // The Start bar alone, as drawn over the Grid's overview: laid out with no
  // graph, and pressing it still starts.
  for(int i=0;i<240;i++){
    mr_clear_frame(h);verse_home_hud_frame(h,30+i/60.,1206,2622,3,102);assert(mr_ok(h));
    assert(mr_dot_count(h)==0&&mr_line_count(h)==0);
    const MRBar*b=mr_bar(h);assert(b->width>1000&&b->y+b->height<=2622-102);
    if(i==120){double x=b->x+b->width/2,y=b->y+b->height/2;
      assert(!verse_home_touch(h,0,x,y));assert(verse_home_touch(h,2,x,y));}
  }
  mr_destroy(h);assert(!mr_ok(h));
  // All bounded parser prefixes, plus malformed continuation/length noise.
  for(size_t i=1;i<n;i++){
    h=mr_create(bytes,i,(size_t)verse_home_capacity());assert(h);assert(!verse_home_init(h));mr_destroy(h);
  }
  unsigned char malformed[1025];memset(malformed,255,sizeof(malformed));
  for(size_t i=1;i<=sizeof(malformed);i++){
    h=mr_create(malformed,i,(size_t)verse_home_capacity());assert(h);assert(!verse_home_init(h));mr_destroy(h);
  }
  puts("Coax home ASan/UBSan passed");
}
