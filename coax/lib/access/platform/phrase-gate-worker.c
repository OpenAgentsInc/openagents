/* Fixed storage, scalar ABI transport and identified-buffer erasure effects.
 * All format, KDF, authentication, admission and completion policy is Coax. */
#include "zeroize.h"
#include <stdint.h>
#if defined(__wasm__)
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static coax_t_PgJob state;
static uint8_t work[22128],password[128],record[4096],message[6144],wire[4202],relay[2048],recipient[64],entropy[28],output[6144];
static int32_t relay_count,recipient_count;
#ifdef COAX_PG_TEST_OBSERVE
#ifdef COAX_PG_TEST_INIT_FAULT
static uint32_t cleanup_fault=1;
#else
static uint32_t cleanup_fault;
#endif
#endif
static coax_v_t_PgJob state_view(void){return(coax_v_t_PgJob){&state,1};}
static coax_v_u8 bytes(uint8_t*p,int64_t n){return(coax_v_u8){p,n};}
static coax_string text(uint8_t*p,int64_t n){return(coax_string){(const char*)p,n};}
static uint32_t erase_inputs(void){
 uint32_t e=coax_secure_zero(password,sizeof password,0,sizeof password);
 e|=coax_secure_zero(record,sizeof record,0,sizeof record);
 e|=coax_secure_zero(message,sizeof message,0,sizeof message);
 e|=coax_secure_zero(entropy,sizeof entropy,0,sizeof entropy);return e;
}
static uint32_t erase_private(void){
 uint32_t e=erase_inputs();e|=coax_secure_zero(work,sizeof work,0,sizeof work);
 e|=coax_secure_zero(wire,sizeof wire,0,sizeof wire);
 e|=coax_secure_zero(relay,sizeof relay,0,sizeof relay);
 e|=coax_secure_zero(recipient,sizeof recipient,0,sizeof recipient);
#ifdef COAX_PG_TEST_OBSERVE
 if(cleanup_fault==1)e|=6; /* Test-only uncertain-effect status, never a success. */
#endif
 return e;
}
static uint32_t close_job(void){
 uint32_t status=erase_private();uint32_t result=pg_ack_cleanup(state_view(),status);
 if(result){uint32_t erased=coax_secure_zero(output,sizeof output,0,sizeof output);if(erased){pg_abort_after_effect(state_view(),erased);return pg_ack_cleanup(state_view(),erased);}}
 return result;
}
EXPORT("password") uintptr_t pg_worker_password(void){return(uintptr_t)password;}
EXPORT("record") uintptr_t pg_worker_record(void){return(uintptr_t)record;}
EXPORT("message") uintptr_t pg_worker_message(void){return(uintptr_t)message;}
EXPORT("relay") uintptr_t pg_worker_relay(void){return(uintptr_t)relay;}
EXPORT("recipient") uintptr_t pg_worker_recipient(void){return(uintptr_t)recipient;}
EXPORT("entropy") uintptr_t pg_worker_entropy(void){return(uintptr_t)entropy;}
EXPORT("output") uintptr_t pg_worker_output(void){return pg_result(state_view()).error?0:(uintptr_t)output;}
EXPORT("written") int32_t pg_worker_written(void){return(int32_t)pg_result(state_view()).written;}
EXPORT("error") uint32_t pg_worker_error(void){return pg_result(state_view()).error;}
EXPORT("running") uint32_t pg_worker_running(void){return pg_is_running(state_view());}
EXPORT("emit") uint32_t pg_worker_emit(void){return pg_may_emit(state_view());}
EXPORT("cancelled") uint32_t pg_worker_cancelled(void){return pg_was_cancelled(state_view());}
EXPORT("budget") int32_t pg_worker_default_budget(void){return(int32_t)pg_worker_budget();}
EXPORT("random_count") int32_t pg_worker_random_count(uint32_t operation){return(int32_t)pg_random_bytes(operation);}
EXPORT("phase") uint32_t pg_worker_phase(void){return pg_progress(state_view()).phase;}
EXPORT("completed") int32_t pg_worker_completed(void){return(int32_t)pg_progress(state_view()).completed;}
EXPORT("total") int32_t pg_worker_total(void){return(int32_t)pg_progress(state_view()).total;}
EXPORT("preflight") uint32_t pg_worker_preflight(int32_t pn,int32_t rn,int32_t mn,int32_t ln,int32_t kn,uint32_t operation){return pg_message_request(pn,rn,mn,ln,kn,28,operation);}
EXPORT("attempts") uint32_t pg_worker_attempts(int32_t legacy,int32_t v2){return pg_attempts(legacy,v2);}
EXPORT("dispose") uint32_t pg_worker_dispose(void){
 uint32_t status=erase_private();status|=coax_secure_zero(output,sizeof output,0,sizeof output);
 return pg_reset_after_wipe(state_view(),status);
}
EXPORT("abort") uint32_t pg_worker_abort(void){pg_abort_after_effect(state_view(),0);return close_job();}
EXPORT("begin") uint32_t pg_worker_begin(int32_t pn,int32_t rn,int32_t mn,int32_t ln,int32_t kn,uint32_t operation){
 uint32_t status=pg_transport_ready(state_view());if(status)return status;
 status=pg_worker_preflight(pn,rn,mn,ln,kn,operation);
 if(status){pg_abort(state_view(),status);return close_job();}
 relay_count=ln;recipient_count=kn;
 status=pg_begin_message_checked(state_view(),bytes(work,sizeof work),bytes(wire,sizeof wire),text(message,mn),text(password,pn),text(record,rn),text(relay,ln),text(recipient,kn),bytes(entropy,sizeof entropy),operation,0);
 uint32_t erased=erase_inputs();if(erased){pg_abort_after_effect(state_view(),erased);return close_job();}
 if(status)return close_job();return 0;
}
EXPORT("advance") uint32_t pg_worker_advance(int32_t budget){
 uint32_t status=pg_step_message_checked(state_view(),bytes(work,sizeof work),bytes(wire,sizeof wire),text(relay,relay_count),text(recipient,recipient_count),bytes(output,sizeof output),budget);
 if(status&&!pg_cleanup_pending(state_view()))pg_abort(state_view(),status);
 if(pg_cleanup_pending(state_view()))return close_job();return status;
}
EXPORT("cancel") uint32_t pg_worker_cancel(void){
 uint32_t status=pg_cancel_checked(state_view(),bytes(work,sizeof work));if(status)pg_abort(state_view(),status);return close_job();
}
#ifdef COAX_PG_TEST_OBSERVE
/* Absent from normal workers. No test path bypasses the real fixed-round KDF. */
EXPORT("test_fault") void pg_worker_test_fault(uint32_t fault){cleanup_fault=fault;}
EXPORT("test_trap") void pg_worker_test_trap(void){__builtin_trap();}
EXPORT("clean") uint32_t pg_worker_clean(void){
 if(cleanup_fault==2)__builtin_trap();
 unsigned bits=0;
#define OBSERVE(a) for(unsigned i=0;i<sizeof(a);i++)bits|=(a)[i]
 OBSERVE(work);OBSERVE(password);OBSERVE(record);OBSERVE(message);OBSERVE(wire);OBSERVE(relay);OBSERVE(recipient);OBSERVE(entropy);OBSERVE(output);
#undef OBSERVE
 return bits==0;
}
#endif
