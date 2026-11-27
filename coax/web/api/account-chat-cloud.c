// Unified Cloud Chat v2 storage & SSE route handlers (#14093).
// Included by account-chat-turns.c; all JSON wire composition and route matching
// are delegated to #authority Coax codecs (account-chat-cloud-wire.coax & stream-policy.coax).

static bool cloud_emit_account_event(PGconn *db, const char *account, const char *conv,
                                     const char *turn, const char *event_type, const char *title,
                                     const char *compute, const char *model, long long cost_msat,
                                     bool pinned, bool archived, const char *source_device,
                                     long long revision, long long now_ms) {
  coax_t_CcAcctEvent ev = {0};
  ev.conversation = ts(conv);
  ev.turn = ts(turn);
  ev.event_type = ts(event_type);
  ev.title = ts(title);
  ev.compute = ts(compute);
  ev.model = ts(model);
  ev.cost_msat = cost_msat;
  ev.pinned = pinned;
  ev.archived = archived;
  ev.source_device = ts(source_device);
  ev.revision = revision;
  ev.updated_at_ms = now_ms;
  unsigned char buf[2048];
  int64_t n = cc_acct_event_write((coax_v_t_CcAcctEvent){&ev, 1}, tb(buf, sizeof buf - 1));
  if (n < 1) return false;
  buf[n] = 0;
  char ms[32];
  num(ms, now_ms);
  const char *args[] = {account, conv ? conv : "", event_type, (char *)buf, ms};
  return exec_sql(db,
    "INSERT INTO chat.account_events_v2(account,conversation,event_type,record,created_ms) "
    "VALUES($1,$2,$3,$4,$5::bigint)",
    5, args);
}

static bool cloud_sync_lock(PGconn *db, const char *account, const char *op) {
  const char *p[] = {account, op};
  return exec_sql(db, "SELECT pg_advisory_xact_lock(hashtextextended('cloud-sync:'||$1||':'||$2,0))", 2, p);
}

// Lookup prior sync operation receipt in chat.sync_operations_v2.
// Returns 1 = fresh, 2 = exact replay (writes cc_sync_reply_write(..., replayed=true) into out),
// 0 = conflict (different digest), -1 = DB error.
static int cloud_sync_receipt_check(PGconn *db, const char *account, const char *op,
                                    const char *hash, unsigned char *out, size_t out_cap,
                                    int64_t *out_len) {
  const char *p[] = {account, op};
  PGresult *r = PQexecParams(db,
    "SELECT digest,response FROM chat.sync_operations_v2 WHERE account=$1 AND operation=$2",
    2, NULL, p, NULL, NULL, 0);
  if (!tuples(r)) { PQclear(r); return -1; }
  if (PQntuples(r) == 0) { PQclear(r); return 1; }
  bool same = !strcmp(PQgetvalue(r, 0, 0), hash);
  if (!same) { PQclear(r); return 0; }
  char saved[160] = {0};
  snprintf(saved, sizeof saved, "%s", PQgetvalue(r, 0, 1));
  PQclear(r);
  // Receipt format: "<cid>|<tid>|<revision>|<last_event_id>"
  char cid[37] = {0}, tid[37] = {0};
  long long rev = 1, last_ev = 0;
  char *p1 = strchr(saved, '|');
  char *p2 = p1 ? strchr(p1 + 1, '|') : NULL;
  char *p3 = p2 ? strchr(p2 + 1, '|') : NULL;
  if (!p1 || !p2 || !p3) return -1;
  *p1 = 0; *p2 = 0; *p3 = 0;
  snprintf(cid, sizeof cid, "%s", saved);
  snprintf(tid, sizeof tid, "%s", p1 + 1);
  rev = strtoll(p2 + 1, NULL, 10);
  last_ev = strtoll(p3 + 1, NULL, 10);
  int64_t n = cc_sync_reply_write(ts(cid), ts(tid), rev, last_ev, true, tb(out, out_cap));
  if (n < 1) return -1;
  *out_len = n;
  return 2;
}

static bool cloud_sync_receipt_save(PGconn *db, const char *account, const char *op,
                                    const char *hash, const char *cid, const char *tid,
                                    long long rev, long long last_ev, long long now_ms) {
  char receipt[160], ms[32];
  snprintf(receipt, sizeof receipt, "%s|%s|%lld|%lld", cid, tid ? tid : "", rev, last_ev);
  num(ms, now_ms);
  const char *p[] = {account, op, hash, receipt, ms};
  return exec_sql(db,
    "INSERT INTO chat.sync_operations_v2(account,operation,digest,response,created_ms) "
    "VALUES($1,$2,$3,$4,$5::bigint)",
    5, p);
}

