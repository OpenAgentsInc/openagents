/* Chapter 237 against suite_oracle.py, which read chapter 16's held-out file
   and hashed the same encoding with Python's hashlib. The suite is built here
   from the oracle's copy of that file, not from the example's. Every digest,
   status, count and score must be equal. */
#include "suite-spans.h"
#include "suite-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static coax_string str(const char *s){return (coax_string){s?s:"",s?(int64_t)strlen(s):0};}
static void same(coax_a32_u8 d,const uint8_t *want){assert(memcmp(d.e,want,32)==0);}
static void suite_is(coax_t_SuSuite s,const uint8_t want[3][32]){
    assert(s.status==COAX_SuStatus_ok&&s.mask==2);same(s.items,want[0]);same(s.questions,want[1]);same(s.gate,want[2]);
}

static coax_t_SuItem items[12],edited[12];
static coax_string questions[12],reworded[12];
static coax_t_SuReceipt store[8];

static coax_t_SuReceipt run(coax_t_SuSuite s,int64_t partition,uint64_t system){
    double values[12];int64_t n=0;double sum=0;
    for(int i=0;i<12;i++)if(items[i].partition==partition){values[n]=system==1||items[i].gold==0?1:0;sum+=values[n];n++;}
    coax_t_SuReceipt r={0};r.suite=s;r.partition=partition;r.system=system;r.read=n;r.score=sum/(double)n;
    r.values=su_values((coax_v_f64){values,n});return r;
}

int main(void){
    for(int i=0;i<12;i++){
        coax_t_SuItem *it=&items[i];
        it->id=str(su_oracle_ids[i]);it->kind=(uint8_t)su_oracle_kind[i];it->task=su_oracle_task[i];
        it->partition=su_oracle_partition[i];it->state=str(su_oracle_states[i]);it->options=su_oracle_options[i];
        for(int k=0;k<it->options;k++){
            const char *name=su_oracle_option_name[i][k],*description=su_oracle_option_description[i][k];
            it->option.e[k]=(coax_t_SuOption){name!=0,str(name),description!=0,str(description)};
        }
        it->gold=su_oracle_gold[i];it->abstain=su_oracle_abstain[i];
        questions[i]=reworded[i]=str(su_oracle_questions[i]);edited[i]=items[i];
    }
    reworded[7]=str(su_oracle_reworded_text);edited[7].state=str(su_oracle_replaced_text);
    const coax_t_SuGate gate={0,40,10,1000,7,1,30};coax_t_SuGate more=gate;more.replicates=2000;
    coax_v_t_SuItem all={items,12},changed={edited,12};coax_v_string q={questions,12},rq={reworded,12};
    coax_t_SuSuite base=su_suite(all,q,gate,2);suite_is(base,su_oracle_base);
    coax_t_SuSuite s1=su_suite(all,rq,gate,2);suite_is(s1,su_oracle_reworded);
    coax_t_SuSuite s2=su_suite(changed,q,gate,2);suite_is(s2,su_oracle_replaced);
    coax_t_SuSuite s3=su_suite(all,q,more,2);suite_is(s3,su_oracle_other_gate);

    coax_v_t_SuReceipt view=psionic_su_store(store,8);int64_t count=0;
    coax_t_SuReceipt tries[7]={run(base,0,1),run(base,1,1),run(base,1,1),run(base,0,2),run(s1,0,2),run(s2,0,2),run(s3,0,2)};
    for(int k=0;k<7;k++){
        coax_t_SuAppend a=su_append(view,count,tries[k]);
        if(k==2){assert(a.status==COAX_SuStatus_locked);continue;}
        assert(a.status==COAX_SuStatus_ok&&a.count==count+1);count=a.count;
        same(store[count-1].digest,su_oracle_receipts[count-1]);assert(store[count-1].score==su_oracle_scores[count-1]);
    }
    assert(count==6&&su_verify(view,count).status==COAX_SuStatus_ok);
    assert(su_compare(store[0],store[2]).status==COAX_SuStatus_ok&&su_compare(store[0],store[2]).questions_same);
    assert(su_compare(store[2],store[3]).status==COAX_SuStatus_ok&&!su_compare(store[2],store[3]).questions_same);
    assert(su_compare(store[2],store[4]).status==COAX_SuStatus_items&&su_compare(store[2],store[5]).status==COAX_SuStatus_gate);
    assert(su_compare(store[0],store[1]).status==COAX_SuStatus_partition);
    store[1].score=0.5;coax_t_SuCheck c=su_verify(view,count);assert(c.status==COAX_SuStatus_broken&&c.index==1);
    store[1].digest=su_receipt_digest(store[1]);c=su_verify(view,count);assert(c.status==COAX_SuStatus_broken&&c.index==2);

    /* Native refusals: NaN and infinity in scores and limits. */
    coax_t_SuReceipt r=run(base,0,2);r.score=NAN;assert(su_append(view,count,r).status==COAX_SuStatus_number);
    r.score=INFINITY;assert(su_append(view,count,r).status==COAX_SuStatus_number);
    coax_t_SuGate bad=gate;bad.cost_limit=INFINITY;assert(su_suite(all,q,bad,2).status==COAX_SuStatus_number);
    bad=gate;bad.seconds_limit=NAN;assert(su_suite(all,q,bad,2).status==COAX_SuStatus_number);
    printf("suite: chapter 16's suite and its edits, every digest, receipt, refusal and comparison equal to the oracle; "
        "the chain breaks where it was changed; NaN and infinity refused\n");
    return 0;
}
