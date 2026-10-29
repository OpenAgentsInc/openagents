/* macOS-only lifecycle experiment. This program never yields a trusted DNS set,
   socket destination, provider snapshot, or authorization. */
#include "generated.h"
#include <arpa/inet.h>
#include <assert.h>
#include <dispatch/dispatch.h>
#include <dns_sd.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

typedef struct {
 coax_t_FiDnsObservation state;
 unsigned callbacks;
 dispatch_semaphore_t first;
} Probe;
static void DNSSD_API receive(DNSServiceRef ref,DNSServiceFlags flags,uint32_t interface_index,
 DNSServiceErrorType error,const char *host,const struct sockaddr *sa,uint32_t ttl,void *context) {
 (void)ref;(void)interface_index;(void)host;(void)ttl;
 Probe *p=context;p->callbacks++;if(p->first)dispatch_semaphore_signal(p->first);
 if(error || !(flags&kDNSServiceFlagsAdd) || !sa || sa->sa_len<sizeof(struct sockaddr)) {
  p->state=fi_dns_observation_fail(p->state);return;
 }
 uint8_t bytes[16];int n=0;
 if(sa->sa_family==AF_INET && sa->sa_len>=sizeof(struct sockaddr_in)) {
  const struct sockaddr_in *a=(const void *)sa;n=4;memcpy(bytes,&a->sin_addr,4);
 } else if(sa->sa_family==AF_INET6 && sa->sa_len>=sizeof(struct sockaddr_in6)) {
  const struct sockaddr_in6 *a=(const void *)sa;
  if(a->sin6_scope_id){p->state=fi_dns_observation_fail(p->state);return;}
  n=16;memcpy(bytes,&a->sin6_addr,16);
 }
 if(n==0){p->state=fi_dns_observation_fail(p->state);return;}
 p->state=fi_dns_observation_add(p->state,(coax_v_u8){bytes,n});
}
static struct sockaddr_in address(unsigned a,unsigned b,unsigned c,unsigned d) {
 struct sockaddr_in s={0};s.sin_len=sizeof(s);s.sin_family=AF_INET;
 unsigned char bytes[]={a,b,c,d};memcpy(&s.sin_addr,bytes,4);return s;
}
int main(void) {
 /* Test-only guard against a stalled platform call; this is not a proof that
    DNSServiceGetAddrInfo or DNSServiceRefDeallocate has a universal deadline. */
 alarm(10);
 assert(signal(SIGCHLD,SIG_IGN)!=SIG_ERR);
 dispatch_queue_t queue=dispatch_queue_create("openagents.fi-dns-probe",DISPATCH_QUEUE_SERIAL);assert(queue);
 __block Probe p={.state=fi_dns_observation_start()};
 struct sockaddr_in public=address(8,8,8,8),private=address(127,0,0,1);
 dispatch_sync(queue, ^{
  receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&public,0,&p);
  assert(p.state.status==0 && p.state.count==1);
  /* A cleared MoreComing flag was already observed; a later unsafe answer
     still poisons the entire observation. */
  receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&private,0,&p);
  assert(p.state.status==1);
  p.state=fi_dns_observation_start();p.state=fi_dns_observation_cancel(p.state);
  receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&public,0,&p);
  assert(p.state.status==2);
  p.state=fi_dns_observation_start();
  for(int i=0;i<17;i++)receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&public,0,&p);
  assert(p.state.status==1);
  p.state=fi_dns_observation_start();
  receive(NULL,0,0,0,NULL,(const void *)&public,0,&p);assert(p.state.status==1);
  p.state=fi_dns_observation_start();
  struct sockaddr_in short_addr=public;short_addr.sin_len=1;
  receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&short_addr,0,&p);assert(p.state.status==1);
  p.state=fi_dns_observation_start();
  struct sockaddr_in6 scoped={0};scoped.sin6_len=sizeof(scoped);scoped.sin6_family=AF_INET6;
  scoped.sin6_addr.s6_addr[0]=38;scoped.sin6_addr.s6_addr[1]=6;scoped.sin6_addr.s6_addr[2]=71;scoped.sin6_scope_id=1;
  receive(NULL,kDNSServiceFlagsAdd,0,0,NULL,(const void *)&scoped,0,&p);assert(p.state.status==1);
 });
 /* Real DNS-SD lifecycle: set up and deallocate the reference on its serial
    dispatch queue. The callback retains only this probe's queue-owned state. */
 __block DNSServiceRef ref=NULL;__block DNSServiceErrorType err=0;
 dispatch_semaphore_t first=dispatch_semaphore_create(0);assert(first);
 dispatch_sync(queue, ^{
  p.state=fi_dns_observation_start();p.callbacks=0;p.first=first;
  err=DNSServiceGetAddrInfo(&ref,0,0,kDNSServiceProtocol_IPv4|kDNSServiceProtocol_IPv6,
                            "localhost.",receive,&p);
  if(!err)err=DNSServiceSetDispatchQueue(ref,queue);
  if(err && ref){DNSServiceRefDeallocate(ref);ref=NULL;}
 });
 assert(err==kDNSServiceErr_NoError && ref);
 assert(dispatch_semaphore_wait(first,dispatch_time(DISPATCH_TIME_NOW,2000000000LL))==0);
 __block unsigned after=0;
 dispatch_sync(queue, ^{
  DNSServiceRefDeallocate(ref);ref=NULL;p.state=fi_dns_observation_cancel(p.state);after=p.callbacks;
 });
 usleep(100000);
 dispatch_sync(queue, ^{assert(after>0 && p.callbacks==after);assert(p.state.status!=0);});
 printf("macOS DNS-SD serialized cancellation and adversarial callback probes passed (%u callbacks before cancellation)\n",after);
 return 0;
}