static void cloud_stream_account(const request *q, PGconn *db, const auth_principal *p) {
  char after_text[32], last_id[64];
  api_query(q, "after", after_text, sizeof after_text);
  api_header(q, "Last-Event-ID", last_id, sizeof last_id);
  int64_t after = sp_effective_cursor(ts(last_id), ts(after_text));
  if (after < 0) { refuse(q, 400, "Invalid stream cursor."); return; }
  const char *args[] = {p->account};
  PGresult *r = PQexecParams(db,
    "SELECT COALESCE(max(id),0) FROM chat.account_events_v2 WHERE account=$1",
    1, NULL, args, NULL, NULL, 0);
  if (!tuples(r) || PQntuples(r) != 1) { PQclear(r); refuse(q, 503, "Chat storage unavailable."); return; }
  int64_t max_id = (int64_t)number(r, 0, 0);
  PQclear(r);
  int64_t cs = sp_cursor_status(after, max_id);
  if (cs == 409) { refuse(q, 409, "The event cursor is ahead of this account stream."); return; }
  if (cs != 200) { refuse(q, 400, "Invalid stream cursor."); return; }
  if (!stream_handoff_account_events(q, p->account, after)) {
    refuse(q, 503, "Event stream unavailable.");
  }
}

static void cloud_stream_conv(const request *q, PGconn *db, const auth_principal *p, const char *cid) {
  char after_text[32], last_id[64];
  api_query(q, "after", after_text, sizeof after_text);
  api_header(q, "Last-Event-ID", last_id, sizeof last_id);
  int64_t after = sp_effective_cursor(ts(last_id), ts(after_text));
  if (!sp_conv_id_valid(ts(cid)) || after < 0) { refuse(q, 400, "Invalid conversation or cursor."); return; }
  const char *args[] = {p->account, cid};
  PGresult *own = PQexecParams(db,
    "SELECT 1 FROM chat.conversations_v2 WHERE account=$1 AND id=$2",
    2, NULL, args, NULL, NULL, 0);
  if (!tuples(own)) { PQclear(own); refuse(q, 503, "Chat storage unavailable."); return; }
  if (PQntuples(own) != 1) { PQclear(own); refuse(q, 404, "not found"); return; }
  PQclear(own);
  PGresult *r = PQexecParams(db,
    "SELECT COALESCE(max(event_id),0) FROM chat.turn_events_v2 WHERE account=$1 AND conversation=$2",
    2, NULL, args, NULL, NULL, 0);
  if (!tuples(r) || PQntuples(r) != 1) { PQclear(r); refuse(q, 503, "Chat storage unavailable."); return; }
  int64_t max_ev = (int64_t)number(r, 0, 0);
  PQclear(r);
  int64_t cs = sp_cursor_status(after, max_ev);
  if (cs == 409) { refuse(q, 409, "The event cursor is ahead of this conversation stream."); return; }
  if (cs != 200) { refuse(q, 400, "Invalid conversation or cursor."); return; }
  if (!stream_handoff_conv_events(q, p->account, cid, after)) {
    refuse(q, 503, "Event stream unavailable.");
  }
}

static void cloud_conv_list(const request *q, PGconn *db, const auth_principal *p) {
  const char *args[] = {p->account};
  PGresult *r = PQexecParams(db,
    "SELECT c.id,c.title,c.revision,c.pinned,c.archived,"
    "coalesce((SELECT t.id FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id AND t.state IN(1,2) LIMIT 1),''),"
    "coalesce((SELECT t.compute FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),''),"
    "coalesce((SELECT t.model FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),''),"
    "coalesce((SELECT t.cost_msat FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),0),"
    "c.source_device,c.workspace,c.created_ms,c.updated_ms "
    "FROM chat.conversations_v2 c WHERE c.account=$1 "
    "ORDER BY c.pinned DESC,c.updated_ms DESC,c.id ASC LIMIT 100",
    1, NULL, args, NULL, NULL, 0);
  unsigned char *out = calloc(1, 262144);
  char *owned[100] = {0};
  coax_string items[100];
  int count = 0;
  bool ok = out && tuples(r);
  for (int i = 0; ok && i < PQntuples(r) && i < 100; i++) {
    coax_t_CcConvSummary c = {
      .id = ts(PQgetvalue(r, i, 0)),
      .title = ts(PQgetvalue(r, i, 1)),
      .revision = number(r, i, 2),
      .pinned = PQgetvalue(r, i, 3)[0] == 't',
      .archived = PQgetvalue(r, i, 4)[0] == 't',
      .active_turn = ts(PQgetvalue(r, i, 5)),
      .compute = ts(PQgetvalue(r, i, 6)),
      .model = ts(PQgetvalue(r, i, 7)),
      .cost_msat = number(r, i, 8),
      .source_device = ts(PQgetvalue(r, i, 9)),
      .workspace = ts(PQgetvalue(r, i, 10)),
      .created_at_ms = number(r, i, 11),
      .updated_at_ms = number(r, i, 12),
    };
    int64_t n = cc_conv_summary_write((coax_v_t_CcConvSummary){&c, 1}, tb(out, 4095));
    if (n < 1) { ok = false; break; }
    out[n] = 0;
    owned[count] = strdup((char *)out);
    ok = owned[count] != NULL;
    if (ok) items[count] = ts(owned[count]);
    count++;
  }
  int64_t n = ok ? cc_conv_list_write((coax_v_string){items, count}, tb(out, 262144)) : -1;
  if (n > 0) reply(q, 200, out, (size_t)n);
  else refuse(q, 503, "Conversation list unavailable.");
  for (int i = 0; i < count; i++) free(owned[i]);
  free(out);
  PQclear(r);
}

