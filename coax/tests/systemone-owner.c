/* Independent scripted process effects for the actual generated SoOwnedCall API.
 * This proves Coax ownership decisions and copied snapshots, not native kill/reap
 * or a live HTTP service. No C padding is compared. */
#include "systemone-owner.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
static FILE *diagnostics(void) { return stderr; }
#undef stderr

enum { SUCCESS, HTTP_ERROR, CHILD_ERROR, TIMEOUT, READ_ERROR, SPAWN_ERROR, EOF_PENDING };
static int scenario, line_index, initial_idle, live, effects, releases, waits;
static const char response[] = "{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}";
static const char child_error[] = "scripted stderr survives slot reuse";
static const char *line;
static int observations;
#define REQUIRE(x) do { observations++; if (!(x)) { fprintf(diagnostics(), "owner check line %d: %s\n", __LINE__, #x); return 1; } } while (0)

typedef struct { uint8_t before[16], data[512], after[16]; } Guard;
static void fill(Guard *g) { memset(g, 0xa5, sizeof *g); }
static int guards(const Guard *g) { for (int i=0; i<16; i++) if (g->before[i]!=0xa5 || g->after[i]!=0xa5) return 0; return 1; }
static coax_string text(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_u8 bytes(Guard *g) { return (coax_v_u8){g->data, sizeof g->data}; }
static coax_v_u8 prefix(Guard *g, int64_t n) { return (coax_v_u8){g->data, n}; }
static coax_v_t_SoOwnedCall owned(coax_t_SoOwnedCall *o) { return (coax_v_t_SoOwnedCall){o,1}; }
static coax_v_t_SoResult result(coax_t_SoResult *r) { return (coax_v_t_SoResult){r,1}; }
static int same_text(coax_string a, coax_string b) { return a.data==b.data && a.count==b.count; }
static int content(coax_string a, const char *s) { return a.count==(int64_t)strlen(s) && (a.count==0 || memcmp(a.data,s,(size_t)a.count)==0); }
static int same_result(const coax_t_SoResult *a, const coax_t_SoResult *b) {
  return a->status==b->status && same_text(a->body,b->body) && same_text(a->problem,b->problem) && same_text(a->stderr,b->stderr) &&
    a->usage.ok==b->usage.ok && a->usage.input_tokens==b->usage.input_tokens && a->usage.output_tokens==b->usage.output_tokens;
}
static int same_call(const coax_t_SoCall *a, const coax_t_SoCall *b) {
  return a->h==b->h && a->out.data==b->out.data && a->out.count==b->out.count && a->err.data==b->err.data && a->err.count==b->err.count &&
    a->at==b->at && a->last==b->last && a->lines==b->lines && a->timeout_s==b->timeout_s && a->done==b->done && same_result(&a->r,&b->r);
}
static int same_owner(const coax_t_SoOwnedCall *a, const coax_t_SoOwnedCall *b) {
  return a->generation==b->generation && a->phase==b->phase && same_call(&a->call.e[0],&b->call.e[0]);
}
static void disturb(void) { volatile uint8_t scratch[16384]; for (size_t i=0; i<sizeof scratch; i++) scratch[i]=(uint8_t)(i*37); }
static void reset(int which) { scenario=which; line_index=0; initial_idle=1; live=0; effects=0; releases=0; waits=0; line=""; }
static void next_response(int which) { scenario=which; line_index=0; initial_idle=1; waits=0; line=""; }

/* One actual logical process slot, reused eagerly after release. */
int64_t cxproc_new(void) { effects++; if (live) return -1; live=1; return 0; }
int64_t cxproc_push(int64_t h,int64_t field,int64_t byte) { (void)field;(void)byte;effects++;return h==0&&live?0:-1; }
int64_t cxproc_end(int64_t h,int64_t field) { (void)field;effects++;return h==0&&live?0:-1; }
int64_t cxproc_spawn(int64_t h) { effects++;return h==0&&live?(scenario==SPAWN_ERROR?2:0):-1; }
int64_t cxproc_write(int64_t h,int64_t byte) { (void)byte;effects++;return h==0&&live?0:-1; }
int64_t cxproc_flush(int64_t h) { effects++;return h==0&&live?0:-1; }
int64_t cxproc_close_stdin(int64_t h) { effects++;return h==0&&live?0:-1; }
int64_t cxproc_read_line(int64_t h,int64_t wait_ms) {
  effects++;if(h!=0||!live)return -9;
  if(scenario==TIMEOUT)return -2;
  if(scenario==READ_ERROR)return -9;
  if(scenario==CHILD_ERROR)return -1;
  if(wait_ms==0&&initial_idle){initial_idle=0;return -2;}
  if(line_index==0){line=response;line_index++;return (int64_t)strlen(line);}
  if(line_index==1){line=scenario==HTTP_ERROR?"503":"200";line_index++;return 3;}
  return -1;
}
int64_t cxproc_line_byte(int64_t h,int64_t i) { effects++;return h==0&&live?(uint8_t)line[i]:0; }
int64_t cxproc_wait(int64_t h,int64_t wait_ms) {
  effects++;waits++;if(h!=0||!live)return -1;
  if(scenario==EOF_PENDING&&wait_ms==0&&waits==1)return -2;
  return scenario==CHILD_ERROR?7:0;
}
int64_t cxproc_stderr_count(int64_t h) { effects++;return h==0&&live?(int64_t)strlen(child_error):0; }
int64_t cxproc_stderr_byte(int64_t h,int64_t i) { effects++;return h==0&&live?(uint8_t)child_error[i]:0; }
int64_t cxproc_release(int64_t h) { effects++;if(h!=0||!live)return -1;releases++;live=0;return 0; }
int64_t cxproc_getenv_push(int64_t byte) { (void)byte;effects++;return 0; }
int64_t cxproc_getenv(void) { effects++;return 0; }
// The transport's other process effects (pid, clock, kill, group, environment
// bytes): scripted children have no pid or process group, and time stands still.
int64_t cxproc_pid(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_now_ms(void){effects++;return 0;}
int64_t cxproc_kill(int64_t h,int64_t sig){(void)sig;effects++;return h==0&&live?0:-1;}
int64_t cxproc_group_alive(int64_t pgid){(void)pgid;effects++;return 0;}
int64_t cxproc_getenv_byte(int64_t i){(void)i;effects++;return 0;}
static coax_t_SoClient client(void) {
  coax_t_SoClient c;coax_default_t_SoClient(&c);c.key_env=text("");c.url=text("http://offline.invalid");c.curl=text("scripted-effect-boundary");return c;
}
static void init(coax_t_SoOwnedCall *o) { so_owner_init(owned(o)); }
static void sentinel_result(coax_t_SoResult *r) {
  r->status=418;r->body=text("unchanged body");r->stderr=text("unchanged stderr");r->problem=text("unchanged problem");
  r->usage.ok=1;r->usage.input_tokens=13;r->usage.output_tokens=17;
}

static int pending_and_atomic_snapshot(void) {
  Guard wire,wire_err,body,err,problem,newwire,newerr;fill(&wire);fill(&wire_err);fill(&body);fill(&err);fill(&problem);fill(&newwire);fill(&newerr);
  coax_t_SoOwnedCall o;init(&o);coax_t_SoResult r;sentinel_result(&r);reset(SUCCESS);
  REQUIRE(o.generation==0&&o.phase==0&&o.call.e[0].h==-1);
  int64_t ticket=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&wire_err));
  REQUIRE(ticket==1&&o.generation==ticket&&o.phase==1&&live);
  coax_t_SoOwnedCall before=o;coax_t_SoResult oldr=r;Guard oldbody=body,olderr=err,oldproblem=problem;
  int count=effects;
  REQUIRE(!so_owner_take(owned(&o),ticket,result(&r),bytes(&body),bytes(&err),bytes(&problem)));
  REQUIRE(same_owner(&o,&before)&&same_result(&r,&oldr)&&effects==count);
  REQUIRE(!memcmp(&body,&oldbody,sizeof body)&&!memcmp(&err,&olderr,sizeof err)&&!memcmp(&problem,&oldproblem,sizeof problem));
  REQUIRE(!so_owner_start(owned(&o),client(),text("{}"),bytes(&newwire),bytes(&newerr))&&same_owner(&o,&before)&&effects==count);
  REQUIRE(!so_owner_poll(owned(&o),ticket,0)&&o.phase==1&&live);
  REQUIRE(so_owner_poll(owned(&o),ticket,1)&&o.phase==2&&!live&&releases==1);
  REQUIRE(o.call.e[0].r.status==200&&content(o.call.e[0].r.body,response));
  before=o;count=effects;
  REQUIRE(so_owner_poll(owned(&o),ticket,0)&&same_owner(&o,&before)&&effects==count);
  REQUIRE(!so_owner_start(owned(&o),client(),text("{}"),bytes(&newwire),bytes(&newerr))&&same_owner(&o,&before)&&effects==count);
  /* One short capacity refuses before writing even the first byte/result. */
  REQUIRE(!so_owner_take(owned(&o),ticket,result(&r),prefix(&body,(int64_t)strlen(response)-1),bytes(&err),bytes(&problem)));
  REQUIRE(same_owner(&o,&before)&&same_result(&r,&oldr)&&effects==count);
  REQUIRE(!memcmp(&body,&oldbody,sizeof body)&&!memcmp(&err,&olderr,sizeof err)&&!memcmp(&problem,&oldproblem,sizeof problem));
  coax_v_t_SoResult empty={NULL,0};
  REQUIRE(!so_owner_take(owned(&o),ticket,empty,bytes(&body),bytes(&err),bytes(&problem))&&same_owner(&o,&before));
  REQUIRE(so_owner_take(owned(&o),ticket,result(&r),prefix(&body,(int64_t)strlen(response)),prefix(&err,0),prefix(&problem,0)));
  REQUIRE(o.phase==0&&o.generation==ticket&&o.call.e[0].h==-1&&effects==count);
  REQUIRE(r.status==200&&content(r.body,response)&&r.body.data==(const char*)body.data&&r.problem.count==0&&r.stderr.count==0);
  REQUIRE(r.usage.ok&&r.usage.input_tokens==7&&r.usage.output_tokens==2);
  /* A copied result borrows only its independent snapshot, after wire reuse. */
  coax_t_SoResult copied=r;memset(wire.data,'X',sizeof wire.data);memset(wire_err.data,'Y',sizeof wire_err.data);disturb();
  REQUIRE(content(copied.body,response)&&guards(&wire)&&guards(&wire_err)&&guards(&body)&&guards(&err)&&guards(&problem));
  oldr=r;before=o;
  REQUIRE(!so_owner_take(owned(&o),ticket,result(&r),bytes(&body),bytes(&err),bytes(&problem))&&same_result(&r,&oldr)&&same_owner(&o,&before));
  return 0;
}

