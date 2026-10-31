/* Firm reason: independent oracle (2). Fixed route tables and exhaustive
   eligibility masks qualify emitted Coax and its C ABI; no product effects. */
#include "guide-game.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static coax_string text(const char *s) { return (coax_string){s,(int64_t)strlen(s)}; }
static coax_t_GgpFacts base(void) {
    return (coax_t_GgpFacts){.followed=0,.stage=1,.guard=0,.help_requested=true,
        .gameplay=true,.foreground=true,.player_alive=true,.guard_calm=true,
        .guard_alive=true,.guard_full_health=true,.guard_visible=true,.guard_near=true,
        .world_known=true,.target_present=true,.destination_present=true};
}
int main(void) {
    unsigned checks=0; const int route[3][3]={{2,2,5},{3,3,5},{4,4,5}};
    const int tags[3][3]={{17,18,19},{33,34,35},{49,50,51}};
    for (int q=0;q<3;q++) for (int stage=1;stage<=3;stage++) {
        coax_t_GgpFacts f=base();f.followed=q;f.stage=stage;
        assert(ggp_goal(q,stage,false)==route[q][stage-1] && ggp_project(f).goal==route[q][stage-1]);
        assert(ggp_offer(f)==1 && ggp_route(f) && ggp_goal_key(q,stage)==tags[q][stage-1]);f.find_work=true;
        assert(ggp_goal(q,stage,true)==0 && !ggp_route(f) && ggp_offer(f)==1);checks++;
    }
    const int64_t badq[]={INT64_MIN,-2,-1,3,4,INT64_MAX}, badstage[]={INT64_MIN,-1,0,4,9,INT64_MAX};
    for (unsigned i=0;i<sizeof badq/sizeof *badq;i++) for (int stage=1;stage<=3;stage++) {assert(ggp_goal(badq[i],stage,false)==0 && ggp_goal_key(badq[i],stage)==-1);checks++;}
    for (unsigned i=0;i<sizeof badstage/sizeof *badstage;i++) for (int q=0;q<3;q++) {assert(ggp_goal(q,badstage[i],false)==0 && ggp_goal_key(q,badstage[i])==-1);checks++;}
    assert(ggp_goal_key(-1,0)==0);checks++;
    for (unsigned bits=0;bits<4;bits++) {assert(ggp_world_ready((bits&1)!=0,(bits&2)!=0)==(bits==3));checks++;}
    for (unsigned bits=0;bits<(1u<<15);bits++) {
        coax_t_GgpFacts f=base();
        f.gameplay=(bits&1)!=0;f.foreground=(bits&2)!=0;f.player_alive=(bits&4)!=0;f.guard_calm=(bits&8)!=0;
        f.guard_alive=(bits&16)!=0;f.guard_full_health=(bits&32)!=0;f.guard_visible=(bits&64)!=0;f.guard_near=(bits&128)!=0;
        f.combat=(bits&256)!=0;f.talking=(bits&512)!=0;f.board=(bits&1024)!=0;f.editing=(bits&2048)!=0;
        f.casting=(bits&4096)!=0;f.map=(bits&8192)!=0;f.warp=(bits&16384)!=0;
        bool eligible=bits==255;coax_t_GgObservation o=ggp_project(f);
        assert(ggp_eligible(f)==eligible && o.eligible==eligible && ggp_offer(f)==(eligible?1:0));
        assert(o.goal==2 && o.help_requested && o.destination_available && !o.hint_visible && !o.at_destination);checks++;
    }
    for (unsigned i=0;i<sizeof badq/sizeof *badq;i++) {if (badq[i]==0) continue;coax_t_GgpFacts f=base();f.guard=badq[i];assert(!ggp_eligible(f));checks++;}
    for (unsigned bits=0;bits<256;bits++) {
        coax_t_GgpFacts f=base();f.world_known=(bits&1)==0;f.edit_count=(bits&2)?1:0;f.gone=(bits&4)?UINT32_MAX:0;
        f.hidden=(bits&8)?UINT32_C(0x80000000):0;f.altered_geometry=(bits&16)!=0;
        f.target_present=(bits&32)==0;f.destination_present=(bits&64)==0;f.at_destination=(bits&128)!=0;
        assert(ggp_route(f)==(bits==0) && ggp_offer(f)==(bits==0?1:0) && ggp_project(f).destination_available==(bits==0));checks++;
    }
    coax_t_GgpFacts f=base();f.help_requested=false;f.followed=-1;f.stage=0;
    assert(ggp_offer(f)==2 && ggp_project(f).goal==0 && !ggp_project(f).help_requested);
    f.find_work=true;assert(ggp_project(f).goal==1 && ggp_offer(f)==2 && ggp_route(f));
    f.hint_visible=true;assert(ggp_offer(f)==0 && ggp_project(f).hint_visible);checks++;
    const int64_t counters[]={INT64_MIN,-1,0,1,99,2147483646,2147483647,2147483648,INT64_MAX-1,INT64_MAX};
    const int64_t next[]={INT64_MIN,-1,1,2,100,2147483647,2147483647,2147483648,INT64_MAX-1,INT64_MAX};
    for (unsigned i=0;i<sizeof counters/sizeof *counters;i++) {assert(ggp_next_intent(counters[i])==next[i]);checks++;}
    const int intent_edge[]={8,8,8,8,8,8,8,9};
    for (unsigned bits=0;bits<8;bits++) {assert(ggp_intent(8,(bits&1)!=0,(bits&2)!=0,(bits&4)?11:10,11)==intent_edge[bits]);checks++;}
    assert(ggp_intent(0,true,true,INT64_MIN,INT64_MAX)==0 && ggp_intent(0,true,true,INT64_MAX,INT64_MIN)==0);checks++;
    for (unsigned i=0;i<sizeof counters/sizeof *counters;i++) {assert(ggp_intent(counters[i],true,true,2,2)==next[i]);checks++;}
    const char *hex="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    char viewer[16385];memset(viewer,'\n',sizeof viewer);memcpy(viewer,"H|",2);memcpy(viewer+2,hex,64);
    assert(ggp_viewer((coax_string){viewer,66}) && ggp_viewer((coax_string){viewer,16384}) && !ggp_viewer((coax_string){viewer,16385}));
    for (int i=2;i<66;i++) {char saved=viewer[i];viewer[i]='G';assert(!ggp_viewer((coax_string){viewer,66}));viewer[i]=saved;checks++;}
    char duplicate[200];snprintf(duplicate,sizeof duplicate,"H|%s\nH|%s\n",hex,hex);assert(!ggp_viewer(text(duplicate)));
    assert(!ggp_viewer(text("A|1\nK|npub-test\n")) && !ggp_viewer((coax_string){NULL,0}));checks++;
    const char *good[]={"200\n{\"version\":0}","200\n{\"doc\":null,\"version\":0}",
        "200\n{\"doc\":{\"edits\":\"eg-edit 1\\n\",\"everglade\":1},\"version\":2,\"publishedBy\":\"test\",\"publishedAt\":1.5}"};
    const char *bad[]={"200\n{\"version\":false}","200\n{\"version\":\"0\"}","200\n{\"version\":-0}",
        "200\n{\"version\":0,\"doc\":{}}","200\n{\"version\":1,\"doc\":null}",
        "200\n{\"version\":0,\"version\":1}","200\n{\"x\":{\"version\":0},\"version\":1,\"doc\":{\"edits\":\"eg-edit 1\\n\"}}",
        "200\n{\"version\":1,\"doc\":{\"edits\":\"\",\"edits\":\"\"}}","200\n{\"version\":0,\"publishedBy\":[]}"};
    for (unsigned i=0;i<sizeof good/sizeof *good;i++) {assert(ggp_world_answer(text(good[i])));checks++;}
    for (unsigned i=0;i<sizeof bad/sizeof *bad;i++) {assert(!ggp_world_answer(text(bad[i])));checks++;}
    printf("guard game native: %u independent route/mask/intent/viewer/world cases passed\n",checks);return 0;
}
