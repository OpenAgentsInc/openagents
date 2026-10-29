/* Address observations only. Parsing, identity, formats and publication are Coax. */
#include "weights-memory.h"
#include "spans.h"
typedef struct { const void *data; int64_t count; size_t width; } Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touching(Span a, Span b) {
    return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);
}
static bool observes(Span *writes, size_t nw, Span *reads, size_t nr) {
    for (size_t i=0;i<nw;i++) {
        for (size_t j=i+1;j<nw;j++) if (touching(writes[i],writes[j])) return true;
        for (size_t j=0;j<nr;j++) if (touching(writes[i],reads[j])) return true;
    }
    return false;
}
coax_t_PwResult psionic_weights_inspect(coax_v_u8 file, coax_v_t_PwTensor output, coax_v_t_PwTensor draft) {
    Span w[]={SPAN(output),SPAN(draft)}, r[]={SPAN(file)};
    return pw_inspect_checked(file,output,draft,observes(w,2,r,1));
}
coax_t_PwResult psionic_weights_decode(coax_v_u8 file, coax_string name,
        int64_t first, int64_t count, coax_v_f32 output, coax_v_f32 scratch,
        coax_v_t_PwTensor inventory, coax_v_t_PwTensor draft) {
    Span w[]={SPAN(output),SPAN(scratch),SPAN(inventory),SPAN(draft)}, r[]={SPAN(file),SPAN(name)};
    return pw_decode_checked(file,name,first,count,output,scratch,inventory,draft,observes(w,4,r,2));
}
coax_t_PwResult psionic_weights_bundle(coax_t_PwIdentity id, coax_v_u8 config,
        coax_v_u8 tokenizer, coax_v_u8 index, coax_v_t_PwShard shards,
        coax_v_t_PwSpec specs, coax_v_t_PwTensor output, coax_v_t_PwTensor draft) {
    if (!pw_bundle_counts(shards.count,specs.count,output.count,draft.count,config.count,tokenizer.count,index.count))
        return pw_bundle_checked(id,config,tokenizer,index,shards,specs,output,draft,true);
    Span w[]={SPAN(output),SPAN(draft)}, r[]={SPAN(config),SPAN(tokenizer),SPAN(index),
        SPAN(shards),SPAN(specs),SPAN(id.origin),SPAN(id.revision),SPAN(id.config_sha),
        SPAN(id.tokenizer_sha),SPAN(id.index_sha)};
    bool overlap=observes(w,2,r,sizeof(r)/sizeof(*r));
    /* Only Coax-admitted counts bound these observations of nested views. */
    if (!overlap) for (int64_t i=0;i<shards.count;i++) {
        Span nested[]={SPAN(shards.data[i].name),SPAN(shards.data[i].data),SPAN(shards.data[i].sha)};
        overlap=overlap || observes(w,2,nested,3);
    }
    if (!overlap) for (int64_t i=0;i<specs.count;i++) {
        Span nested[]={SPAN(specs.data[i].name)};
        overlap=overlap || observes(w,2,nested,1);
    }
    return pw_bundle_checked(id,config,tokenizer,index,shards,specs,output,draft,overlap);
}