static int stale_tickets_and_slot_reuse(void) {
  Guard wire,wire_err,body,err,problem;fill(&wire);fill(&wire_err);fill(&body);fill(&err);fill(&problem);
  coax_t_SoOwnedCall o;init(&o);reset(SUCCESS);coax_t_SoResult r;sentinel_result(&r);
  int64_t old=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&wire_err));
  int64_t copied=old;
  REQUIRE(old==1&&so_owner_discard(owned(&o),old)&&o.phase==0&&releases==1&&!live);
  next_response(SUCCESS);
  int64_t current=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&wire_err));
  REQUIRE(current==2&&live&&o.phase==1&&o.call.e[0].h==0);
  coax_t_SoOwnedCall before=o;coax_t_SoResult before_result=r;Guard oldbody=body,olderr=err,oldproblem=problem;int count=effects;
  const int64_t invalid[]={old,copied,0,-1,INT64_MIN,INT64_MAX};
  for(size_t i=0;i<sizeof invalid/sizeof invalid[0];i++){
    REQUIRE(!so_owner_poll(owned(&o),invalid[i],1));
    REQUIRE(!so_owner_cancel(owned(&o),invalid[i]));
    REQUIRE(!so_owner_discard(owned(&o),invalid[i]));
    REQUIRE(!so_owner_take(owned(&o),invalid[i],result(&r),bytes(&body),bytes(&err),bytes(&problem)));
    REQUIRE(same_owner(&o,&before)&&same_result(&r,&before_result)&&effects==count&&live&&releases==1);
    REQUIRE(!memcmp(&body,&oldbody,sizeof body)&&!memcmp(&err,&olderr,sizeof err)&&!memcmp(&problem,&oldproblem,sizeof problem));
  }
  REQUIRE(so_owner_cancel(owned(&o),current)&&o.phase==2&&!live&&releases==2);
  before=o;count=effects;
  REQUIRE(so_owner_cancel(owned(&o),current)&&same_owner(&o,&before)&&effects==count);
  REQUIRE(so_owner_take(owned(&o),current,result(&r),bytes(&body),bytes(&err),bytes(&problem))&&content(r.problem,"cancelled"));
  REQUIRE(r.problem.data==(const char*)problem.data&&r.status==0&&r.body.count==0&&r.stderr.count==0);
  coax_t_SoResult snapshot=r;
  /* The released process slot now belongs to an unrelated host user. */
  REQUIRE(cxproc_new()==0&&live);count=effects;before=o;
  REQUIRE(!so_owner_poll(owned(&o),current,1)&&!so_owner_cancel(owned(&o),current)&&!so_owner_discard(owned(&o),current));
  REQUIRE(same_owner(&o,&before)&&effects==count&&live&&releases==2&&content(snapshot.problem,"cancelled"));
  REQUIRE(cxproc_release(0)==0);return 0;
}

