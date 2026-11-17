// A GameBuilder build agent (docs/protocol/gamebuilder-agent-v1.md): one piece of work on one game,
// asked from gamebuilder.org, run as one durable account turn on the hosted runner. Included by
// account-chat-turns.c, whose submit/read effects it reuses unchanged: the start body becomes a
// SubmitRequest in Coax (gamebuilder-agent-wire.coax, gba_submit), and reading one is the durable
// turn's own read. The turn has no tools; the page makes its plan lines after checking them.
//   POST /api/gamebuilder/agents/v1                 start: 202 ControlReply (turn_id, state)
//   GET  /api/gamebuilder/agent/v1?id=TURN          the Turn: state, the answer so far, usage
//   GET  /api/gamebuilder/agent/events/v1?id=&after= its events after a cursor
bool gamebuilder_agents_route(const request *q){
 const char *path=q->path;int op=!strcmp(path,"/api/gamebuilder/agents/v1")?1:!strcmp(path,"/api/gamebuilder/agent/v1")?2:!strcmp(path,"/api/gamebuilder/agent/events/v1")?3:0;if(!op)return false;
 const auth_principal *p=auth_principal_of(q);if(!p||!ctw_owns(p->kind,p->level,ts(p->account),ts(p->account))){refuse(q,401,"Log in first.");return true;}
 if(strcmp(q->method,op==1?"POST":"GET")){refuse(q,405,op==1?"POST this":"GET this");return true;}
 if(op==1&&!auth_first_party(q)){refuse(q,403,"Cross-site write refused.");return true;}
 PGconn *db=api_db();if(!db){refuse(q,503,"Agent storage unavailable.");return true;}
 if(op>1){read_turn(q,db,p,op==3);return true;}
 unsigned char *body=calloc(1,131073);if(!body){refuse(q,503,"Memory unavailable.");return true;}
 int64_t n=gba_submit(tn(q->body,(int64_t)q->body_len),tb(body,131072));
 if(n<1){free(body);refuse(q,400,"Invalid build agent request.");return true;}
 // The submit path keeps this route's own path, so its operation receipts and its limited-session
 // rule (ap_limited_write) are this route's, never /api/chat/submit/v2alpha1's.
 request next=*q;next.body=(const char *)body;next.body_len=(size_t)n;submit(&next,db,p);free(body);return true;
}