static void cloud_conv_get(const request *q, PGconn *db, const auth_principal *p, const char *cid) {
  if (!exec_sql(db, "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY", 0, NULL)) {
    refuse(q, 503, "Chat storage unavailable.");
    return;
  }
  const char *args[] = {p->account, cid};
  PGresult *cr = PQexecParams(db,
    "SELECT c.id,c.title,c.revision,c.pinned,c.archived,"
    "coalesce((SELECT t.id FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id AND t.state IN(1,2) LIMIT 1),''),"
    "coalesce((SELECT t.compute FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),''),"
    "coalesce((SELECT t.model FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),''),"
    "coalesce((SELECT t.cost_msat FROM chat.turns_v2 t WHERE t.account=c.account AND t.conversation=c.id ORDER BY t.ordinal DESC LIMIT 1),0),"
    "c.source_device,c.workspace,c.created_ms,c.updated_ms "
    "FROM chat.conversations_v2 c WHERE c.account=$1 AND c.id=$2",
    2, NULL, args, NULL, NULL, 0);
  PGresult *tr = NULL, *er = NULL;
  unsigned char *out = calloc(1, CT_CAP);
  unsigned char *tmp = calloc(1, 262144);
  char *turn_owned[128] = {0};
  coax_string turn_items[128];
  int tcount = 0;
  coax_string ev_items[512];
  int ecount = 0;
  int status = 503;
  const char *why = "Conversation unavailable.";

  if (!out || !tmp || !tuples(cr)) goto done;
  if (PQntuples(cr) != 1) { status = 404; why = "not found"; goto done; }

  coax_t_CcConvSummary cs = {
    .id = ts(PQgetvalue(cr, 0, 0)),
    .title = ts(PQgetvalue(cr, 0, 1)),
    .revision = number(cr, 0, 2),
    .pinned = PQgetvalue(cr, 0, 3)[0] == 't',
    .archived = PQgetvalue(cr, 0, 4)[0] == 't',
    .active_turn = ts(PQgetvalue(cr, 0, 5)),
    .compute = ts(PQgetvalue(cr, 0, 6)),
    .model = ts(PQgetvalue(cr, 0, 7)),
    .cost_msat = number(cr, 0, 8),
    .source_device = ts(PQgetvalue(cr, 0, 9)),
    .workspace = ts(PQgetvalue(cr, 0, 10)),
    .created_at_ms = number(cr, 0, 11),
    .updated_at_ms = number(cr, 0, 12),
  };

  tr = PQexecParams(db,
    "SELECT id,ordinal,state,compute,model,cost_msat,source_device,workspace,request_json,output,"
    "attachments_json,stop_reason,error,input_tokens,output_tokens,cache_read_tokens,"
    "cache_write_tokens,reasoning_tokens,sequence,created_ms,updated_ms "
    "FROM chat.turns_v2 WHERE account=$1 AND conversation=$2 ORDER BY ordinal ASC LIMIT 128",
    2, NULL, args, NULL, NULL, 0);
  if (!tuples(tr)) goto done;

  for (int i = 0; i < PQntuples(tr) && i < 128; i++) {
    coax_t_CtUsage u = {
      .input = nullable(tr, i, 13),
      .output = nullable(tr, i, 14),
      .cache_read = nullable(tr, i, 15),
      .cache_write = nullable(tr, i, 16),
      .reasoning = nullable(tr, i, 17),
    };
    bool has_u = (u.input >= 0 || u.output >= 0 || u.cache_read >= 0 || u.cache_write >= 0 || u.reasoning >= 0);
    // Extract plain user text when request_json is a v2alpha1 submit or plain synced text.
    const char *raw_req = PQgetvalue(tr, i, 8);
    const char *user_text = raw_req;
    char submit_buf[16385] = {0};
    if (raw_req && raw_req[0] == '{') {
      coax_t_CtSubmit *sub = calloc(1, sizeof *sub);
      unsigned char sarena[174];
      if (sub && ct_submit_read(ts(raw_req), (coax_v_t_CtSubmit){sub, 1}, tb(sarena, sizeof sarena)) && sub->blocks > 0) {
        int64_t s0 = sub->starts.e[0], c0 = sub->counts.e[0];
        if (s0 >= 0 && c0 > 0 && s0 + c0 <= (int64_t)sizeof sub->data.e && (size_t)c0 < sizeof submit_buf) {
          memcpy(submit_buf, sub->data.e + s0, (size_t)c0);
          submit_buf[c0] = 0;
          user_text = submit_buf;
        }
      }
      free(sub);
    }
    const char *att = PQgetvalue(tr, i, 10);
    if (att && !strcmp(att, "[]")) att = "";
    coax_t_CcTurnSummary ts_row = {
      .id = ts(PQgetvalue(tr, i, 0)),
      .ordinal = number(tr, i, 1),
      .state = (int)number(tr, i, 2),
      .compute = ts(PQgetvalue(tr, i, 3)),
      .model = ts(PQgetvalue(tr, i, 4)),
      .cost_msat = number(tr, i, 5),
      .source_device = ts(PQgetvalue(tr, i, 6)),
      .workspace = ts(PQgetvalue(tr, i, 7)),
      .user_text = ts(user_text),
      .output = ts(PQgetvalue(tr, i, 9)),
      .attachments = ts(att),
      .stop_reason = ts(PQgetvalue(tr, i, 11)),
      .error = ts(PQgetvalue(tr, i, 12)),
      .has_usage = has_u,
      .usage = u,
      .sequence = number(tr, i, 18),
      .created_at_ms = number(tr, i, 19),
      .updated_at_ms = number(tr, i, 20),
    };
    int64_t tn = cc_turn_summary_write((coax_v_t_CcTurnSummary){&ts_row, 1}, tb(tmp, 262143));
    if (tn < 1) goto done;
    tmp[tn] = 0;
    turn_owned[tcount] = strdup((char *)tmp);
    if (!turn_owned[tcount]) goto done;
    turn_items[tcount] = ts(turn_owned[tcount]);
    tcount++;
  }

  er = PQexecParams(db,
    "SELECT record FROM chat.turn_events_v2 WHERE account=$1 AND conversation=$2 ORDER BY event_id ASC LIMIT 512",
    2, NULL, args, NULL, NULL, 0);
  if (!tuples(er)) goto done;
  size_t ev_bytes = 0;
  for (int i = 0; i < PQntuples(er) && i < 512; i++) {
    const char *rec = PQgetvalue(er, i, 0);
    size_t rlen = (size_t)PQgetlength(er, i, 0);
    if (ecount && ev_bytes + rlen > 700000) break;
    ev_items[ecount++] = tn(rec, (int64_t)rlen);
    ev_bytes += rlen;
  }

  int64_t n = cc_conv_detail_write((coax_v_t_CcConvSummary){&cs, 1},
                                   (coax_v_string){turn_items, tcount},
                                   (coax_v_string){ev_items, ecount},
                                   tb(out, CT_CAP));
  if (n < 1) goto done;
  reply(q, 200, out, (size_t)n);
  goto finish;

done:
  refuse(q, status, why);
finish:
  (void)exec_sql(db, "ROLLBACK", 0, NULL);
  for (int i = 0; i < tcount; i++) free(turn_owned[i]);
  PQclear(er);
  PQclear(tr);
  PQclear(cr);
  free(tmp);
  free(out);
}

