/* Startup authentication observations only: no CPU or device model decision.
 * Reuse the exact production mapper/session/arena constructor. Tooling only. */
#define main psionic_auth_unused_host_main
#include "../platform/serve-host.c"
#undef main
int main(int argc,char **argv){
    if(argc!=3)return 2;Server s={.metrics_fd=-1,.host_budget=INT64_C(68719476736)};int64_t started=now_us();
    open_elephant(&s,argv[1],argv[2],128,false,0);
    coax_t_LsSample sample={.ok=true,.kind=1,.total=ls_elapsed(started,now_us()),.auth=s.auth_us,.peak_bytes=peak_bytes(),.cached_pages=s.cached_pages,.total_pages=s.total_pages,.execution_bytes=s.execution_bytes,.cpu_execution_bytes=s.cpu_execution_bytes};
    uint8_t bytes[4096];int64_t n=ls_record(str("production-auth-only-probe"),s.engine.digest,0,sample,(coax_v_u8){bytes,sizeof bytes});if(n<0)return 1;
    return fwrite(bytes,1,(size_t)n,stdout)==(size_t)n?0:1;
}
