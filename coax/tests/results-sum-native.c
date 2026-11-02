/* Focused native checks for the actual emitted compiler policies. Compile with
   -I pointing to one coherent generated compiler tuple, at O0/O2/fatal ASan.
   The implementation is Coax-generated; this file supplies literal oracles. */
#include "proto_importer.c"
#include <assert.h>
#include <string.h>

static void resource(coax_t_CK_State *s, coax_t_CK_Error *errors) {
    assert(s->fatal && s->error_count == 1);
    assert(errors[0].code.count == 5 && !memcmp(errors[0].code.data,"E0381",5));
}

int main(void) {
    coax_t_CK_State state[1] = {0};
    coax_t_CK_Error errors[4] = {0};
    coax_t_CK_Context c = {0};
    c.state=(coax_v_t_CK_State){state,1};c.errors=(coax_v_t_CK_Error){errors,4};
    const int64_t limits[] = {-1,0,1,2,1048576};
    for (unsigned i=0;i<sizeof(limits)/sizeof(limits[0]);i++) {
        memset(state,0,sizeof(state));memset(errors,0,sizeof(errors));
        coax_t_CK_SumBudget budget[1]={{.used=limits[i]>0?limits[i]-1:0,.limit=limits[i]}};
        coax_v_t_CK_SumBudget view={budget,1};
        if(limits[i]>0){assert(coax_p_CK_sum_tick(c,view));assert(budget[0].used==limits[i]&&!state[0].fatal);}
        assert(!coax_p_CK_sum_tick(c,view));assert(budget[0].used==(limits[i]>0?limits[i]:0));resource(state,errors);
    }
    for(uint32_t tag=0;tag<100;tag++)assert(coax_p_CK_sum_type_tag(tag)==(tag>=1&&tag<=6));
    assert(!coax_p_CK_sum_type_tag(UINT32_MAX));
    assert(!coax_p_CK_sum_can_propagate(0,COAX_CK_Kind_result,12,COAX_CK_Kind_result,12));
    assert(coax_p_CK_sum_can_propagate(1,COAX_CK_Kind_result,12,COAX_CK_Kind_result,12));
    assert(coax_p_CK_sum_can_propagate(2,COAX_CK_Kind_optional,0,COAX_CK_Kind_optional,0));
    assert(!coax_p_CK_sum_can_propagate(2,COAX_CK_Kind_result,12,COAX_CK_Kind_result,13));
    assert(!coax_p_CK_sum_can_propagate(2,COAX_CK_Kind_result,12,COAX_CK_Kind_optional,0));
    assert(!coax_p_CK_sum_can_propagate(2,COAX_CK_Kind_optional,0,COAX_CK_Kind_result,12));
    const struct {int64_t n,a,size,alignment,offset;_Bool valid;} cases[]={
        {0,1,4,4,0,1},{1,1,8,4,4,1},{8,8,16,8,8,1},{17,8,32,8,8,1},
        {0,1073741824,4,4,0,1},{1,536870912,1073741824,536870912,536870912,1},
        {2147483640,4,2147483644,4,4,1},{2147483644,4,0,0,0,0},
        {2147483647,1,0,0,0,0},{INT64_MAX,1,0,0,0,0},{-1,1,0,0,0,0},
        {0,2147483648,0,0,0,0},{0,INT64_MAX,0,0,0,0},{0,INT64_MIN,0,0,0,0}
    };
    for(unsigned i=0;i<sizeof(cases)/sizeof(cases[0]);i++){
        int64_t sizes[]={cases[i].n},aligns[]={cases[i].a};
        coax_t_SL_Layout l=coax_p_SL_layout((coax_v_s64){sizes,1},(coax_v_s64){aligns,1});
        assert(l.valid==cases[i].valid&&l.size==cases[i].size&&l.alignment==cases[i].alignment&&l.offset==cases[i].offset);
    }
    /* The original four-unit root-cycle contract survives the extracted meter. */
    coax_t_CK_Node nodes[8]={0};coax_t_CK_Type types[20]={0};
    c.nodes=(coax_v_t_CK_Node){nodes,8};c.types=(coax_v_t_CK_Type){types,20};
    types[16].kind=COAX_CK_Kind_struct_;types[16].decl=2;
    nodes[2].kind=COAX_CK_Tag_struct_;nodes[2].head=3;
    nodes[3].kind=COAX_CK_Tag_field;nodes[3].type=16;
    for(int budget=3;budget<=4;budget++){
        memset(state,0,sizeof(state));memset(errors,0,sizeof(errors));state[0].used=3;state[0].type_count=17;
        assert(coax_p_CK_sum_scan(c,16,0,budget)==(budget==3));
        if(budget==3)resource(state,errors);else assert(!state[0].fatal&&!state[0].error_count);
    }
    return 0;
}
