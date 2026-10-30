#include "bpe-train-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct bt_vector {coax_string text;const int64_t *ids;int64_t count;const int64_t *ends;int64_t pieces;};
#include "bpe-train-goldens.h"
static unsigned char *read_file(const char *path,size_t *n){FILE *f=fopen(path,"rb");assert(f && !fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0 && !fseek(f,0,SEEK_SET));unsigned char *p=malloc((size_t)size);assert(p && fread(p,1,(size_t)size,f)==(size_t)size && !fclose(f));*n=(size_t)size;return p;}
int main(int argc,char **argv){assert(argc==4);size_t nc,ne;unsigned char *classes=read_file(argv[1],&nc),*expected=read_file(argv[2],&ne),*out=malloc(200000),*stage=malloc(200000);assert(out && stage);memset(out,77,200000);memset(stage,165,200000);
 coax_t_BtModel m=bt_train((coax_v_u8){classes,(int64_t)nc},(coax_v_string){bt_documents,sizeof bt_documents/sizeof *bt_documents},64);assert(!m.status && m.merges==bt_summary[0] && m.source_bytes==bt_summary[1] && m.pieces==bt_summary[2] && m.remaining==bt_summary[3]);for(int i=0;i<m.merges;i++){coax_t_BtRule r=m.rules.e[i];assert(r.left==bt_rules[i][0] && r.right==bt_rules[i][1] && r.frequency==bt_rules[i][2] && r.replaced==bt_rules[i][3]);}
 coax_t_PtResult packed=psionic_bt_pack(m,(coax_v_u8){classes,(int64_t)nc},(coax_v_u8){out,200000},(coax_v_u8){stage,200000});assert(!packed.status && packed.written==(int64_t)ne && !memcmp(out,expected,ne) && out[ne]==77 && stage[ne]==165);
 FILE *f=fopen(argv[3],"wb");assert(f && fwrite(out,1,ne,f)==ne && !fclose(f));size_t nr;unsigned char *reopened=read_file(argv[3],&nr);assert(nr==ne && !memcmp(out,reopened,ne));
 for(size_t i=0;i<sizeof bt_vectors/sizeof *bt_vectors;i++){const struct bt_vector *v=&bt_vectors[i];coax_t_BtSplits s=bt_split((coax_v_u8){classes,(int64_t)nc},v->text);assert(!s.status && s.written==v->pieces);for(int64_t k=0;k<s.written;k++){if(s.ends.e[k]!=v->ends[k])fprintf(stderr,"split %zu/%lld got %lld expected %lld\n",i,(long long)k,(long long)s.ends.e[k],(long long)v->ends[k]);assert(s.ends.e[k]==v->ends[k]);}
 coax_t_BtEncoding e=bt_encode((coax_v_u8){reopened,(int64_t)nr},v->text);assert(!e.status && e.written==v->count);for(int64_t k=0;k<e.written;k++)assert(e.ids.e[k]==v->ids[k]);coax_t_BtText d=bt_decode((coax_v_u8){reopened,(int64_t)nr},(coax_v_s64){e.ids.e,e.written});assert(!d.status && d.written==v->text.count && !memcmp(d.bytes.e,v->text.data,(size_t)d.written));}
 assert(psionic_bt_pack(m,(coax_v_u8){classes,(int64_t)nc},(coax_v_u8){out,200000},(coax_v_u8){out,200000}).status==COAX_PtStatus_alias);assert(psionic_bt_pack(m,(coax_v_u8){classes,(int64_t)nc},(coax_v_u8){classes,(int64_t)nc},(coax_v_u8){stage,200000}).status==COAX_PtStatus_alias);
 printf("BPE: all %lld merge rules, exact PST5 persistence, %zu independent Unicode regex/ID/byte round trips pass\n",(long long)m.merges,sizeof bt_vectors/sizeof *bt_vectors);free(classes);free(expected);free(out);free(stage);free(reopened);return 0;}
