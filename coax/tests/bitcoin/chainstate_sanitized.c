/* Firm reason: 2 (bounded I/O and allocation for the independent native
   chapter 58 oracle, enabling ASan/UBSan without injecting them into Python).
   Consensus, state transitions and serialization all execute in Coax. */
#include "chainstate-generated.c"

int main(void) {
    uint64_t q[6];
    if (fread(q, sizeof(q), 1, stdin) != 1 || q[0] > 3 ||
        q[3] > 160000000 || q[4] > 8000000 || q[5] > 64000000) return 2;
    size_t cap = q[5] * 3 + 16000000;
    uint8_t *headers = calloc(1,q[3]+1), *body = calloc(1,q[4]+1), *flat = calloc(1,q[5]+1);
    uint8_t *table = calloc(1,cap), *stage = calloc(1,cap), *batch = calloc(1,64000000);
    uint8_t *undo = calloc(1,32000000), *after = calloc(1,cap), *restored = calloc(1,cap);
    coax_t_ValidationNative *state = calloc(1,(size_t)validation_native_size());
    if (!headers || !body || !flat || !table || !stage || !batch || !undo || !after || !restored || !state) return 2;
    if (fread(headers,1,q[3],stdin) != q[3] || fread(body,1,q[4],stdin) != q[4] ||
        fread(flat,1,q[5],stdin) != q[5] || fgetc(stdin) != EOF) return 2;
    coax_v_t_ValidationNative d = {state,1};
    coax_v_u8 h = {headers,(int64_t)q[3]}, b = {body,(int64_t)q[4]};
    coax_v_u8 v = {table,(int64_t)cap}, s = {stage,(int64_t)cap};
    coax_v_u8 batch_view = {batch,64000000}, undo_view = {undo,32000000};
    int64_t header = (q[0]&1) ? validation_native_fixture(d,(int64_t)q[1],(int64_t)q[2],h)
                            : validation_native_init(d,(int64_t)q[1],h);
    int64_t used = (int64_t)q[5];
    if (q[0]&2) used = validation_native_indexed(v,(coax_v_u8){flat,(int64_t)q[5]});
    else memcpy(table,flat,q[5]);
    coax_t_CsResult r = {.code=-1};
    int64_t after_n = 0, restored_n = 0, undo_n = 0;
    if (header >= 0 && used >= 0 && q[4] >= 36 && q[2] >= 1) {
        used = cs_native_fixture_tip(v,used,(int64_t)q[1],(int64_t)q[2]-1,(coax_v_u8){body+4,32});
        if (used < 0) return 2;
        r = cs_native_connect(d,(int64_t)q[2],b,v,used,s,batch_view,undo_view);
        if (r.code == 0) {
            after_n = validation_native_canonical(s,r.used,(coax_v_u8){after,(int64_t)cap});
            undo_n = r.undo_n;
            /* Save the returned undo before disconnect reconstructs its scratch. */
            if (undo_n < 0 || undo_n > 32000000 || after_n < 0) return 2;
            coax_t_CsResult back = cs_disconnect_vartime(s,r.used,v,batch_view,undo_view);
            if (back.code != 0) return 3;
            restored_n = validation_native_canonical(v,back.used,(coax_v_u8){restored,(int64_t)cap});
        } else restored_n = validation_native_canonical(v,used,(coax_v_u8){restored,(int64_t)cap});
        if (restored_n < 0) return 2;
    }
    if (fwrite(&header,8,1,stdout) != 1 || fwrite(&r,sizeof(r),1,stdout) != 1 ||
        fwrite(&after_n,8,1,stdout) != 1 || fwrite(&undo_n,8,1,stdout) != 1 || fwrite(&restored_n,8,1,stdout) != 1 ||
        fwrite(after,1,(size_t)after_n,stdout) != (size_t)after_n || fwrite(undo,1,(size_t)undo_n,stdout) != (size_t)undo_n ||
        fwrite(restored,1,(size_t)restored_n,stdout) != (size_t)restored_n) return 2;
    free(state); free(restored); free(after); free(undo); free(batch); free(stage); free(table); free(flat); free(body); free(headers);
    return 0;
}