static void cloud_sync(const request *q, PGconn *db, const auth_principal *p, const char *path_cid) {
  coax_t_CcSyncReq *req = calloc(1, sizeof *req);
  unsigned char *arena = calloc(1, 262144);
  unsigned char *ev_arena = calloc(1, 67072);
  unsigned char *out = calloc(1, 4096);
  PGresult *r = NULL;
  bool tx = false;
  int status = 503;
  const char *why = "Chat sync unavailable.";
  char op[81] = {0}, cid[37] = {0}, tid[37] = {0}, hash[65] = {0};
  char title[161] = {0}, compute[16] = {0}, model[161] = {0}, dev[81] = {0}, ws[513] = {0};
  char stop_reason[81] = {0}, err_s[513] = {0};
  char *user_text = NULL, *output = NULL, *att_s = NULL;

  if (!req || !arena || !ev_arena || !out) goto done;
  if (!cc_sync_read(tn(q->body, q->body_len), (coax_v_t_CcSyncReq){req, 1}, tb(arena, 262144))) {
    status = 400; why = "Invalid cloud chat sync payload."; goto done;
  }
  copy_string(op, sizeof op, req->operation);
  copy_string(cid, sizeof cid, req->conversation);
  copy_string(tid, sizeof tid, req->turn);
  copy_string(title, sizeof title, req->title);
  copy_string(compute, sizeof compute, req->compute);
  copy_string(model, sizeof model, req->model);
  copy_string(dev, sizeof dev, req->source_device);
  copy_string(ws, sizeof ws, req->workspace);
  copy_string(stop_reason, sizeof stop_reason, req->stop_reason);
  copy_string(err_s, sizeof err_s, req->error);
  user_text = strndup(req->user_text.data ? req->user_text.data : "", (size_t)req->user_text.count);
  output = strndup(req->output.data ? req->output.data : "", (size_t)req->output.count);
  att_s = req->attachments.count > 0
    ? strndup(req->attachments.data, (size_t)req->attachments.count)
    : strdup("[]");
  if (!user_text || !output || !att_s) goto done;

  if (path_cid && *path_cid) {
    if (*cid && strcmp(cid, path_cid)) {
      status = 400; why = "Conversation ID mismatch."; goto done;
    }
    snprintf(cid, sizeof cid, "%s", path_cid);
  }

  (void)pr_hash(tn(q->body, q->body_len), tb(hash, 64));
  tx = exec_sql(db, "BEGIN", 0, NULL);
  if (!tx || !cloud_sync_lock(db, p->account, op)) goto done;

  int64_t replay_n = 0;
  int prior = cloud_sync_receipt_check(db, p->account, op, hash, out, 4096, &replay_n);
  if (prior < 0) goto done;
  if (prior == 0) { status = 409; why = "Operation ID already used with different payload."; goto done; }
  if (prior == 2) {
    (void)exec_sql(db, "ROLLBACK", 0, NULL); tx = false;
    reply(q, 200, out, (size_t)replay_n);
    goto finish;
  }

  long long now = (long long)api_now_ms();
  char ms[32];
  num(ms, now);
  if (!*cid && !api_object_id(cid, "cnv_")) goto done;

  bool fresh_conv = false;
  long long rev = 1;
  bool eff_pinned = req->has_pinned ? req->pinned : false;
  bool eff_archived = req->has_archived ? req->archived : false;
  char eff_title[161] = {0}, eff_dev[81] = {0}, eff_ws[513] = {0};

  const char *cargs[] = {p->account, cid};
  r = PQexecParams(db,
    "SELECT revision,title,pinned,archived,source_device,workspace "
    "FROM chat.conversations_v2 WHERE account=$1 AND id=$2 FOR UPDATE",
    2, NULL, cargs, NULL, NULL, 0);
  if (!tuples(r)) goto done;
  if (PQntuples(r) == 1) {
    rev = number(r, 0, 0) + 1;
    snprintf(eff_title, sizeof eff_title, "%s", *title ? title : PQgetvalue(r, 0, 1));
    eff_pinned = req->has_pinned ? req->pinned : (PQgetvalue(r, 0, 2)[0] == 't');
    eff_archived = req->has_archived ? req->archived : (PQgetvalue(r, 0, 3)[0] == 't');
    snprintf(eff_dev, sizeof eff_dev, "%s", *dev ? dev : PQgetvalue(r, 0, 4));
    snprintf(eff_ws, sizeof eff_ws, "%s", *ws ? ws : PQgetvalue(r, 0, 5));
    PQclear(r); r = NULL;
    char rev_s[32]; num(rev_s, rev);
    const char *uv[] = {p->account, cid, rev_s, eff_title, eff_pinned ? "true" : "false",
                        eff_archived ? "true" : "false", eff_dev, eff_ws, ms};
    if (!exec_sql(db,
      "UPDATE chat.conversations_v2 SET revision=$3::bigint,title=$4,pinned=$5::bool,"
      "archived=$6::bool,source_device=$7,workspace=$8,updated_ms=$9::bigint "
      "WHERE account=$1 AND id=$2", 9, uv)) goto done;
  } else {
    PQclear(r); r = NULL;
    if (path_cid && *path_cid) { status = 404; why = "not found"; goto done; }
    fresh_conv = true;
    rev = 1;
    snprintf(eff_title, sizeof eff_title, "%s", *title ? title : "New chat");
    snprintf(eff_dev, sizeof eff_dev, "%s", dev);
    snprintf(eff_ws, sizeof eff_ws, "%s", ws);
    const char *iv[] = {cid, p->account, eff_title, eff_dev, eff_ws,
                        eff_pinned ? "true" : "false", eff_archived ? "true" : "false", ms};
    PGresult *ins = PQexecParams(db,
      "INSERT INTO chat.conversations_v2(id,account,revision,title,source_device,workspace,pinned,archived,created_ms,updated_ms) "
      "VALUES($1,$2,1,$3,$4,$5,$6::bool,$7::bool,$8::bigint,$8::bigint)",
      8, NULL, iv, NULL, NULL, 0);
    if (!ins || PQresultStatus(ins) != PGRES_COMMAND_OK) {
      const char *sqlstate = ins ? PQresultErrorField(ins, PG_DIAG_SQLSTATE) : NULL;
      if (sqlstate && !strcmp(sqlstate, "23505")) { status = 409; why = "Conversation ID conflict."; }
      PQclear(ins);
      goto done;
    }
    PQclear(ins);
  }

  bool has_turn = (*tid != 0) || (req->state > 0) || (*user_text != 0) || (req->event_count > 0);
  bool fresh_turn = false;
  int eff_state = (int)(req->state > 0 ? req->state : RUNNING);
  long long last_event_id = 0;

  if (has_turn) {
    if (!*tid && !api_object_id(tid, "trn_")) goto done;
    const char *targs[] = {p->account, tid};
    r = PQexecParams(db,
      "SELECT sequence,state,conversation FROM chat.turns_v2 WHERE account=$1 AND id=$2 FOR UPDATE",
      2, NULL, targs, NULL, NULL, 0);
    if (!tuples(r)) goto done;
    char cost_s[32], st_s[32], u[5][32];
    num(cost_s, (long long)req->cost_msat);
    num(st_s, eff_state);
    long long counts[] = {req->usage.input, req->usage.output, req->usage.cache_read, req->usage.cache_write, req->usage.reasoning};
    for (int i = 0; i < 5; i++) {
      u[i][0] = 0;
      if (req->has_usage && counts[i] >= 0) num(u[i], counts[i]);
    }
    if (PQntuples(r) == 1) {
      if (strcmp(PQgetvalue(r, 0, 2), cid)) {
        PQclear(r); r = NULL;
        status = 409; why = "Turn belongs to a different conversation."; goto done;
      }
      if (req->state == 0) eff_state = (int)number(r, 0, 1);
      num(st_s, eff_state);
      PQclear(r); r = NULL;
      const char *utv[] = {
        p->account, tid, st_s, compute, model, cost_s, eff_dev, eff_ws,
        output, att_s, stop_reason, err_s, u[0], u[1], u[2], u[3], u[4], ms,
        req->has_usage ? "true" : "false",
        req->attachments.count > 0 ? "true" : "false"
      };
      if (!exec_sql(db,
        "UPDATE chat.turns_v2 SET state=$3::int,compute=$4,model=$5,cost_msat=$6::bigint,"
        "source_device=CASE WHEN $7<>'' THEN $7 ELSE source_device END,"
        "workspace=CASE WHEN $8<>'' THEN $8 ELSE workspace END,"
        "output=CASE WHEN $9<>'' THEN $9 ELSE output END,"
        "attachments_json=CASE WHEN $20::bool THEN $10 ELSE attachments_json END,"
        "stop_reason=CASE WHEN $11<>'' THEN $11 ELSE stop_reason END,"
        "error=CASE WHEN $12<>'' THEN $12 ELSE error END,"
        "input_tokens=CASE WHEN $19::bool THEN nullif($13,'')::bigint ELSE input_tokens END,"
        "output_tokens=CASE WHEN $19::bool THEN nullif($14,'')::bigint ELSE output_tokens END,"
        "cache_read_tokens=CASE WHEN $19::bool THEN nullif($15,'')::bigint ELSE cache_read_tokens END,"
        "cache_write_tokens=CASE WHEN $19::bool THEN nullif($16,'')::bigint ELSE cache_write_tokens END,"
        "reasoning_tokens=CASE WHEN $19::bool THEN nullif($17,'')::bigint ELSE reasoning_tokens END,"
        "updated_ms=$18::bigint WHERE account=$1 AND id=$2",
        20, utv)) goto done;
    } else {
      PQclear(r); r = NULL;
      fresh_turn = true;
      // Close any stale active turn in this conversation before starting a new active turn.
      if (eff_state == Q || eff_state == RUNNING) {
        const char *clv[] = {p->account, cid, ms, tid};
        if (!exec_sql(db,
          "UPDATE chat.turns_v2 SET state=CASE WHEN output<>'' THEN 3 ELSE 6 END,updated_ms=$3::bigint "
          "WHERE account=$1 AND conversation=$2 AND id<>$4 AND state IN(1,2)", 4, clv)) goto done;
      }
      r = PQexecParams(db,
        "SELECT COALESCE(max(ordinal),0)+1 FROM chat.turns_v2 WHERE account=$1 AND conversation=$2",
        2, NULL, cargs, NULL, NULL, 0);
      if (!tuples(r) || PQntuples(r) != 1) goto done;
      char ord_s[32];
      num(ord_s, number(r, 0, 0));
      PQclear(r); r = NULL;
      const char *prov = !strcmp(compute, "local") ? "local" : "openagents";
      const char *itv[] = {
        tid, p->account, cid, ord_s, op, hash, user_text, "{}", st_s,
        compute, model, prov, "medium", cost_s, eff_dev, eff_ws, att_s,
        output, stop_reason, err_s, u[0], u[1], u[2], u[3], u[4], ms
      };
      PGresult *ins = PQexecParams(db,
        "INSERT INTO chat.turns_v2("
        "id,account,conversation,ordinal,operation,digest,request_json,input_json,state,"
        "compute,model,provider,effort,cost_msat,source_device,workspace,attachments_json,"
        "output,stop_reason,error,input_tokens,output_tokens,cache_read_tokens,cache_write_tokens,"
        "reasoning_tokens,created_ms,updated_ms,native_input_json) "
        "VALUES($1,$2,$3,$4::bigint,$5,$6,$7,$8,$9::int,$10,$11,$12,$13,$14::bigint,$15,$16,$17,"
        "$18,$19,$20,nullif($21,'')::bigint,nullif($22,'')::bigint,nullif($23,'')::bigint,"
        "nullif($24,'')::bigint,nullif($25,'')::bigint,$26::bigint,$26::bigint,'{}')",
        26, NULL, itv, NULL, NULL, 0);
      if (!ins || PQresultStatus(ins) != PGRES_COMMAND_OK) {
        const char *sqlstate = ins ? PQresultErrorField(ins, PG_DIAG_SQLSTATE) : NULL;
        if (sqlstate && !strcmp(sqlstate, "23505")) { status = 409; why = "Turn ID or active turn conflict."; }
        PQclear(ins);
        goto done;
      }
      PQclear(ins);
    }

    long long max_seq = 0;
    for (int64_t i = 0; i < req->event_count; i++) {
      int64_t from = req->event_starts.e[i], to = req->event_ends.e[i];
      if (from < 0 || to <= from || (size_t)to > q->body_len) goto done;
      coax_t_CcEvent ev = {0};
      if (!cc_event_read(tn(q->body + from, to - from), (coax_v_t_CcEvent){&ev, 1}, tb(ev_arena, 67072))) {
        status = 400; why = "Invalid event item."; goto done;
      }
      char *rec = strndup(q->body + from, (size_t)(to - from));
      if (!rec) goto done;
      char seq_s[32]; num(seq_s, (long long)ev.sequence);
      const char *evp[] = {p->account, tid, seq_s, cid, rec};
      PGresult *er = PQexecParams(db,
        "INSERT INTO chat.turn_events_v2(account,turn,sequence,conversation,record) "
        "VALUES($1,$2,$3::bigint,$4,$5) ON CONFLICT (turn,sequence) DO NOTHING RETURNING event_id",
        5, NULL, evp, NULL, NULL, 0);
      free(rec);
      if (!tuples(er)) { PQclear(er); goto done; }
      if (PQntuples(er) == 1) {
        long long eid = number(er, 0, 0);
        if (eid > last_event_id) last_event_id = eid;
      }
      PQclear(er);
      if (ev.sequence > max_seq) max_seq = ev.sequence;
    }
    if (max_seq > 0) {
      char mseq[32]; num(mseq, max_seq);
      const char *sv[] = {p->account, tid, mseq, ms};
      if (!exec_sql(db,
        "UPDATE chat.turns_v2 SET sequence=GREATEST(sequence,$3::bigint),updated_ms=$4::bigint "
        "WHERE account=$1 AND id=$2", 4, sv)) goto done;
    }
  }

  const char *ev_type = fresh_conv ? "conversation_created"
    : (has_turn && eff_state == DONE) ? "turn_completed"
    : (has_turn && eff_state == FAILED) ? "turn_failed"
    : (has_turn && eff_state == CANCELLED) ? "turn_cancelled"
    : fresh_turn ? "turn_started"
    : "conversation_updated";
  if (!cloud_emit_account_event(db, p->account, cid, tid, ev_type, eff_title,
                                compute, model, (long long)req->cost_msat,
                                eff_pinned, eff_archived, eff_dev, rev, now)) goto done;

  int64_t rn = cc_sync_reply_write(ts(cid), ts(tid), rev, last_event_id, false, tb(out, 4096));
  if (rn < 1) goto done;
  if (!cloud_sync_receipt_save(db, p->account, op, hash, cid, tid, rev, last_event_id, now)) goto done;
  if (!exec_sql(db, "COMMIT", 0, NULL)) goto done;
  tx = false;
  reply(q, 200, out, (size_t)rn);
  goto finish;

done:
  if (tx) (void)exec_sql(db, "ROLLBACK", 0, NULL);
  refuse(q, status, why);
finish:
  PQclear(r);
  free(user_text);
  free(output);
  free(att_s);
  free(out);
  free(ev_arena);
  free(arena);
  free(req);
}

