// Signed runner lifecycle & live event ingest for Cloud Sandbox turns (Neon Core on GCE via oa-boat).
// Included by account-chat-turns.c; all events persisted to chat.turn_events_v2 are encoded with the
// #authority cc_event_write codec (openagents.chat.event.v2) so Web, Desktop, and Mobile SSE clients
// render thinking, tool calls, diffs, output deltas, and terminal state identically.
#include "workos.h"
#include <ctype.h>
#include <openssl/crypto.h>

enum {
  SB_CC_STARTED = 1,
  SB_CC_USER_MESSAGE = 2,
  SB_CC_TEXT_DELTA = 3,
  SB_CC_THINKING_DELTA = 4,
  SB_CC_TOOL_CALL = 5,
  SB_CC_TOOL_PROGRESS = 6,
  SB_CC_TOOL_RESULT = 7,
  SB_CC_APPROVAL = 8,
  SB_CC_USAGE = 9,
  SB_CC_COMPLETED = 10,
  SB_CC_FAILED = 11,
  SB_CC_CANCELLED = 12,
  SB_CC_UNKNOWN = 13
};

static const char *sb_skip_ws(const char *p, const char *end) {
  while (p < end && (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n')) p++;
  return p;
}

static const char *sb_skip_val(const char *p, const char *end) {
  p = sb_skip_ws(p, end);
  if (p >= end) return p;
  if (*p == '"') {
    for (p++; p < end && *p != '"'; p++) if (*p == '\\' && p + 1 < end) p++;
    return p < end ? p + 1 : p;
  }
  if (*p == '{' || *p == '[') {
    int depth = 0;
    for (; p < end; p++) {
      if (*p == '"') {
        for (p++; p < end && *p != '"'; p++) if (*p == '\\' && p + 1 < end) p++;
        if (p >= end) return end;
        continue;
      }
      if (*p == '{' || *p == '[') depth++;
      else if (*p == '}' || *p == ']') { if (--depth == 0) return p + 1; }
    }
    return end;
  }
  while (p < end && *p != ',' && *p != '}' && *p != ']' && *p != ' ' && *p != '\n' && *p != '\r' && *p != '\t') p++;
  return p;
}

static bool sb_jget(const char *json, size_t n, const char *key, const char **at, size_t *len) {
  const char *end = json + n, *p = sb_skip_ws(json, end);
  if (p >= end || *p != '{') return false;
  p++;
  size_t kn = strlen(key);
  for (;;) {
    p = sb_skip_ws(p, end);
    if (p >= end || *p != '"') return false;
    const char *k = p + 1, *ke = sb_skip_val(p, end);
    bool match = ke > k && (size_t)(ke - k - 1) == kn && !memcmp(k, key, kn);
    p = sb_skip_ws(ke, end);
    if (p >= end || *p != ':') return false;
    const char *v = sb_skip_ws(p + 1, end), *ve = sb_skip_val(v, end);
    if (match) { *at = v; *len = (size_t)(ve - v); return true; }
    p = sb_skip_ws(ve, end);
    if (p >= end || *p != ',') return false;
    p++;
  }
}

static size_t sb_jstr(const char *json, size_t n, const char *key, char *out, size_t cap) {
  if (!cap) return 0;
  out[0] = 0;
  const char *at = NULL; size_t len = 0;
  if (!sb_jget(json, n, key, &at, &len) || len < 2 || *at != '"') return 0;
  size_t w = 0;
  for (size_t i = 1; i + 1 < len && w + 1 < cap; i++) {
    char c = at[i];
    if (c != '\\') { out[w++] = c; continue; }
    if (i + 1 >= len - 1) break;
    char e = at[++i];
    if (e == 'n') out[w++] = '\n';
    else if (e == 'r') out[w++] = '\r';
    else if (e == 't') out[w++] = '\t';
    else if (e == 'b') out[w++] = '\b';
    else if (e == 'f') out[w++] = '\f';
    else if (e == 'u' && i + 4 < len - 1) {
      unsigned v = 0;
      char h[5] = {at[i + 1], at[i + 2], at[i + 3], at[i + 4], 0};
      if (sscanf(h, "%4x", &v) == 1) {
        if (v < 0x80) out[w++] = (char)v;
        else if (v < 0x800 && w + 2 < cap) {
          out[w++] = (char)(0xC0 | (v >> 6));
          out[w++] = (char)(0x80 | (v & 0x3F));
        } else if (w + 3 < cap) {
          out[w++] = (char)(0xE0 | (v >> 12));
          out[w++] = (char)(0x80 | ((v >> 6) & 0x3F));
          out[w++] = (char)(0x80 | (v & 0x3F));
        }
      }
      i += 4;
    } else out[w++] = e;
  }
  out[w] = 0;
  return w;
}

static long long sb_jint(const char *json, size_t n, const char *key, long long defv) {
  const char *at = NULL; size_t len = 0;
  if (!sb_jget(json, n, key, &at, &len) || len == 0) return defv;
  if (*at != '-' && !isdigit((unsigned char)*at)) return defv;
  char tmp[32];
  size_t k = len < sizeof tmp - 1 ? len : sizeof tmp - 1;
  memcpy(tmp, at, k); tmp[k] = 0;
  return strtoll(tmp, NULL, 10);
}

static bool sb_jbool(const char *json, size_t n, const char *key) {
  const char *at = NULL; size_t len = 0;
  return sb_jget(json, n, key, &at, &len) && len == 4 && !memcmp(at, "true", 4);
}

typedef struct { char *p; size_t n, cap; } sb_buf;
static bool sb_grow(sb_buf *b, size_t add) {
  if (b->n + add + 1 <= b->cap) return true;
  size_t cap = b->cap ? b->cap * 2 : 4096;
  while (cap < b->n + add + 1) cap *= 2;
  if (cap > (4u << 20)) return false;
  char *np = realloc(b->p, cap);
  if (!np) return false;
  b->p = np; b->cap = cap;
  return true;
}
static bool sb_puts(sb_buf *b, const char *s) {
  size_t k = strlen(s);
  if (!sb_grow(b, k)) return false;
  memcpy(b->p + b->n, s, k); b->n += k; b->p[b->n] = 0;
  return true;
}
static bool sb_putq(sb_buf *b, const char *s) {
  if (!s) s = "";
  if (!sb_puts(b, "\"")) return false;
  for (const unsigned char *c = (const unsigned char *)s; *c; c++) {
    if (*c == '"' || *c == '\\') {
      char e[3] = {'\\', (char)*c, 0};
      if (!sb_puts(b, e)) return false;
    } else if (*c == '\n') { if (!sb_puts(b, "\\n")) return false; }
    else if (*c == '\r') { if (!sb_puts(b, "\\r")) return false; }
    else if (*c == '\t') { if (!sb_puts(b, "\\t")) return false; }
    else if (*c < 0x20) {
      char u[8]; snprintf(u, sizeof u, "\\u%04x", *c);
      if (!sb_puts(b, u)) return false;
    } else {
      char ch[2] = {(char)*c, 0};
      if (!sb_puts(b, ch)) return false;
    }
  }
  return sb_puts(b, "\"");
}

static size_t sb_utf8_clamp(const char *s, size_t max_bytes) {
  size_t n = strlen(s);
  if (n <= max_bytes) return n;
  size_t k = max_bytes;
  while (k > 0 && ((unsigned char)s[k] & 0xC0) == 0x80) k--;
  return k;
}

static bool sb_insert_cc_event(PGconn *db, const char *account, const char *cid, const char *tid,
                               long long seq, int kind, int state, const char *text,
                               const char *tool_id, const char *tool_name, const char *tool_args,
                               const char *diff, bool failed, const char *model, long long cost_msat,
                               const char *source_device, const char *workspace, const char *stop_reason,
                               long long now_ms) {
  unsigned char *wire = calloc(1, 131073);
  if (!wire) return false;
  size_t txtn = text ? sb_utf8_clamp(text, 16000) : 0;
  size_t tidn = tool_id ? sb_utf8_clamp(tool_id, 120) : 0;
  size_t tnm = tool_name ? sb_utf8_clamp(tool_name, 120) : 0;
  size_t tan = tool_args ? sb_utf8_clamp(tool_args, 16000) : 0;
  size_t dn = diff ? sb_utf8_clamp(diff, 32000) : 0;
  coax_t_CcEvent ev = {
    .conversation = ts(cid),
    .turn = ts(tid),
    .sequence = seq,
    .kind = kind,
    .state = state,
    .text = tn(text ? text : "", (int64_t)txtn),
    .tool_id = tn(tool_id ? tool_id : "", (int64_t)tidn),
    .tool_name = tn(tool_name ? tool_name : "", (int64_t)tnm),
    .tool_args = tn(tool_args ? tool_args : "", (int64_t)tan),
    .diff = tn(diff ? diff : "", (int64_t)dn),
    .failed = failed,
    .compute = ts("sandbox"),
    .model = ts(model && *model ? model : "google/gemini-4-neon"),
    .cost_msat = cost_msat >= 0 ? cost_msat : 0,
    .source_device = ts(source_device && *source_device ? source_device : "cloud-sandbox"),
    .workspace = ts(workspace ? workspace : ""),
    .has_usage = false,
    .usage = {-1, -1, -1, -1, -1},
    .stop_reason = ts(stop_reason ? stop_reason : ""),
    .created_at_ms = now_ms,
  };
  int64_t wn = cc_event_write((coax_v_t_CcEvent){&ev, 1}, tb(wire, 131072));
  bool ok = false;
  if (wn > 0) {
    wire[wn] = 0;
    char seq_s[32]; num(seq_s, seq);
    const char *p[] = {account, tid, seq_s, cid, (char *)wire};
    ok = exec_sql(db,
      "INSERT INTO chat.turn_events_v2(account,turn,sequence,conversation,record) "
      "VALUES($1,$2,$3::bigint,$4,$5) ON CONFLICT (turn,sequence) DO NOTHING", 5, p);
  }
  free(wire);
  return ok;
}

static void sb_parse_workspace(const char *ws, char size_out[16], char repo_out[130], char ref_out[130]) {
  snprintf(size_out, 16, "small");
  repo_out[0] = 0;
  ref_out[0] = 0;
  if (!ws || strncmp(ws, "sandbox:", 8) != 0) return;
  const char *p = ws + 8;
  const char *c = strchr(p, ':');
  if (!c) {
    if (*p) snprintf(size_out, 16, "%.15s", p);
    return;
  }
  size_t sl = (size_t)(c - p);
  if (sl > 0 && sl < 16) { memcpy(size_out, p, sl); size_out[sl] = 0; }
  const char *r = c + 1;
  const char *at = strchr(r, '@');
  if (at) {
    size_t rl = (size_t)(at - r);
    if (rl > 0 && rl < 130) { memcpy(repo_out, r, rl); repo_out[rl] = 0; }
    snprintf(ref_out, 130, "%.129s", at + 1);
  } else if (*r) {
    snprintf(repo_out, 130, "%.129s", r);
  }
}

static void sb_extract_request_text(const char *raw_req, char *out, size_t cap) {
  out[0] = 0;
  if (!raw_req || !*raw_req) return;
  if (raw_req[0] != '{') { snprintf(out, cap, "%s", raw_req); return; }
  coax_t_CtSubmit *sub = calloc(1, sizeof *sub);
  unsigned char arena[174];
  if (sub && ct_submit_read(ts(raw_req), (coax_v_t_CtSubmit){sub, 1}, tb(arena, sizeof arena)) && sub->blocks > 0) {
    size_t w = 0;
    for (int i = 0; i < sub->blocks; i++) {
      int64_t s0 = sub->starts.e[i], c0 = sub->counts.e[i];
      if (s0 < 0 || c0 <= 0 || s0 + c0 > (int64_t)sizeof sub->data.e) continue;
      if (w > 0 && w + 1 < cap) out[w++] = '\n';
      size_t take = (size_t)c0;
      if (w + take >= cap) take = cap - 1 - w;
      memcpy(out + w, sub->data.e + s0, take);
      w += take;
    }
    out[w] = 0;
  }
  free(sub);
}

static void sandbox_runner_begin(const request *q, PGconn *db, const char *key,
                                 const char *tid, const char *dispatch, const char *worker) {
  bool tx = exec_sql(db, "BEGIN", 0, NULL);
  PGresult *r = NULL, *pr = NULL, *br = NULL, *hr = NULL;
  sb_buf resp = {0}, prompt = {0};
  char cloud_tok[160] = {0}, tok_sid[40] = {0}, gh_tok[512] = {0}, gh_err[128] = {0};
  int status = 503;
  const char *why = "Sandbox begin unavailable.";
  if (!tx) goto done;

  const char *tv[] = {tid};
  r = PQexecParams(db,
    "SELECT t.account,t.conversation,t.state,coalesce(t.compute,''),coalesce(t.model,''),"
    "coalesce(t.workspace,''),coalesce(t.worker,''),coalesce(t.runner_key,''),coalesce(t.dispatch,''),"
    "t.request_json,coalesce(c.project,''),t.stop_requested,t.sequence,c.revision,coalesce(t.snapshot_json,'') "
    "FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation "
    "WHERE t.id=$1 FOR UPDATE OF t", 1, NULL, tv, NULL, NULL, 0);
  if (!tuples(r)) goto done;
  if (PQntuples(r) != 1) { status = 404; why = "not found"; goto done; }

  const char *account = PQgetvalue(r, 0, 0);
  const char *cid = PQgetvalue(r, 0, 1);
  int st = (int)number(r, 0, 2);
  const char *compute = PQgetvalue(r, 0, 3);
  const char *model = PQgetvalue(r, 0, 4);
  const char *workspace = PQgetvalue(r, 0, 5);
  const char *r_worker = PQgetvalue(r, 0, 6);
  const char *r_key = PQgetvalue(r, 0, 7);
  const char *r_disp = PQgetvalue(r, 0, 8);
  const char *req_json = PQgetvalue(r, 0, 9);
  const char *project = PQgetvalue(r, 0, 10);
  bool stop_req = PQgetvalue(r, 0, 11)[0] == 't';
  long long rev = number(r, 0, 13);

  if (strcmp(r_worker, worker) || strcmp(r_key, key) || strcmp(r_disp, dispatch)) {
    status = 409; why = "Worker claim was replaced."; goto done;
  }
  if (strcmp(compute, "sandbox") != 0) {
    (void)exec_sql(db, "ROLLBACK", 0, NULL); tx = false;
    reply(q, 200, "{\"sandbox\":false}", 17);
    goto finish;
  }
  long long now = (long long)api_now_ms();
  char ms[32], until[32];
  num(ms, now);
  num(until, now + 900000LL);
  if (ct_terminal(st) || stop_req) {
    if (!ct_terminal(st) && stop_req) {
      const char *cv[] = {account, tid, ms};
      (void)exec_sql(db, "UPDATE chat.turns_v2 SET state=5,stop_reason='user_stop',updated_ms=$3::bigint,heartbeat_ms=$3::bigint WHERE account=$1 AND id=$2", 3, cv);
      (void)sb_insert_cc_event(db, account, cid, tid, number(r, 0, 12) + 1, SB_CC_CANCELLED, CANCELLED,
                               "", "", "", "", "", false, model, 0, "cloud-sandbox", workspace, "user_stop", now);
      (void)cloud_emit_account_event(db, account, cid, tid, "turn_cancelled", "", "sandbox", model, 0, false, false, "cloud-sandbox", rev, now);
    }
    (void)exec_sql(db, "COMMIT", 0, NULL); tx = false;
    reply(q, 200, "{\"sandbox\":true,\"already_done\":true}", 36);
    goto finish;
  }

  const char *uv[] = {account, tid, ms, until};
  if (!exec_sql(db,
      "UPDATE chat.turns_v2 SET state=2,dispatched_ms=coalesce(dispatched_ms,$3::bigint),"
      "heartbeat_ms=$3::bigint,claim_until_ms=$4::bigint,sequence=GREATEST(sequence,1),updated_ms=$3::bigint "
      "WHERE account=$1 AND id=$2", 4, uv)) goto done;

  (void)sb_insert_cc_event(db, account, cid, tid, 1, SB_CC_STARTED, RUNNING,
                           "", "", "", "", "", false, model, 0, "cloud-sandbox", workspace, "", now);

  char size[16], repo[130], ref[130], warm_box[32] = {0};
  sb_parse_workspace(workspace, size, repo, ref);

  if (!repo[0] && *project) {
    const char *pv[] = {account, project};
    pr = PQexecParams(db, "SELECT coalesce(repo_canonical,'') FROM projects.private_v1 WHERE account=$1 AND id=$2",
                      2, NULL, pv, NULL, NULL, 0);
    if (tuples(pr) && PQntuples(pr) == 1) {
      const char *rc = PQgetvalue(pr, 0, 0);
      if (rc && *rc) {
        if (!strncmp(rc, "https://github.com/", 19)) rc += 19;
        else if (!strncmp(rc, "github.com/", 11)) rc += 11;
        snprintf(repo, sizeof repo, "%.129s", rc);
        size_t rl = strlen(repo);
        if (rl > 4 && !strcmp(repo + rl - 4, ".git")) repo[rl - 4] = 0;
      }
    }
  }

  const char *bv[] = {account, cid, tid};
  br = PQexecParams(db,
    "SELECT source_device FROM chat.turns_v2 "
    "WHERE account=$1 AND conversation=$2 AND id<>$3 AND compute='sandbox' AND source_device LIKE 'bx_%' "
    "ORDER BY ordinal DESC LIMIT 1", 3, NULL, bv, NULL, NULL, 0);
  if (tuples(br) && PQntuples(br) == 1) {
    snprintf(warm_box, sizeof warm_box, "%.31s", PQgetvalue(br, 0, 0));
  }

  // Assemble multi-turn conversation prompt so Neon Core in the cloud sandbox has full prior turn context.
  const char *hv[] = {account, cid, tid};
  hr = PQexecParams(db,
    "SELECT request_json,output FROM chat.turns_v2 "
    "WHERE account=$1 AND conversation=$2 AND id<>$3 AND state=3 ORDER BY ordinal ASC LIMIT 16",
    3, NULL, hv, NULL, NULL, 0);
  char *turn_txt = malloc(32769);
  if (!turn_txt) goto done;
  if (tuples(hr) && PQntuples(hr) > 0) {
    sb_puts(&prompt, "Previous conversation turns in this workspace:\n");
    for (int i = 0; i < PQntuples(hr); i++) {
      sb_extract_request_text(PQgetvalue(hr, i, 0), turn_txt, 4096);
      if (*turn_txt) {
        sb_puts(&prompt, "User: "); sb_puts(&prompt, turn_txt); sb_puts(&prompt, "\n");
      }
      const char *ans = PQgetvalue(hr, i, 1);
      if (ans && *ans) {
        size_t al = sb_utf8_clamp(ans, 4096);
        char *ac = strndup(ans, al);
        if (ac) { sb_puts(&prompt, "Assistant: "); sb_puts(&prompt, ac); sb_puts(&prompt, "\n"); free(ac); }
      }
    }
    sb_puts(&prompt, "\nCurrent request:\n");
  }
  sb_extract_request_text(req_json, turn_txt, 32768);
  sb_puts(&prompt, turn_txt);
  free(turn_txt);

  if (!auth_mint_sandbox_token(db, account, cloud_tok, tok_sid)) {
    status = 503; why = "Could not mint ephemeral cloud inference token."; goto done;
  }
  (void)workos_pipes_github_token(db, account, gh_tok, sizeof gh_tok, gh_err, sizeof gh_err);

  if (!exec_sql(db, "COMMIT", 0, NULL)) goto done;
  tx = false;

  const char *origin = getenv("OA_API_ORIGIN");
  if (!origin || !*origin) origin = "https://openagents.com";
  const char *eff_model = (model && *model) ? model : "google/gemini-4-neon";

  sb_puts(&resp, "{\"sandbox\":true,\"account\":"); sb_putq(&resp, account);
  sb_puts(&resp, ",\"conversation_id\":"); sb_putq(&resp, cid);
  sb_puts(&resp, ",\"turn_id\":"); sb_putq(&resp, tid);
  sb_puts(&resp, ",\"size\":"); sb_putq(&resp, size);
  sb_puts(&resp, ",\"model\":"); sb_putq(&resp, eff_model);
  sb_puts(&resp, ",\"box_id\":"); sb_putq(&resp, warm_box);
  sb_puts(&resp, ",\"repo\":"); sb_putq(&resp, repo);
  sb_puts(&resp, ",\"ref\":"); sb_putq(&resp, ref);
  sb_puts(&resp, ",\"prompt\":"); sb_putq(&resp, prompt.p ? prompt.p : "");
  sb_puts(&resp, ",\"cloud_origin\":"); sb_putq(&resp, origin);
  sb_puts(&resp, ",\"cloud_token\":"); sb_putq(&resp, cloud_tok);
  sb_puts(&resp, ",\"token_session\":"); sb_putq(&resp, tok_sid);
  sb_puts(&resp, ",\"gh_token\":"); sb_putq(&resp, gh_tok);
  sb_puts(&resp, "}");

  reply(q, 200, resp.p, resp.n);
  goto finish;

done:
  if (tx) (void)exec_sql(db, "ROLLBACK", 0, NULL);
  refuse(q, status, why);
finish:
  OPENSSL_cleanse(cloud_tok, sizeof cloud_tok);
  OPENSSL_cleanse(gh_tok, sizeof gh_tok);
  if (resp.p) OPENSSL_cleanse(resp.p, resp.n);
  free(resp.p);
  free(prompt.p);
  PQclear(hr); PQclear(br); PQclear(pr); PQclear(r);
}

static void sandbox_runner_event(const request *q, PGconn *db, const char *key,
                                 const char *tid, const char *dispatch, const char *worker) {
  char kind_s[32] = {0}, box_id[32] = {0}, tool_id[130] = {0}, tool_name[130] = {0};
  char *text_s = calloc(1, 16385), *tool_args = calloc(1, 16385), *diff_s = calloc(1, 32769);
  if (!text_s || !tool_args || !diff_s) { free(text_s); free(tool_args); free(diff_s); refuse(q, 503, "memory"); return; }

  sb_jstr(q->body, q->body_len, "kind", kind_s, sizeof kind_s);
  sb_jstr(q->body, q->body_len, "box_id", box_id, sizeof box_id);
  sb_jstr(q->body, q->body_len, "text", text_s, 16384);
  sb_jstr(q->body, q->body_len, "tool_id", tool_id, sizeof tool_id);
  sb_jstr(q->body, q->body_len, "tool_name", tool_name, sizeof tool_name);
  sb_jstr(q->body, q->body_len, "tool_args", tool_args, 16384);
  sb_jstr(q->body, q->body_len, "diff", diff_s, 32768);
  bool failed = sb_jbool(q->body, q->body_len, "failed");

  bool tx = exec_sql(db, "BEGIN", 0, NULL);
  PGresult *r = NULL;
  int status = 503;
  const char *why = "Event storage unavailable.";
  bool stop_req = false;
  if (!tx) goto done;

  const char *tv[] = {tid};
  r = PQexecParams(db,
    "SELECT account,conversation,state,stop_requested,sequence,coalesce(model,''),coalesce(workspace,''),"
    "coalesce(worker,''),coalesce(runner_key,''),coalesce(dispatch,''),coalesce(source_device,'cloud-sandbox') "
    "FROM chat.turns_v2 WHERE id=$1 FOR UPDATE", 1, NULL, tv, NULL, NULL, 0);
  if (!tuples(r)) goto done;
  if (PQntuples(r) != 1) { status = 404; why = "not found"; goto done; }

  const char *account = PQgetvalue(r, 0, 0);
  const char *cid = PQgetvalue(r, 0, 1);
  int st = (int)number(r, 0, 2);
  stop_req = PQgetvalue(r, 0, 3)[0] == 't';
  long long seq = number(r, 0, 4);
  const char *model = PQgetvalue(r, 0, 5);
  const char *workspace = PQgetvalue(r, 0, 6);
  const char *eff_dev = *box_id ? box_id : PQgetvalue(r, 0, 10);

  if (strcmp(PQgetvalue(r, 0, 7), worker) || strcmp(PQgetvalue(r, 0, 8), key) || strcmp(PQgetvalue(r, 0, 9), dispatch)) {
    status = 409; why = "Worker claim was replaced."; goto done;
  }
  long long now = (long long)api_now_ms();
  char ms[32], until[32];
  num(ms, now);
  num(until, now + 300000LL);

  if (!ct_terminal(st)) {
    int ev_kind = 0;
    if (!strcmp(kind_s, "thinking_delta")) ev_kind = SB_CC_THINKING_DELTA;
    else if (!strcmp(kind_s, "tool_call")) ev_kind = SB_CC_TOOL_CALL;
    else if (!strcmp(kind_s, "tool_progress")) ev_kind = SB_CC_TOOL_PROGRESS;
    else if (!strcmp(kind_s, "tool_result")) ev_kind = SB_CC_TOOL_RESULT;
    else if (!strcmp(kind_s, "text_delta")) ev_kind = SB_CC_TEXT_DELTA;

    long long next_seq = ev_kind > 0 ? (seq + 1) : seq;
    char seq_s[32]; num(seq_s, next_seq);
    const char *append_txt = (ev_kind == SB_CC_TEXT_DELTA) ? text_s : "";
    const char *uv[] = {account, tid, seq_s, ms, until, eff_dev, append_txt};
    if (!exec_sql(db,
        "UPDATE chat.turns_v2 SET sequence=$3::bigint,heartbeat_ms=$4::bigint,updated_ms=$4::bigint,"
        "claim_until_ms=$5::bigint,source_device=$6,"
        "output=CASE WHEN $7<>'' AND length(output)+length($7)<=131072 THEN output||$7 ELSE output END "
        "WHERE account=$1 AND id=$2", 7, uv)) goto done;

    if (ev_kind > 0) {
      if ((ev_kind == SB_CC_TOOL_CALL || ev_kind == SB_CC_TOOL_PROGRESS || ev_kind == SB_CC_TOOL_RESULT) && !*tool_name) {
        snprintf(tool_name, sizeof tool_name, "tool");
      }
      if (!sb_insert_cc_event(db, account, cid, tid, next_seq, ev_kind, RUNNING,
                              text_s, tool_id, tool_name, tool_args, diff_s, failed,
                              model, 0, eff_dev, workspace, "", now)) goto done;
    }
    const char *cv[] = {account, cid, ms, eff_dev};
    (void)exec_sql(db, "UPDATE chat.conversations_v2 SET updated_ms=$3::bigint,source_device=$4 WHERE account=$1 AND id=$2", 4, cv);
  }
  if (!exec_sql(db, "COMMIT", 0, NULL)) goto done;
  tx = false;

  char out[96];
  int n = snprintf(out, sizeof out, "{\"ok\":true,\"stop_requested\":%s}", stop_req ? "true" : "false");
  reply(q, 200, out, (size_t)n);
  goto finish;

done:
  if (tx) (void)exec_sql(db, "ROLLBACK", 0, NULL);
  refuse(q, status, why);
finish:
  PQclear(r);
  free(text_s); free(tool_args); free(diff_s);
}

static void sandbox_runner_finish(const request *q, PGconn *db, const char *key,
                                  const char *tid, const char *dispatch, const char *worker) {
  char st_s[32] = {0}, box_id[32] = {0}, tok_sid[40] = {0}, err_s[513] = {0};
  char *out_s = calloc(1, 131073);
  if (!out_s) { refuse(q, 503, "memory"); return; }

  sb_jstr(q->body, q->body_len, "status", st_s, sizeof st_s);
  sb_jstr(q->body, q->body_len, "box_id", box_id, sizeof box_id);
  sb_jstr(q->body, q->body_len, "token_session", tok_sid, sizeof tok_sid);
  sb_jstr(q->body, q->body_len, "error", err_s, sizeof err_s);
  sb_jstr(q->body, q->body_len, "output", out_s, 131072);
  long long sb_secs = sb_jint(q->body, q->body_len, "sandbox_seconds", 0);
  if (sb_secs < 0) sb_secs = 0;
  if (sb_secs > 86400) sb_secs = 86400;

  if (*tok_sid) auth_revoke_sandbox_token(db, tok_sid);

  bool tx = exec_sql(db, "BEGIN", 0, NULL);
  PGresult *r = NULL, *ar = NULL, *br = NULL;
  int status = 503;
  const char *why = "Sandbox finish unavailable.";
  if (!tx) goto done;

  const char *tv[] = {tid};
  r = PQexecParams(db,
    "SELECT t.account,t.conversation,t.state,t.sequence,coalesce(t.model,''),coalesce(t.workspace,''),"
    "coalesce(t.worker,''),coalesce(t.runner_key,''),coalesce(t.dispatch,''),coalesce(t.source_device,'cloud-sandbox'),"
    "c.revision,coalesce(c.title,''),t.output "
    "FROM chat.turns_v2 t JOIN chat.conversations_v2 c ON c.account=t.account AND c.id=t.conversation "
    "WHERE t.id=$1 FOR UPDATE OF t", 1, NULL, tv, NULL, NULL, 0);
  if (!tuples(r)) goto done;
  if (PQntuples(r) != 1) { status = 404; why = "not found"; goto done; }

  const char *account = PQgetvalue(r, 0, 0);
  const char *cid = PQgetvalue(r, 0, 1);
  int cur_st = (int)number(r, 0, 2);
  long long seq = number(r, 0, 3);
  const char *model = PQgetvalue(r, 0, 4);
  const char *workspace = PQgetvalue(r, 0, 5);
  const char *eff_dev = *box_id ? box_id : PQgetvalue(r, 0, 9);
  long long rev = number(r, 0, 10);
  const char *title = PQgetvalue(r, 0, 11);
  const char *existing_out = PQgetvalue(r, 0, 12);

  if (strcmp(PQgetvalue(r, 0, 6), worker) || strcmp(PQgetvalue(r, 0, 7), key) || strcmp(PQgetvalue(r, 0, 8), dispatch)) {
    status = 409; why = "Worker claim was replaced."; goto done;
  }

  if (!ct_terminal(cur_st)) {
    int final_st = !strcmp(st_s, "completed") ? DONE : !strcmp(st_s, "cancelled") ? CANCELLED : FAILED;
    int ev_kind = final_st == DONE ? SB_CC_COMPLETED : final_st == CANCELLED ? SB_CC_CANCELLED : SB_CC_FAILED;
    const char *stop_reason = final_st == DONE ? "end_turn" : final_st == CANCELLED ? "user_stop" : "error";
    const char *acct_ev = final_st == DONE ? "turn_completed" : final_st == CANCELLED ? "turn_cancelled" : "turn_failed";

    bool is_free = false;
    const char *av[] = {account};
    ar = PQexecParams(db, "SELECT team FROM auth.accounts WHERE id=$1", 1, NULL, av, NULL, NULL, 0);
    if (tuples(ar) && PQntuples(ar) == 1 && PQgetvalue(ar, 0, 0)[0] == 't') is_free = true;

    long long usd_per_btc = 100000;
    br = PQexec(db, "SELECT usd_per_btc FROM inference.btc_usd LIMIT 1");
    if (tuples(br) && PQntuples(br) == 1 && number(br, 0, 0) > 0) usd_per_btc = number(br, 0, 0);

    char size[16], repo[130], ref[130];
    sb_parse_workspace(workspace, size, repo, ref);
    long long cost_msat = is_free ? 0 : (long long)cp_c_turn_debit_msat(ts("sandbox"), ts(size), (int64_t)sb_secs, (int64_t)usd_per_btc);
    if (cost_msat < 0) cost_msat = 0;

    long long now = (long long)api_now_ms();
    long long next_seq = seq + 1;
    char st_n[16], seq_s[32], sec_s[32], cost_s[32], ms[32];
    num(st_n, final_st); num(seq_s, next_seq); num(sec_s, sb_secs); num(cost_s, cost_msat); num(ms, now);

    const char *final_out = (*out_s) ? out_s : existing_out;
    const char *uv[] = {account, tid, st_n, seq_s, final_out, stop_reason, err_s, sec_s, cost_s, eff_dev, ms};
    if (!exec_sql(db,
        "UPDATE chat.turns_v2 SET state=$3::int,sequence=$4::bigint,output=$5,stop_reason=$6,error=$7,"
        "sandbox_seconds=GREATEST(sandbox_seconds,$8::bigint),cost_msat=GREATEST(cost_msat,$9::bigint),"
        "source_device=$10,updated_ms=$11::bigint,heartbeat_ms=$11::bigint WHERE account=$1 AND id=$2",
        11, uv)) goto done;

    const char *ev_text = (final_st == FAILED && *err_s) ? err_s : "";
    if (!sb_insert_cc_event(db, account, cid, tid, next_seq, ev_kind, final_st,
                            ev_text, "", "", "", "", final_st == FAILED,
                            model, cost_msat, eff_dev, workspace, stop_reason, now)) goto done;

    const char *cv[] = {account, cid, ms, eff_dev};
    (void)exec_sql(db, "UPDATE chat.conversations_v2 SET updated_ms=$3::bigint,source_device=$4 WHERE account=$1 AND id=$2", 4, cv);

    if (!cloud_emit_account_event(db, account, cid, tid, acct_ev, title, "sandbox",
                                  model && *model ? model : "google/gemini-4-neon", cost_msat,
                                  false, false, eff_dev, rev, now)) goto done;
  }

  if (!exec_sql(db, "COMMIT", 0, NULL)) goto done;
  tx = false;
  reply(q, 200, "{\"ok\":true}", 11);
  goto finish;

done:
  if (tx) (void)exec_sql(db, "ROLLBACK", 0, NULL);
  refuse(q, status, why);
finish:
  PQclear(br); PQclear(ar); PQclear(r);
  free(out_s);
}

static void sandbox_runner(const request *q, PGconn *db) {
  if (strcmp(q->method, "POST")) { refuse(q, 405, "POST this"); return; }
  char key[65];
  int auth = chat_runner_who(q, db, key);
  if (auth) { refuse(q, auth, "Sign this request with a configured runner key."); return; }
  char op[24] = {0}, tid[37] = {0}, dispatch[37] = {0}, worker[81] = {0};
  sb_jstr(q->body, q->body_len, "op", op, sizeof op);
  sb_jstr(q->body, q->body_len, "turn", tid, sizeof tid);
  sb_jstr(q->body, q->body_len, "dispatch", dispatch, sizeof dispatch);
  sb_jstr(q->body, q->body_len, "worker", worker, sizeof worker);
  if (!*op || !*tid || !*dispatch || !*worker) {
    refuse(q, 400, "Invalid sandbox runner request.");
    return;
  }
  if (!strcmp(op, "begin")) sandbox_runner_begin(q, db, key, tid, dispatch, worker);
  else if (!strcmp(op, "event")) sandbox_runner_event(q, db, key, tid, dispatch, worker);
  else if (!strcmp(op, "finish")) sandbox_runner_finish(q, db, key, tid, dispatch, worker);
  else refuse(q, 400, "Unknown sandbox runner op.");
}