static int terminal_snapshots(void) {
  for(int which=HTTP_ERROR;which<=SPAWN_ERROR;which++){
    Guard wire,wire_err,body,err,problem;fill(&wire);fill(&wire_err);fill(&body);fill(&err);fill(&problem);
    coax_t_SoOwnedCall o;init(&o);coax_t_SoResult r;sentinel_result(&r);reset(which);
    int64_t ticket=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&wire_err));
    REQUIRE(ticket==1);
    REQUIRE(so_owner_poll(owned(&o),ticket,1)&&o.phase==2&&!live&&releases==1);
    int64_t nb=o.call.e[0].r.body.count,ne=o.call.e[0].r.stderr.count,np=o.call.e[0].r.problem.count;
    REQUIRE(np>0);coax_t_SoOwnedCall before=o;coax_t_SoResult oldr=r;Guard oldbody=body,olderr=err,oldproblem=problem;int count=effects;
    for(int short_field=0;short_field<3;short_field++){
      int64_t n=short_field==0?nb:short_field==1?ne:np;if(n==0)continue;
      REQUIRE(!so_owner_take(owned(&o),ticket,result(&r),prefix(&body,short_field==0?n-1:512),prefix(&err,short_field==1?n-1:512),prefix(&problem,short_field==2?n-1:512)));
      REQUIRE(same_owner(&o,&before)&&same_result(&r,&oldr)&&effects==count);
      REQUIRE(!memcmp(&body,&oldbody,sizeof body)&&!memcmp(&err,&olderr,sizeof err)&&!memcmp(&problem,&oldproblem,sizeof problem));
    }
    REQUIRE(so_owner_take(owned(&o),ticket,result(&r),prefix(&body,nb),prefix(&err,ne),prefix(&problem,np)));
    REQUIRE(o.phase==0&&r.body.count==nb&&r.stderr.count==ne&&r.problem.count==np&&effects==count);
    if(which==HTTP_ERROR)REQUIRE(r.status==503&&content(r.body,response)&&content(r.problem,"the API refused the request"));
    if(which==CHILD_ERROR)REQUIRE(r.status==0&&content(r.stderr,child_error)&&r.stderr.data==(const char*)err.data&&content(r.problem,"the server is unreachable"));
    if(which==TIMEOUT)REQUIRE(content(r.problem,"curl did not finish in time"));
    if(which==READ_ERROR)REQUIRE(content(r.problem,"the response does not fit its buffer"));
    if(which==SPAWN_ERROR)REQUIRE(content(r.problem,"curl not found"));
    coax_t_SoResult saved=r;
    next_response(SUCCESS);int64_t later=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&wire_err));
    REQUIRE(later==ticket+1&&so_owner_poll(owned(&o),later,1));
    memset(wire.data,'Q',sizeof wire.data);memset(wire_err.data,'R',sizeof wire_err.data);disturb();
    REQUIRE(saved.problem.count==np&&memcmp(saved.problem.data,problem.data,(size_t)np)==0);
    if(which==HTTP_ERROR)REQUIRE(content(saved.body,response));
    if(which==CHILD_ERROR)REQUIRE(content(saved.stderr,child_error));
    REQUIRE(so_owner_discard(owned(&o),later)&&guards(&wire)&&guards(&wire_err)&&guards(&body)&&guards(&err)&&guards(&problem));
  }
  return 0;
}

