#define _POSIX_C_SOURCE 200809L
#include "deepseek-engram-memory.h"
#include "engram-goldens.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#define VI(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define V8(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define V16(p,n) ((coax_v_u16){(uint16_t *)(p),(n)})
static void read_file(const char *path,void *data,size_t bytes) {
    FILE *f=fopen(path,"rb");assert(f);assert(fread(data,1,bytes,f)==bytes);assert(fgetc(f)==EOF);assert(!fclose(f));
}
static void unchanged(uint16_t *words,size_t n) {for(size_t i=0;i<n;i++)assert(words[i]==0x1234);}
static void hash_original(uint8_t *map_bytes) {
    coax_t_EgAdmission a=eg_v41_map(V8(map_bytes,517120));assert(!a.status&&a.value.raw==129280&&a.value.compressed==99092&&a.value.pad==2);
    assert(a.value.data.data==map_bytes);
    int64_t output[913],work[912];output[912]=77;
    coax_t_EgHash r=psionic_eg_hash(a.value,eg_start(),VI(egg_original,19),V8(egg_mask,19),VI(output,913),VI(work,912));
    assert(!r.status&&r.written==912&&r.next.next==19&&!memcmp(output,egg_original_hashes,912*sizeof(int64_t))&&output[912]==77);
    for(int kind=0;kind<2;kind++) {
        coax_t_EgHistory state=eg_start();int64_t at=0;
        while(at<19) {
            int64_t n=kind?(at%5+1):1;if(n>19-at)n=19-at;
            r=psionic_eg_hash(a.value,state,VI(egg_original+at,n),V8(egg_mask+at,n),VI(output,n*48),VI(work,n*48));
            assert(!r.status&&!memcmp(output,egg_original_hashes+at*48,(size_t)n*48*sizeof(int64_t)));state=r.next;at+=n;
        }
        assert(state.next==19);
        for(int i=0;i<3;i++) {
            int64_t t=egg_original[18-i];uint32_t c=(uint32_t)map_bytes[4*t]|(uint32_t)map_bytes[4*t+1]<<8|(uint32_t)map_bytes[4*t+2]<<16|(uint32_t)map_bytes[4*t+3]<<24;
            assert(state.last.e[i]==(egg_mask[18-i]?(int64_t)c:-1));
        }
    }
    r=psionic_eg_hash(a.value,eg_start(),VI(egg_original,19),V8(egg_mask,19),VI(work,912),VI(work,912));assert(r.status==COAX_EgStatus_alias);
    int64_t owned_tokens[19];memcpy(owned_tokens,egg_original,sizeof owned_tokens);
    r=psionic_eg_hash(a.value,eg_start(),VI(owned_tokens,19),V8(egg_mask,19),VI(owned_tokens,19),VI(work,912));assert(r.status==COAX_EgStatus_capacity);
    // Real overlapping allocation with enough extent, including an immutable input.
    int64_t overlap[913];memcpy(overlap,egg_original,19*sizeof(int64_t));
    assert(psionic_eg_hash(a.value,eg_start(),VI(overlap,19),V8(egg_mask,19),VI(overlap,912),VI(work,912)).status==COAX_EgStatus_alias);
    map_bytes[517119]^=1;assert(eg_v41_map(V8(map_bytes,517120)).status==COAX_EgStatus_identity);map_bytes[517119]^=1;
    puts("Engram: admitted original tokenizer map; 912 original-ID hashes and whole/single/irregular history match independent prefix equations");
}
static void row_io(const uint8_t *all_packet,const uint16_t *all_words) {
    int64_t logical=0,allocated=0;
    for(int layer=0;layer<2;layer++) {
        int64_t rows=egg_tables[layer][0];
        coax_t_EgTable table={.rows=rows,.data=egg_tables[layer][1],.scales=egg_tables[layer][2],.file_bytes=egg_tables[layer][3]};
        assert(eg_table_valid(table));
        FILE *file=tmpfile();assert(file);int fd=fileno(file);assert(!ftruncate(fd,table.file_bytes));
        int64_t ids[24];memcpy(ids,egg_original_hashes+18*48+layer*24,sizeof ids);
        // Exercise the exact last row and >2 GiB offsets as well as original hashed rows.
        ids[23]=rows-1;
        coax_t_EgRead plan[48],draft[48];coax_t_EgResult p=eg_reads(table,VI(ids,24),(coax_v_t_EgRead){plan,48},(coax_v_t_EgRead){draft,48},0);
        assert(!p.status&&p.written==48&&p.bytes==6336);
        for(int64_t i=0;i<p.written;i++)assert(pwrite(fd,all_packet+layer*6336+plan[i].destination,(size_t)plan[i].bytes,plan[i].offset)==plan[i].bytes);
        struct stat st;assert(!fstat(fd,&st));assert(st.st_size==table.file_bytes&&st.st_blocks*512<1048576);logical+=st.st_size;allocated+=st.st_blocks*512;
        uint16_t output[6145],work[6144];uint8_t packet[6336];
        for(int mapped=0;mapped<2;mapped++) {
            for(int i=0;i<6145;i++)output[i]=0x1234;
            coax_t_EgResult r=psionic_eg_lookup(fd,mapped,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(work,6144));
            assert(!r.status&&r.written==6144&&r.bytes==6336&&!memcmp(output,all_words+layer*6144,6144*sizeof(uint16_t))&&output[6144]==0x1234);
        }
        for(int i=0;i<6145;i++)output[i]=0x1234;
        assert(psionic_eg_lookup(fd,0,table,VI(ids,24),V16(output,6145),V8(packet,6335),V16(work,6144)).status==COAX_EgStatus_capacity);
        assert(psionic_eg_lookup(fd,0,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(output,6144)).status==COAX_EgStatus_alias);
        assert(psionic_eg_lookup(-1,0,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(work,6144)).status==COAX_EgStatus_io);
        int64_t last=ids[23];ids[23]=rows;
        assert(psionic_eg_lookup(fd,0,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(work,6144)).status==COAX_EgStatus_token);ids[23]=last;
        uint8_t bad=255;
        assert(pwrite(fd,&bad,1,table.scales+ids[23]*8+7)==1);
        for(int mapped=0;mapped<2;mapped++)assert(psionic_eg_lookup(fd,mapped,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(work,6144)).status==COAX_EgStatus_number);
        unchanged(output,6145);
        assert(!ftruncate(fd,table.file_bytes-1));
        assert(psionic_eg_lookup(fd,0,table,VI(ids,24),V16(output,6145),V8(packet,6336),V16(work,6144)).status==COAX_EgStatus_io);unchanged(output,6145);
        fclose(file);
    }
    printf("Engram: mmap and pread read 12672 selected bytes from %lld logical bytes (%lld allocated); 12288 BF16 words match independent row equations\n",(long long)logical,(long long)allocated);
}
int main(int argc,char **argv) {
    assert(argc==4);uint8_t *map=malloc(517120);assert(map);read_file(argv[1],map,517120);hash_original(map);free(map);
    uint8_t packet[12672];uint16_t words[12288];read_file(argv[2],packet,sizeof packet);read_file(argv[3],words,sizeof words);row_io(packet,words);
    puts("Engram: native address checks, exact capacities, late invalid rows and file failures preserve public outputs");return 0;
}
