/* Single-worker storage, ABI transport and explicit platform erasure only.
 * Compile with a zeroize-policy.h generated from pbkdf2-worker.coax. */
#include "zeroize.h"
#include <stdint.h>
#if defined(__wasm__)
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static uint8_t state[832],password[1024],salt[1024],output[256];
static coax_t_PbProgress progress;
EXPORT("password") uintptr_t pb_worker_password(void){return(uintptr_t)password;}
EXPORT("salt") uintptr_t pb_worker_salt(void){return(uintptr_t)salt;}
EXPORT("output") uintptr_t pb_worker_output(void){return(uintptr_t)output;}
EXPORT("budget") int32_t pb_worker_budget(void){return(int32_t)pbkdf2_worker_budget();}
EXPORT("preflight") uint32_t pb_worker_preflight(int32_t pn,int32_t sn,int32_t rounds,int32_t length,int32_t budget,int32_t gate){return pbkdf2_worker_request(pn,sn,rounds,length,budget,gate!=0);}
EXPORT("dispose") uint32_t pb_worker_dispose(void){
    uint32_t status=coax_secure_zero(state,sizeof(state),0,sizeof(state));
    status|=coax_secure_zero(password,sizeof(password),0,sizeof(password));
    status|=coax_secure_zero(salt,sizeof(salt),0,sizeof(salt));
    status|=coax_secure_zero(output,sizeof(output),0,sizeof(output));
    return status;
}
EXPORT("begin") uint32_t pb_worker_begin(int32_t pn,int32_t sn,int32_t rounds,int32_t length,int32_t gate){
    coax_v_u8 s={state,sizeof(state)},p={password,pn},n={salt,sn};
    uint32_t status=pbkdf2_worker_request(pn,sn,rounds,length,pbkdf2_worker_budget(),gate!=0);
    if(!status)status=gate?pbkdf2_gate_init(s,p,n):pbkdf2_init(s,p,n,rounds,length);
    uint32_t cleanup=coax_secure_zero(password,sizeof(password),0,sizeof(password));
    cleanup|=coax_secure_zero(salt,sizeof(salt),0,sizeof(salt));
    if(status||cleanup){pb_worker_dispose();return status?status:cleanup;}
    progress=pbkdf2_info(s);return progress.error;
}
EXPORT("advance") uint32_t pb_worker_advance(int32_t budget){progress=pbkdf2_step((coax_v_u8){state,sizeof(state)},budget);return progress.error;}
EXPORT("phase") uint32_t pb_worker_phase(void){return progress.phase;}
EXPORT("completed") int32_t pb_worker_completed(void){return(int32_t)progress.completed;}
EXPORT("total") int32_t pb_worker_total(void){return(int32_t)progress.total;}
EXPORT("finish") uint32_t pb_worker_finish(void){return pbkdf2_result((coax_v_u8){state,sizeof(state)},(coax_v_u8){output,sizeof(output)});}
EXPORT("cancel") uint32_t pb_worker_cancel(void){
    uint32_t status=pbkdf2_cancel((coax_v_u8){state,sizeof(state)});
    uint32_t cleanup=pb_worker_dispose();return status?status:cleanup;
}
#ifdef COAX_PBKDF2_TEST_OBSERVE
/* Explicit observable fixture only; absent from a normal worker build. */
EXPORT("clean") uint32_t pb_worker_clean(void){
    unsigned bits=0;
    for(unsigned i=0;i<sizeof(state);i++)bits|=state[i];
    for(unsigned i=0;i<sizeof(password);i++)bits|=password[i];
    for(unsigned i=0;i<sizeof(salt);i++)bits|=salt[i];
    for(unsigned i=0;i<sizeof(output);i++)bits|=output[i];
    return bits==0;
}
#endif