static void cloud_meta(const request *q, PGconn *db, const auth_principal *p, const char *cid) {
  coax_t_CcMetaReq req = {0};
  unsigned char arena[512], out[1024];
  PGresult *r = NULL;
  bool tx = false;
  int status = 503;
  const char *why = "Metadata update unavailable.";
  char op[81] = {0}, hash[65] = {0}, title[161] = {0};

  if (!cc_meta_read(tn(q->body, q->body_len), (coax_v_t_CcMetaReq){&req, 1}, tb(arena, sizeof arena))) {
    status = 400; why = "Invalid conversation metadata request."; goto done;
  }
  copy_string(op, sizeof op, req.operation);
  if (req.has_title) copy_string(title, sizeof title, req.title);
  (void)pr_hash(tn(q->body, q->body_len), tb(hash, 64));

  tx = exec_sql(db, "BEGIN", 0, NULL);
  if (!tx || !cloud_sync_lock(db, p->account, op)) goto done;

  int64_t replay_n = 0;
  int prior = cloud_sync_receipt_check(db, p->account, op, hash, out, sizeof out, &replay_n);
  if (prior < 0) goto done;
  if (prior == 0) { status = 409; why = "Operation ID already used with different payload."; goto done; }
  if (prior == 2) {
    (void)exec_sql(db, "ROLLBACK", 0, NULL); tx = false;
    reply(q, 200, out, (size_t)replay_n);
    goto finish;
  }

  const char *cargs[] = {p->account, cid};
  r = PQexecParams(db,
    "SELECT revision,title,pinned,archived,source_device FROM chat.conversations_v2 "
    "WHERE account=$1 AND id=$2 FOR UPDATE",
    2, NULL, cargs, NULL, NULL, 0);
  if (!tuples(r)) goto done;
  if (PQntuples(r) != 1) { status = 404; why = "not found"; goto done; }

  long long rev = number(r, 0, 0) + 1;
  char eff_title[161] = {0}, eff_dev[81] = {0};
  snprintf(eff_title, sizeof eff_title, "%s", req.has_title ? title : PQgetvalue(r, 0, 1));
  bool eff_pinned = req.has_pinned ? req.pinned : (PQgetvalue(r, 0, 2)[0] == 't');
  bool eff_archived = req.has_archived ? req.archived : (PQgetvalue(r, 0, 3)[0] == 't');
  snprintf(eff_dev, sizeof eff_dev, "%s", PQgetvalue(r, 0, 4));
  PQclear(r); r = NULL;

  long long now = (long long)api_now_ms();
  char rev_s[32], ms[32];
  num(rev_s, rev); num(ms, now);
  const char *uv[] = {p->account, cid, rev_s, eff_title, eff_pinned ? "true" : "false",
                      eff_archived ? "true" : "false", ms};
  if (!exec_sql(db,
    "UPDATE chat.conversations_v2 SET revision=$3::bigint,title=$4,pinned=$5::bool,"
    "archived=$6::bool,updated_ms=$7::bigint WHERE account=$1 AND id=$2",
    7, uv)) goto done;

  if (!cloud_emit_account_event(db, p->account, cid, "", "conversation_updated", eff_title,
                                "", "", 0, eff_pinned, eff_archived, eff_dev, rev, now)) goto done;

  int64_t rn = cc_sync_reply_write(ts(cid), ts(""), rev, 0, false, tb(out, sizeof out));
  if (rn < 1) goto done;
  if (!cloud_sync_receipt_save(db, p->account, op, hash, cid, "", rev, 0, now)) goto done;
  if (!exec_sql(db, "COMMIT", 0, NULL)) goto done;
  tx = false;
  reply(q, 200, out, (size_t)rn);
  goto finish;

done:
  if (tx) (void)exec_sql(db, "ROLLBACK", 0, NULL);
  refuse(q, status, why);
finish:
  PQclear(r);
}

static bool account_chat_cloud_dispatch(const request *q, int rt, const char *cid) {
  if (rt == -1) { refuse(q, 405, "Method not allowed."); return true; }
  PGconn *db = api_db();
  if (!db) { refuse(q, 503, "Chat storage unavailable."); return true; }
  const auth_principal *p = auth_principal_of(q);
  if (!p || !ctw_owns(p->kind, p->level, ts(p->account), ts(p->account))) {
    refuse(q, 401, "Log in first.");
    return true;
  }
  bool post = (rt == 5 || rt == 6 || rt == 7);
  if (post && !auth_same_site(q)) { refuse(q, 403, "Cross-site write refused."); return true; }
  if (rt == 1) cloud_stream_account(q, db, p);
  else if (rt == 2) cloud_conv_list(q, db, p);
  else if (rt == 3) cloud_conv_get(q, db, p, cid);
  else if (rt == 4) cloud_stream_conv(q, db, p, cid);
  else if (rt == 5) cloud_sync(q, db, p, "");
  else if (rt == 6) cloud_sync(q, db, p, cid);
  else if (rt == 7) cloud_meta(q, db, p, cid);
  return true;
}
