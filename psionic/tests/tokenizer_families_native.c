#include "families.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
struct vector {const char *text;int64_t bytes;const int64_t *ids;int64_t count;};
struct chat {coax_string source,bos,eos,expected;_Bool generation;int64_t count;coax_t_JtMessage messages[16];};
#include "families-goldens.h"
static void tokens(coax_t_TfPiece *v,int64_t n,const struct vector *vectors,size_t count,_Bool sentencepiece){
 for(size_t i=0;i<count;i++){const struct vector *x=&vectors[i];coax_t_TfResult r=sentencepiece?tf_sentencepiece((coax_v_t_TfPiece){v,n},(coax_string){x->text,x->bytes},2):tf_wordpiece((coax_v_t_TfPiece){v,n},(coax_string){x->text,x->bytes},100);if(r.status || r.written!=x->count){fprintf(stderr,"%s vector %zu: status=%u count=%lld expected=%lld\n",sentencepiece?"T5":"BERT",i,r.status,(long long)r.written,(long long)x->count);}assert(!r.status && r.written==x->count);for(int64_t k=0;k<r.written;k++){if(r.ids.e[k]!=x->ids[k])fprintf(stderr,"vector %zu token %lld: got %lld expected %lld\n",i,(long long)k,(long long)r.ids.e[k],(long long)x->ids[k]);assert(r.ids.e[k]==x->ids[k]);}}
}
int main(void){
 tokens(bert_vocab,sizeof bert_vocab/sizeof *bert_vocab,bert_vectors,sizeof bert_vectors/sizeof *bert_vectors,0);
 tokens(t5_vocab,sizeof t5_vocab/sizeof *t5_vocab,t5_vectors,sizeof t5_vectors/sizeof *t5_vectors,1);
 for(size_t i=0;i<sizeof chats/sizeof *chats;i++){const struct chat *x=&chats[i];coax_t_JtResult r=jt_render(x->source,(coax_v_t_JtMessage){(coax_t_JtMessage *)x->messages,x->count},x->generation,x->bos,x->eos);if(r.status || r.written!=x->expected.count || memcmp(r.bytes.e,x->expected.data,(size_t)x->expected.count)){fprintf(stderr,"Jinja vector %zu: status %u bytes %lld expected %lld\n",i,r.status,(long long)r.written,(long long)x->expected.count);}assert(!r.status && r.written==x->expected.count && !memcmp(r.bytes.e,x->expected.data,(size_t)r.written));}
 coax_t_TfPiece finite[]={{.text={"a",1},.id=1}};uint64_t nonfinite=UINT64_C(0x7ff0000000000000);memcpy(&finite[0].score,&nonfinite,8);assert(tf_wordpiece((coax_v_t_TfPiece){finite,1},(coax_string){"a",1},0).status==COAX_TfStatus_vocabulary);nonfinite=UINT64_C(0x7ff8000000000000);memcpy(&finite[0].score,&nonfinite,8);assert(tf_wordpiece((coax_v_t_TfPiece){finite,1},(coax_string){"a",1},0).status==COAX_TfStatus_vocabulary);
 puts("BERT/T5: 290 original-vocabulary ID vectors; bounded Jinja: 12 upstream-rendered vectors pass");return 0;
}
