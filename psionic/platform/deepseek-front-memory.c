#include "deepseek-front-memory.h"
#include "spans.h"
typedef struct { const void *data; int64_t count; size_t width; } DfSpan;
#define DF_SPAN(v) (DfSpan){(v).data,(v).count,sizeof(*(v).data)}
static _Bool df_overlap(DfSpan a, DfSpan b) {
    return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);
}
static _Bool df_writes(DfSpan *read, size_t nr, DfSpan *write, size_t nw) {
    for (size_t i=0;i<nw;i++) {
        for (size_t j=0;j<nr;j++) if (df_overlap(write[i],read[j])) return 1;
        for (size_t j=0;j<i;j++) if (df_overlap(write[i],write[j])) return 1;
    }
    return 0;
}
coax_t_DfResult psionic_df_feed(coax_t_DfState s, coax_v_s64 units,
        coax_v_t_DfEvent out, coax_v_t_DfEvent stage) {
    DfSpan read[]={DF_SPAN(units)},write[]={DF_SPAN(out),DF_SPAN(stage)};
    return df_feed(s,units,out,stage,df_writes(read,1,write,2));
}
coax_t_DfRecipe psionic_df_recipe(coax_v_t_DfMessage messages, _Bool thinking,
        coax_v_t_DfSegment out, coax_v_t_DfSegment stage) {
    DfSpan read[]={DF_SPAN(messages)},write[]={DF_SPAN(out),DF_SPAN(stage)};
    _Bool alias=df_writes(read,1,write,2);
    for (int64_t i=0;i<messages.count;i++) {
        DfSpan text={messages.data[i].content.data,messages.data[i].content.count,1};
        for (size_t j=0;j<2;j++) alias |= df_overlap(text,write[j]);
    }
    return df_recipe(messages,thinking,out,stage,alias);
}
coax_t_PtResult psionic_df_encode_r1(coax_v_u8 table, coax_v_t_DfSegment segments,
        coax_v_s64 out, coax_v_s64 stage, coax_v_s64 temp, coax_v_u8 raw,
        coax_v_u8 normal, coax_v_s64 scalars, coax_v_s64 symbols) {
    DfSpan read[]={DF_SPAN(table),DF_SPAN(segments)};
    DfSpan write[]={DF_SPAN(out),DF_SPAN(stage),DF_SPAN(temp),DF_SPAN(raw),DF_SPAN(normal),DF_SPAN(scalars),DF_SPAN(symbols)};
    _Bool alias=df_writes(read,2,write,7);
    for (int64_t i=0;i<segments.count;i++) {
        DfSpan text={segments.data[i].text.data,segments.data[i].text.count,1};
        for (size_t j=0;j<7;j++) alias |= df_overlap(text,write[j]);
    }
    return df_encode_r1(table,segments,out,stage,temp,raw,normal,scalars,symbols,alias);
}
coax_t_PtResult psionic_df_r1_units(coax_v_u8 table, coax_v_s64 ids,
        coax_v_s64 out, coax_v_s64 stage) {
    DfSpan read[]={DF_SPAN(table),DF_SPAN(ids)},write[]={DF_SPAN(out),DF_SPAN(stage)};
    return df_r1_units(table,ids,out,stage,df_writes(read,2,write,2));
}
