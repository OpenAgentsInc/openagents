/* Firm reason: independent oracle (2). Recomputes route masks, result counts,
   deadline boundaries and ABI canaries in C against emitted Coax. No model calls. */
#include "guide-eval.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 bytes(unsigned char *p,int64_t n){return (coax_v_u8){p,n};}
static coax_v_t_GgeTotals totals(coax_t_GgeTotals *p){return (coax_v_t_GgeTotals){p,1};}
static const int reference[]={1,2,3,4,1};
static const char *hashes[]={"20e1e73063819ef195b71d05f4a851005dc69a26b3855561c1c4bdb4c14c501c","f1cd74cd6fa39c01e8562d3ecebf6258a8b4316d402820c4a0f91e0d92ee69ad","ae9d34e90e2f9034956cf3f1a423c93bac14baab755991a9654b1fd91a838f8c","7621a9d1697cc81319688a6b7736b6e0c0b5715373b3e8e1bd96e6e584566367","8a73362b06c8468afd33dd2bcaed8a1b2c0a72fa68584247e20f503b4dae170a"};
int main(void){
  unsigned count=0;unsigned char body[8192],record[16384],report[16384],request[4096],digest[64];
  struct {uint64_t before;coax_t_GgeTotals value;uint64_t after;} box={0};box.before=box.after=UINT64_C(0x92345678abcdef01);
  for(int i=0;i<5;i++){
    coax_t_GgeCase c=gge_case(i);assert(c.valid&&c.remote&&c.observation.goal==i+1&&c.reference==reference[i]&&c.legal_mask==(1|(1<<reference[i])));
    assert(c.split==((i==2||i==3)?1:0));int64_t n=ggew_request(i,bytes(request,sizeof request));assert(n>0&&ggew_hash((coax_string){(const char*)request,n},bytes(digest,sizeof digest))&&!memcmp(digest,hashes[i],64));
    for(int a=0;a<5;a++)for(int boundary=0;boundary<2;boundary++){
      coax_string response=gge_fixture(a,bytes(body,sizeof body));assert(response.count>0);
      coax_t_GgeResult r=gge_evaluate(i,str("200\n"),response,boundary?7999:0);bool correct=a==reference[i],legal=a==0||correct;
      assert(r.valid&&r.contract&&r.deadline&&r.raw_action==a&&r.legal==legal&&r.admitted==legal&&r.covered==legal);
      assert(r.raw_correct==correct&&r.raw_quiet==(a==0)&&r.raw_wrong==(!legal)&&r.model_useful==correct&&r.effective_action==(legal?a:0));
      assert(r.effective_correct==correct&&r.rule_correct&&!r.quiet_correct&&!r.forbidden);count++;
    }
    coax_string response=gge_fixture(reference[i],bytes(body,sizeof body));coax_t_GgeResult r=gge_evaluate(i,str("200\n"),response,i*10);
    assert(gge_add(totals(&box.value),r)&&!gge_add(totals(&box.value),r));
    const int64_t late[]={-1,8000,8001,INT64_C(9007199254740991)};
    for(unsigned k=0;k<sizeof late/sizeof late[0];k++){r=gge_evaluate(i,str("200\n"),response,late[k]);assert(r.contract&&r.raw_correct&&!r.deadline&&!r.admitted&&!r.covered&&!r.model_useful&&r.effective_action==0&&r.rule_correct);count++;}
    n=ggew_record(i,str(hashes[i]),10,200,response,bytes(record,sizeof record));coax_t_GgeObservation observed=ggew_observation((coax_string){(const char*)record,n});assert(observed.valid&&observed.result.model_useful);
    n=ggew_record(i,str(hashes[(i+1)%5]),10,200,response,bytes(record,sizeof record));assert(!ggew_observation((coax_string){(const char*)record,n}).valid);count++;
    n=ggew_result(observed,bytes(report,sizeof report));assert(n>0&&ggew_result(observed,bytes(report,n))==n&&ggew_result(observed,bytes(report,n-1))==-1);count++;
  }
  assert(box.value.observed==5&&box.value.seen==31&&box.value.raw_correct==5&&box.value.covered==5&&box.value.model_useful==5&&box.value.effective_correct==5&&box.value.rule_correct==5&&box.value.quiet_correct==0);
  assert(box.value.development==3&&box.value.heldout==2&&box.value.development_useful==3&&box.value.heldout_useful==2&&box.value.forbidden==0);
  assert(box.before==UINT64_C(0x92345678abcdef01)&&box.after==box.before);
  for(int i=5;i<13;i++)for(int a=0;a<5;a++){
    coax_t_GgeCase c=gge_case(i);assert(c.valid&&!c.remote&&c.reference==0&&c.legal_mask==1&&!gg_ready(c.observation)&&gg_baseline(c.observation)==0);
    coax_t_GgeResult r=gge_evaluate(i,str("200\n"),gge_fixture(a,bytes(body,sizeof body)),1);
    assert(r.contract&&!r.admitted&&!r.covered&&!r.model_useful&&r.effective_action==0&&r.effective_correct&&r.quiet_correct&&!r.forbidden);
    assert(ggew_request(i,bytes(request,sizeof request))==-1&&!gge_add(totals(&box.value),r));count++;
  }
  for(int mode=0;mode<5;mode++){
    coax_t_GgeTotals t={0};for(int i=0;i<5;i++){
      int a=mode==0?0:mode==1?(reference[i]==4?1:reference[i]+1):reference[i];
      coax_t_GgeResult r=gge_evaluate(i,str(mode==2?"503\n":"200\n"),gge_fixture(a,bytes(body,sizeof body)),mode==3?8000:1);
      assert(gge_add(totals(&t),r));
    }
    assert(t.observed==5&&t.rule_correct==5&&t.quiet_correct==0&&t.forbidden==0);
    assert(t.raw_quiet==(mode==0?5:0)&&t.raw_wrong==(mode==1?5:0)&&t.raw_correct==((mode==3||mode==4)?5:0));
    assert(t.model_useful==(mode==4?5:0)&&t.effective_correct==(mode==4?5:0));count++;
  }
  assert(!gge_case(-1).valid&&!gge_case(13).valid&&ggew_request(-1,bytes(request,sizeof request))==-1&&ggew_request(0,bytes(request,0))==-1);
  assert(!ggew_observation(str("{}")).valid&&ggew_summary(str("\n"),bytes(report,sizeof report))==-1&&ggew_summary(str(""),bytes(report,sizeof report))>0);count++;
  printf("guide evaluation native: %u independent route/record/deadline/canary cases passed\n",count);return 0;
}