static int never_started_exhaustion_and_eof(void) {
  Guard wire,err,body,problem;fill(&wire);fill(&err);fill(&body);fill(&problem);coax_t_SoOwnedCall o;init(&o);reset(SUCCESS);
  REQUIRE(cxproc_new()==0);int count=effects;coax_t_SoOwnedCall before=o;
  REQUIRE(!so_owner_discard(owned(&o),0)&&!so_owner_cancel(owned(&o),0)&&!so_owner_poll(owned(&o),0,0));
  REQUIRE(same_owner(&o,&before)&&effects==count&&live);REQUIRE(cxproc_release(0)==0);
  o.generation=INT64_MAX;before=o;count=effects;
  REQUIRE(!so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&err))&&same_owner(&o,&before)&&effects==count);
  o.generation=-1;before=o;
  REQUIRE(!so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&err))&&same_owner(&o,&before)&&effects==count);
  init(&o);next_response(SUCCESS);int64_t ticket=so_owner_start(owned(&o),client(),text(""),bytes(&wire),bytes(&err));
  REQUIRE(ticket==1&&o.phase==2&&!live&&effects==count&&content(o.call.e[0].r.problem,"no request body"));
  REQUIRE(so_owner_discard(owned(&o),ticket)&&o.phase==0&&effects==count);
  coax_v_t_SoOwnedCall empty={NULL,0};coax_t_SoResult r;sentinel_result(&r);coax_t_SoResult oldr=r;
  REQUIRE(!so_owner_start(empty,client(),text("{}"),bytes(&wire),bytes(&err)));
  REQUIRE(!so_owner_poll(empty,1,0)&&!so_owner_cancel(empty,1)&&!so_owner_discard(empty,1));
  REQUIRE(!so_owner_take(empty,1,result(&r),bytes(&body),bytes(&err),bytes(&problem))&&same_result(&r,&oldr)&&effects==count);
  next_response(EOF_PENDING);ticket=so_owner_start(owned(&o),client(),text("{}"),bytes(&wire),bytes(&err));
  REQUIRE(ticket==2&&!so_owner_poll(owned(&o),ticket,0));
  REQUIRE(!so_owner_poll(owned(&o),ticket,0)&&o.phase==1&&live);
  REQUIRE(so_owner_poll(owned(&o),ticket,0)&&o.phase==2&&!live);
  REQUIRE(so_owner_discard(owned(&o),ticket)&&o.phase==0);return 0;
}

int main(void) {
  if(pending_and_atomic_snapshot()||stale_tickets_and_slot_reuse()||terminal_snapshots()||never_started_exhaustion_and_eof())return 1;
  printf("{\"scope\":\"actual generated owned System One API; scripted process effects\",\"observations\":%d,\"groups\":4}\n",observations);return 0;
}
