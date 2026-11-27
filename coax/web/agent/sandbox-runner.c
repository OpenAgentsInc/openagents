// Cloud Sandbox durable turn driver for oa-agent:
// Executes compute="sandbox" turns end-to-end on a first-party GCP GCE VM managed by oa-boat,
// running Neon Core (`/usr/local/bin/oa-worker --json --model google/gemini-4-neon`) inside the VM
// with an ephemeral short-lived oa_at_ token (zero raw provider keys ever touch the VM) and streaming
// live thinking, tool calls, tool results, diffs, text deltas, and terminal status back to oa-api's
// /api/chat/runner/sandbox/v1 (which fans out openagents.chat.event.v2 SSE to Web, Desktop & Mobile).
#include <ctype.h>
#include <openssl/crypto.h>

static char *sb_jstr_utf8(const char *json, const char *key, const char *dflt) {
  const char *at = NULL;
  size_t n = 0;
  if (!json || !jget(json, key, &at, &n) || n < 2 || *at != '"') return strdup(dflt);
  char *s = malloc(n * 3 + 1);
  if (!s) return strdup(dflt);
  char *o = s;
  for (const char *p = at + 1; p < at + n - 1; p++) {
    if (*p != '\\') { *o++ = *p; continue; }
    if (++p >= at + n - 1) break;
    if (*p == 'n') *o++ = '\n';
    else if (*p == 't') *o++ = '\t';
    else if (*p == 'r') *o++ = '\r';
    else if (*p == 'b') *o++ = '\b';
    else if (*p == 'f') *o++ = '\f';
    else if (*p == 'u' && p + 4 < at + n - 1) {
      unsigned v = 0;
      char h[5] = {p[1], p[2], p[3], p[4], 0};
      if (sscanf(h, "%4x", &v) == 1) {
        if (v < 0x80) *o++ = (char)v;
        else if (v < 0x800) {
          *o++ = (char)(0xC0 | (v >> 6));
          *o++ = (char)(0x80 | (v & 0x3F));
        } else {
          *o++ = (char)(0xE0 | (v >> 12));
          *o++ = (char)(0x80 | ((v >> 6) & 0x3F));
          *o++ = (char)(0x80 | (v & 0x3F));
        }
      }
      p += 4;
    } else {
      *o++ = *p;
    }
  }
  *o = 0;
  return s;
}

static bool sb_boat_state_up(const char *st) {
  return st && (!strcmp(st, "ready") || !strcmp(st, "running") || !strcmp(st, "idle"));
}

static bool sb_boat_state_bad(const char *st) {
  return st && (!strcmp(st, "error") || !strcmp(st, "archived") || !strcmp(st, "archiving") || !strcmp(st, "destroyed") || !strcmp(st, "failed"));
}

static long sb_boat_req(const char *method, const char *path, const char *body, long timeout_s, text *out) {
  const char *base = env("OA_BOAT_API_BASE", "");
  if (!base[0] || !boat_key[0]) return 0;
  size_t bl = strlen(base);
  while (bl > 0 && base[bl - 1] == '/') bl--;
  char url[1024], auth[640];
  snprintf(url, sizeof url, "%.*s%s", (int)bl, base, path);
  snprintf(auth, sizeof auth, "Authorization: Bearer %s", boat_key);
  CURL *h = curl_easy_init();
  if (!h) { OPENSSL_cleanse(auth, sizeof auth); return 0; }
  struct curl_slist *hd = NULL;
  hd = curl_slist_append(hd, auth);
  if (body) hd = curl_slist_append(hd, "Content-Type: application/json");
  out->n = 0; if (out->p) out->p[0] = 0;
  curl_easy_setopt(h, CURLOPT_URL, url);
  curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
  curl_easy_setopt(h, CURLOPT_CUSTOMREQUEST, method);
  if (body) {
    curl_easy_setopt(h, CURLOPT_POSTFIELDS, body);
    curl_easy_setopt(h, CURLOPT_POSTFIELDSIZE_LARGE, (curl_off_t)strlen(body));
  }
  curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, take);
  curl_easy_setopt(h, CURLOPT_WRITEDATA, out);
  curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT, 15L);
  curl_easy_setopt(h, CURLOPT_TIMEOUT, timeout_s);
  long status = 0;
  if (curl_easy_perform(h) == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(hd);
  curl_easy_cleanup(h);
  OPENSSL_cleanse(auth, sizeof auth);
  if (!out->p) puts_(out, "");
  return status;
}

static long sb_api_event(const char *worker, const char *tid, const char *disp, const char *box_id,
                         const char *kind, const char *text_s, const char *tool_id,
                         const char *tool_name, const char *tool_args, const char *diff_s,
                         bool failed, bool *stop_req) {
  text b = {0}, out = {0};
  puts_(&b, "{\"op\":\"event\",\"turn\":"); putq(&b, tid);
  puts_(&b, ",\"dispatch\":"); putq(&b, disp);
  puts_(&b, ",\"worker\":"); putq(&b, worker);
  if (box_id && *box_id) { puts_(&b, ",\"box_id\":"); putq(&b, box_id); }
  if (kind && *kind) { puts_(&b, ",\"kind\":"); putq(&b, kind); }
  if (text_s && *text_s) { puts_(&b, ",\"text\":"); putq(&b, text_s); }
  if (tool_id && *tool_id) { puts_(&b, ",\"tool_id\":"); putq(&b, tool_id); }
  if (tool_name && *tool_name) { puts_(&b, ",\"tool_name\":"); putq(&b, tool_name); }
  if (tool_args && *tool_args) { puts_(&b, ",\"tool_args\":"); putq(&b, tool_args); }
  if (diff_s && *diff_s) { puts_(&b, ",\"diff\":"); putq(&b, diff_s); }
  if (failed) puts_(&b, ",\"failed\":true");
  puts_(&b, "}");
  long st = signed_post("/api/chat/runner/sandbox/v1", b.p, 25, &out);
  if (stop_req && st == 200 && out.p && strstr(out.p, "\"stop_requested\":true")) *stop_req = true;
  free(b.p);
  free(out.p);
  return st;
}

static void sb_api_finish(const char *worker, const char *tid, const char *disp,
                          const char *status_s, const char *output_s, const char *error_s,
                          const char *box_id, long long sb_secs, const char *tok_sid) {
  text b = {0};
  puts_(&b, "{\"op\":\"finish\",\"turn\":"); putq(&b, tid);
  puts_(&b, ",\"dispatch\":"); putq(&b, disp);
  puts_(&b, ",\"worker\":"); putq(&b, worker);
  puts_(&b, ",\"status\":"); putq(&b, status_s);
  puts_(&b, ",\"output\":"); putq(&b, output_s ? output_s : "");
  puts_(&b, ",\"error\":"); putq(&b, error_s ? error_s : "");
  puts_(&b, ",\"box_id\":"); putq(&b, box_id ? box_id : "");
  putf(&b, ",\"sandbox_seconds\":%lld", sb_secs > 0 ? sb_secs : 0);
  puts_(&b, ",\"token_session\":"); putq(&b, tok_sid ? tok_sid : "");
  puts_(&b, "}");
  for (int attempt = 0; attempt < 3; attempt++) {
    text out = {0};
    long st = signed_post("/api/chat/runner/sandbox/v1", b.p, 30, &out);
    free(out.p);
    if (st == 200 || (st >= 400 && st < 500)) break;
    sleep((unsigned)(attempt + 1));
  }
  free(b.p);
}

static void sb_put_env_line(text *t, const char *k, const char *v) {
  if (!v || !*v) return;
  puts_(t, k);
  puts_(t, "='");
  for (const char *p = v; *p; p++) {
    if (*p == '\'' || *p == '\r' || *p == '\n') continue;
    put(t, p, 1);
  }
  puts_(t, "'\n");
}

static bool sb_upload_file(const char *box_id, const char *path, const char *content) {
  char ep[128];
  snprintf(ep, sizeof ep, "/sandboxes/%s/files", box_id);
  text body = {0}, out = {0};
  puts_(&body, "{\"path\":"); putq(&body, path);
  puts_(&body, ",\"content\":"); putq(&body, content ? content : "");
  puts_(&body, "}");
  long st = sb_boat_req("PUT", ep, body.p, 30, &out);
  if (body.p) OPENSSL_cleanse(body.p, body.n);
  free(body.p);
  free(out.p);
  return st >= 200 && st < 300;
}

typedef struct {
  const char *worker;
  const char *tid;
  const char *disp;
  const char *box_id;
  long long stdout_off;
  long long stderr_off;
  text ndjson_buf;
  text worker_buf;
  text full_answer;
  text stderr_tail;
  bool saw_exit;
  bool saw_end;
  bool saw_reconnect;
  bool stop_requested;
  int exit_code;
  char end_status[24];
  char end_error[512];
  double last_hb_ms;
  double deadline_ms;
} sb_stream_ctx;

static void sb_handle_worker_line(sb_stream_ctx *ctx, const char *line) {
  const char *p = skip_ws(line);
  if (*p != '{') return;
  char *kind = sb_jstr_utf8(p, "kind", "");
  if (!strcmp(kind, "thinking")) {
    char *txt = sb_jstr_utf8(p, "text", "");
    if (*txt) {
      sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "thinking_delta",
                   txt, NULL, NULL, NULL, NULL, false, &ctx->stop_requested);
      ctx->last_hb_ms = now_ms();
    }
    free(txt);
  } else if (!strcmp(kind, "tool")) {
    char *id = sb_jstr_utf8(p, "id", "");
    char *tname = sb_jstr_utf8(p, "tool", "Tool");
    char *lbl = sb_jstr_utf8(p, "label", "");
    sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "tool_call",
                 NULL, id, tname, lbl, NULL, false, &ctx->stop_requested);
    ctx->last_hb_ms = now_ms();
    free(id); free(tname); free(lbl);
  } else if (!strcmp(kind, "result")) {
    char *id = sb_jstr_utf8(p, "id", "");
    char *txt = sb_jstr_utf8(p, "text", "");
    bool err = strstr(p, "\"error\":true") != NULL || strstr(p, "\"error\": true") != NULL;
    sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "tool_result",
                 txt, id, NULL, NULL, NULL, err, &ctx->stop_requested);
    ctx->last_hb_ms = now_ms();
    free(id); free(txt);
  } else if (!strcmp(kind, "diff")) {
    char *id = sb_jstr_utf8(p, "id", "");
    char *diff = sb_jstr_utf8(p, "diff", "");
    if (*diff) {
      sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "tool_result",
                   NULL, id, NULL, NULL, diff, false, &ctx->stop_requested);
      ctx->last_hb_ms = now_ms();
    }
    free(id); free(diff);
  } else if (!strcmp(kind, "reply")) {
    char *txt = sb_jstr_utf8(p, "text", "");
    if (*txt) {
      puts_(&ctx->full_answer, txt);
      sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "text_delta",
                   txt, NULL, NULL, NULL, NULL, false, &ctx->stop_requested);
      ctx->last_hb_ms = now_ms();
    }
    free(txt);
  } else if (!strcmp(kind, "end")) {
    char *ending = sb_jstr_utf8(p, "ending", "finished");
    char *txt = sb_jstr_utf8(p, "text", "");
    if (!strcmp(ending, "finished")) {
      if (*txt && ctx->full_answer.n == 0) {
        puts_(&ctx->full_answer, txt);
        sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "text_delta",
                     txt, NULL, NULL, NULL, NULL, false, &ctx->stop_requested);
      }
      snprintf(ctx->end_status, sizeof ctx->end_status, "completed");
    } else if (!strcmp(ending, "stopped")) {
      snprintf(ctx->end_status, sizeof ctx->end_status, "cancelled");
    } else {
      snprintf(ctx->end_status, sizeof ctx->end_status, "failed");
      if (*txt) snprintf(ctx->end_error, sizeof ctx->end_error, "%.500s", txt);
    }
    ctx->saw_end = true;
    free(ending); free(txt);
  }
  free(kind);
}

static void sb_drain_worker_lines(sb_stream_ctx *ctx, bool flush_partial) {
  char *nl;
  while (ctx->worker_buf.p && (nl = memchr(ctx->worker_buf.p, '\n', ctx->worker_buf.n))) {
    *nl = 0;
    sb_handle_worker_line(ctx, ctx->worker_buf.p);
    size_t rest = ctx->worker_buf.n - (size_t)(nl + 1 - ctx->worker_buf.p);
    memmove(ctx->worker_buf.p, nl + 1, rest);
    ctx->worker_buf.n = rest;
    ctx->worker_buf.p[rest] = 0;
  }
  if (flush_partial && ctx->worker_buf.p && ctx->worker_buf.n > 0) {
    sb_handle_worker_line(ctx, ctx->worker_buf.p);
    ctx->worker_buf.n = 0;
    ctx->worker_buf.p[0] = 0;
  }
}

static void sb_handle_boat_frame(sb_stream_ctx *ctx, const char *frame) {
  const char *p = skip_ws(frame);
  if (*p != '{') return;
  char *type = sb_jstr_utf8(p, "type", "");
  if (!strcmp(type, "stdout")) {
    char *data = sb_jstr_utf8(p, "data", "");
    long long off = jint(p, "nextOffset");
    if (off > ctx->stdout_off) ctx->stdout_off = off;
    if (*data) {
      puts_(&ctx->worker_buf, data);
      sb_drain_worker_lines(ctx, false);
    }
    free(data);
  } else if (!strcmp(type, "stderr")) {
    char *data = sb_jstr_utf8(p, "data", "");
    long long off = jint(p, "nextOffset");
    if (off > ctx->stderr_off) ctx->stderr_off = off;
    if (*data) {
      puts_(&ctx->stderr_tail, data);
      if (ctx->stderr_tail.n > 4096) {
        size_t keep = 2048;
        memmove(ctx->stderr_tail.p, ctx->stderr_tail.p + (ctx->stderr_tail.n - keep), keep);
        ctx->stderr_tail.n = keep;
        ctx->stderr_tail.p[keep] = 0;
      }
    }
    free(data);
  } else if (!strcmp(type, "reconnect")) {
    long long oo = jint(p, "stdoutOffset");
    long long eo = jint(p, "stderrOffset");
    if (oo > ctx->stdout_off) ctx->stdout_off = oo;
    if (eo > ctx->stderr_off) ctx->stderr_off = eo;
    ctx->saw_reconnect = true;
  } else if (!strcmp(type, "exit")) {
    ctx->saw_exit = true;
    ctx->exit_code = (int)jint(p, "exitCode");
  }
  free(type);
}

static size_t sb_stream_write_cb(char *ptr, size_t size, size_t nmemb, void *userdata) {
  sb_stream_ctx *ctx = userdata;
  size_t total = size * nmemb;
  put(&ctx->ndjson_buf, ptr, total);
  char *nl;
  while (ctx->ndjson_buf.p && (nl = memchr(ctx->ndjson_buf.p, '\n', ctx->ndjson_buf.n))) {
    *nl = 0;
    sb_handle_boat_frame(ctx, ctx->ndjson_buf.p);
    size_t rest = ctx->ndjson_buf.n - (size_t)(nl + 1 - ctx->ndjson_buf.p);
    memmove(ctx->ndjson_buf.p, nl + 1, rest);
    ctx->ndjson_buf.n = rest;
    ctx->ndjson_buf.p[rest] = 0;
  }
  if (ctx->stop_requested || now_ms() >= ctx->deadline_ms) return 0;
  return total;
}

static int sb_stream_progress_cb(void *userdata, curl_off_t dlt, curl_off_t dln, curl_off_t ult, curl_off_t uln) {
  (void)dlt; (void)dln; (void)ult; (void)uln;
  sb_stream_ctx *ctx = userdata;
  if (ctx->stop_requested || now_ms() >= ctx->deadline_ms) return 1;
  if (now_ms() - ctx->last_hb_ms >= 4000.0) {
    ctx->last_hb_ms = now_ms();
    sb_api_event(ctx->worker, ctx->tid, ctx->disp, ctx->box_id, "",
                 NULL, NULL, NULL, NULL, NULL, false, &ctx->stop_requested);
    if (ctx->stop_requested) return 1;
  }
  return 0;
}

static void sb_parse_boat_sandbox(const char *resp, char id_out[40], char state_out[32]) {
  id_out[0] = 0;
  state_out[0] = 0;
  if (!resp || !*resp) return;
  char *inner = jraw(resp, "sandbox", "");
  const char *src = (inner && inner[0] == '{') ? inner : resp;
  char *id = sb_jstr_utf8(src, "id", "");
  char *st = sb_jstr_utf8(src, "state", "");
  snprintf(id_out, 40, "%.39s", id);
  snprintf(state_out, 32, "%.31s", st);
  free(id); free(st); free(inner);
}

// Attempts to run a claimed durable turn as a Cloud Sandbox (compute == "sandbox").
// Returns true if this turn was a sandbox turn (handled here, including errors/cancellations),
// or false if it is a normal hosted/native durable turn that should proceed via op=2.
static bool sandbox_try_run(const char *worker, coax_string turn_cs, coax_string disp_cs) {
  char tid[40] = {0}, disp[40] = {0};
  if (turn_cs.count <= 0 || turn_cs.count >= (int64_t)sizeof tid || disp_cs.count <= 0 || disp_cs.count >= (int64_t)sizeof disp) return false;
  memcpy(tid, turn_cs.data, (size_t)turn_cs.count);
  memcpy(disp, disp_cs.data, (size_t)disp_cs.count);

  text req = {0}, out = {0};
  puts_(&req, "{\"op\":\"begin\",\"turn\":"); putq(&req, tid);
  puts_(&req, ",\"dispatch\":"); putq(&req, disp);
  puts_(&req, ",\"worker\":"); putq(&req, worker);
  puts_(&req, "}");
  long st = signed_post("/api/chat/runner/sandbox/v1", req.p, 30, &out);
  free(req.p);
  if (st == 409) { free(out.p); return true; }
  if (st != 200 || !out.p) {
    say("sandbox begin failed HTTP %ld: %.160s", st, out.p ? out.p : "");
    free(out.p);
    return false;
  }
  if (strstr(out.p, "\"sandbox\":false") || strstr(out.p, "\"sandbox\": false")) {
    free(out.p);
    return false;
  }
  if (strstr(out.p, "\"already_done\":true") || strstr(out.p, "\"already_done\": true")) {
    free(out.p);
    return true;
  }

  char *size = sb_jstr_utf8(out.p, "size", "small");
  char *model = sb_jstr_utf8(out.p, "model", "google/gemini-4-neon");
  char *warm_box = sb_jstr_utf8(out.p, "box_id", "");
  char *repo = sb_jstr_utf8(out.p, "repo", "");
  char *ref = sb_jstr_utf8(out.p, "ref", "");
  char *prompt = sb_jstr_utf8(out.p, "prompt", "");
  char *cloud_origin = sb_jstr_utf8(out.p, "cloud_origin", env("OA_API_ORIGIN", "https://openagents.com"));
  char *cloud_token = sb_jstr_utf8(out.p, "cloud_token", "");
  char *tok_sid = sb_jstr_utf8(out.p, "token_session", "");
  char *gh_token = sb_jstr_utf8(out.p, "gh_token", "");
  OPENSSL_cleanse(out.p, out.n);
  free(out.p);

  double t_start = now_ms();
  char active_box[40] = {0};
  bool reused_warm = false;
  bool stop_req = false;
  sb_stream_ctx ctx = {0};
  ctx.worker = worker;
  ctx.tid = tid;
  ctx.disp = disp;
  ctx.last_hb_ms = t_start;
  ctx.deadline_ms = t_start + 12.0 * 60.0 * 1000.0;

  const char *boat_base = env("OA_BOAT_API_BASE", "");
  if (!boat_base[0] || !boat_key[0]) {
    say("sandbox turn %s: OA_BOAT_API_BASE or OA_BOAT_API_KEY missing", tid);
    sb_api_finish(worker, tid, disp, "failed", "", "Cloud sandbox service is not configured on this runner.", "", 0, tok_sid);
    goto cleanup;
  }

  // 1. Reuse warm sandbox from earlier turn in this conversation if still ready/up.
  if (warm_box[0]) {
    char gp[96]; snprintf(gp, sizeof gp, "/sandboxes/%s", warm_box);
    text bo = {0};
    long bs = sb_boat_req("GET", gp, NULL, 15, &bo);
    char bid[40], bst[32];
    sb_parse_boat_sandbox(bo.p, bid, bst);
    free(bo.p);
    if (bs == 200 && bid[0] && sb_boat_state_up(bst)) {
      snprintf(active_box, sizeof active_box, "%s", bid);
      reused_warm = true;
      char note[128];
      snprintf(note, sizeof note, "Reusing Google Cloud computer (%s)…\n", active_box);
      sb_api_event(worker, tid, disp, active_box, "thinking_delta", note, NULL, NULL, NULL, NULL, false, &stop_req);
    }
  }

  // 2. Otherwise provision a fresh GCE VM via oa-boat and wait for SSH readiness.
  if (!active_box[0] && !stop_req) {
    sb_api_event(worker, tid, disp, "cloud-sandbox", "thinking_delta",
                 "Provisioning Google Cloud computer (Debian 12 x86_64)…\n",
                 NULL, NULL, NULL, NULL, false, &stop_req);
    if (stop_req) {
      sb_api_finish(worker, tid, disp, "cancelled", "", "", "", 1, tok_sid);
      goto cleanup;
    }
    char short_id[12];
    snprintf(short_id, sizeof short_id, "%.8s", tid + (strncmp(tid, "trn_", 4) == 0 ? 4 : 0));
    text cbody = {0}, cout = {0};
    putf(&cbody, "{\"name\":\"oa-sb-%s\",\"type\":", short_id);
    putq(&cbody, size[0] ? size : "small");
    puts_(&cbody, ",\"ttlSeconds\":3600}");
    long cs_code = sb_boat_req("POST", "/sandboxes", cbody.p, 35, &cout);
    char bid[40], bst[32];
    sb_parse_boat_sandbox(cout.p, bid, bst);
    free(cbody.p);
    if (cs_code < 200 || cs_code >= 300 || !bid[0]) {
      say("sandbox turn %s: boat create failed HTTP %ld: %.200s", tid, cs_code, cout.p ? cout.p : "");
      free(cout.p);
      sb_api_finish(worker, tid, disp, "failed", "", "Could not provision Google Cloud sandbox VM.", "", 1, tok_sid);
      goto cleanup;
    }
    free(cout.p);
    snprintf(active_box, sizeof active_box, "%s", bid);
    {
      char note[160];
      snprintf(note, sizeof note, "Booting GCE VM %s (%s); waiting for SSH…\n", active_box, size[0] ? size : "small");
      sb_api_event(worker, tid, disp, active_box, "thinking_delta", note, NULL, NULL, NULL, NULL, false, &stop_req);
    }
    double wait_end = now_ms() + 180000.0;
    double last_hb = now_ms();
    bool ready = sb_boat_state_up(bst);
    while (!ready && !stop_req && now_ms() < wait_end) {
      sleep(2);
      char gp[96]; snprintf(gp, sizeof gp, "/sandboxes/%s", active_box);
      text po = {0};
      long ps = sb_boat_req("GET", gp, NULL, 15, &po);
      sb_parse_boat_sandbox(po.p, bid, bst);
      free(po.p);
      if (ps == 200 && sb_boat_state_up(bst)) { ready = true; break; }
      if (ps == 200 && sb_boat_state_bad(bst)) {
        say("sandbox turn %s: box %s entered state %s", tid, active_box, bst);
        sb_api_finish(worker, tid, disp, "failed", "", "Google Cloud sandbox VM failed to boot.", active_box,
                      (long long)((now_ms() - t_start) / 1000.0), tok_sid);
        goto cleanup;
      }
      if (now_ms() - last_hb >= 5000.0) {
        last_hb = now_ms();
        sb_api_event(worker, tid, disp, active_box, "", NULL, NULL, NULL, NULL, NULL, false, &stop_req);
      }
    }
    if (stop_req) {
      sb_api_finish(worker, tid, disp, "cancelled", "", "", active_box, (long long)((now_ms() - t_start) / 1000.0), tok_sid);
      goto cleanup;
    }
    if (!ready) {
      sb_api_finish(worker, tid, disp, "failed", "", "Timed out waiting for Google Cloud sandbox VM to boot.", active_box,
                    (long long)((now_ms() - t_start) / 1000.0), tok_sid);
      goto cleanup;
    }
  }

  ctx.box_id = active_box;

  // 3. Stage prompt file + 0600 ephemeral env file (unlinked before oa-worker executes).
  {
    char ppath[128], epath[128];
    snprintf(ppath, sizeof ppath, "/tmp/oa-prompt-%s.txt", tid);
    snprintf(epath, sizeof epath, "/tmp/oa-run-%s.env", tid);
    text env_txt = {0};
    sb_put_env_line(&env_txt, "OA_CLOUD_ORIGIN", cloud_origin);
    sb_put_env_line(&env_txt, "OA_CLOUD_TOKEN", cloud_token);
    sb_put_env_line(&env_txt, "OA_AGENT_BYPASS_PERMISSIONS", "1");
    sb_put_env_line(&env_txt, "OA_MODEL", model);
    if (gh_token[0]) sb_put_env_line(&env_txt, "GH_TOKEN", gh_token);

    bool ok_p = sb_upload_file(active_box, ppath, prompt);
    bool ok_e = ok_p && sb_upload_file(active_box, epath, env_txt.p ? env_txt.p : "");
    if (env_txt.p) OPENSSL_cleanse(env_txt.p, env_txt.n);
    free(env_txt.p);
    OPENSSL_cleanse(cloud_token, strlen(cloud_token));
    OPENSSL_cleanse(gh_token, strlen(gh_token));

    if (!ok_p || !ok_e) {
      say("sandbox turn %s: file upload failed on %s", tid, active_box);
      sb_api_finish(worker, tid, disp, "failed", "", "Could not stage turn inputs on Google Cloud sandbox.", active_box,
                    (long long)((now_ms() - t_start) / 1000.0), tok_sid);
      goto cleanup;
    }
  }

  // 4. Compose detached worker command via #authority sb_worker_command and launch on the VM.
  char cmd_buf[2048] = {0};
  int64_t cmd_n = sb_worker_command(
      cs(tid), cs(model),
      cs(reused_warm ? "" : repo), cs(reused_warm ? "" : ref),
      true, (coax_v_u8){(uint8_t *)cmd_buf, (int64_t)(sizeof cmd_buf - 1)});
  if (cmd_n <= 0) {
    sb_api_finish(worker, tid, disp, "failed", "", "Sandbox command policy refused turn parameters.", active_box,
                  (long long)((now_ms() - t_start) / 1000.0), tok_sid);
    goto cleanup;
  }
  cmd_buf[cmd_n] = 0;

  long long proc_id = 0;
  {
    char cep[128]; snprintf(cep, sizeof cep, "/sandboxes/%s/commands", active_box);
    text cbody = {0}, cout = {0};
    puts_(&cbody, "{\"command\":"); putq(&cbody, cmd_buf);
    puts_(&cbody, ",\"cwd\":\"/workspace\",\"detached\":true}");
    long cst = sb_boat_req("POST", cep, cbody.p, 30, &cout);
    proc_id = (cst >= 200 && cst < 300) ? jint(cout.p, "processId") : 0;
    if (proc_id <= 0) {
      say("sandbox turn %s: detached command launch failed HTTP %ld: %.200s", tid, cst, cout.p ? cout.p : "");
      free(cbody.p); free(cout.p);
      sb_api_finish(worker, tid, disp, "failed", "", "Could not launch Neon Core agent in Google Cloud sandbox.", active_box,
                    (long long)((now_ms() - t_start) / 1000.0), tok_sid);
      goto cleanup;
    }
    free(cbody.p); free(cout.p);
  }

  {
    char note[160];
    snprintf(note, sizeof note, "Running Neon Core (%s) on %s…\n", model[0] ? model : "google/gemini-4-neon", active_box);
    sb_api_event(worker, tid, disp, active_box, "thinking_delta", note, NULL, NULL, NULL, NULL, false, &ctx.stop_requested);
  }

  // 5. Stream NDJSON from oa-boat with seamless reconnect + heartbeat/cancel checks.
  int errs = 0;
  while (!ctx.saw_exit && !ctx.saw_end && !ctx.stop_requested && now_ms() < ctx.deadline_ms) {
    ctx.saw_reconnect = false;
    size_t bl = strlen(boat_base);
    while (bl > 0 && boat_base[bl - 1] == '/') bl--;
    char url[1024], auth[640];
    snprintf(url, sizeof url, "%.*s/sandboxes/%s/commands/%lld/stream?stdoutOffset=%lld&stderrOffset=%lld",
             (int)bl, boat_base, active_box, proc_id, ctx.stdout_off, ctx.stderr_off);
    snprintf(auth, sizeof auth, "Authorization: Bearer %s", boat_key);
    CURL *h = curl_easy_init();
    struct curl_slist *hd = curl_slist_append(NULL, auth);
    curl_easy_setopt(h, CURLOPT_URL, url);
    curl_easy_setopt(h, CURLOPT_HTTPHEADER, hd);
    curl_easy_setopt(h, CURLOPT_WRITEFUNCTION, sb_stream_write_cb);
    curl_easy_setopt(h, CURLOPT_WRITEDATA, &ctx);
    curl_easy_setopt(h, CURLOPT_NOPROGRESS, 0L);
    curl_easy_setopt(h, CURLOPT_XFERINFOFUNCTION, sb_stream_progress_cb);
    curl_easy_setopt(h, CURLOPT_XFERINFODATA, &ctx);
    curl_easy_setopt(h, CURLOPT_CONNECTTIMEOUT, 15L);
    curl_easy_setopt(h, CURLOPT_TIMEOUT, 150L);
    CURLcode rc = curl_easy_perform(h);
    long http_st = 0;
    if (rc == CURLE_OK) curl_easy_getinfo(h, CURLINFO_RESPONSE_CODE, &http_st);
    curl_slist_free_all(hd);
    curl_easy_cleanup(h);
    OPENSSL_cleanse(auth, sizeof auth);

    if (ctx.stop_requested) {
      char kep[128]; snprintf(kep, sizeof kep, "/sandboxes/%s/commands/%lld", active_box, proc_id);
      text ko = {0};
      (void)sb_boat_req("DELETE", kep, NULL, 15, &ko);
      free(ko.p);
      break;
    }
    if (ctx.saw_exit || ctx.saw_end) break;
    if (ctx.saw_reconnect) { errs = 0; continue; }

    // Check process status before retrying a dropped stream.
    char sep[128]; snprintf(sep, sizeof sep, "/sandboxes/%s/commands/%lld", active_box, proc_id);
    text so = {0};
    long sst = sb_boat_req("GET", sep, NULL, 15, &so);
    if (sst == 200 && so.p && strstr(so.p, "\"running\":false")) {
      ctx.saw_exit = true;
      ctx.exit_code = (int)jint(so.p, "exitCode");
      free(so.p);
      break;
    }
    free(so.p);
    if (++errs >= 5) break;
    sleep(1);
  }

  sb_drain_worker_lines(&ctx, true);

  const char *final_st = "failed";
  const char *final_err = "";
  if (ctx.stop_requested) {
    final_st = "cancelled";
  } else if (ctx.saw_end) {
    final_st = ctx.end_status[0] ? ctx.end_status : "completed";
    final_err = ctx.end_error;
  } else if (ctx.saw_exit && ctx.exit_code == 0) {
    final_st = "completed";
  } else if (now_ms() >= ctx.deadline_ms) {
    final_st = "failed";
    final_err = "Cloud sandbox turn timed out after 12 minutes.";
  } else {
    final_st = "failed";
    final_err = ctx.end_error[0] ? ctx.end_error : (ctx.stderr_tail.p && ctx.stderr_tail.n ? ctx.stderr_tail.p : "Neon Core agent exited unexpectedly in cloud sandbox.");
  }

  long long secs = (long long)((now_ms() - t_start) / 1000.0);
  if (secs < 1) secs = 1;
  say("sandbox turn %s %s box=%s secs=%lld answer=%zu", tid, final_st, active_box, secs, ctx.full_answer.n);
  sb_api_finish(worker, tid, disp, final_st, ctx.full_answer.p ? ctx.full_answer.p : "", final_err, active_box, secs, tok_sid);

cleanup:
  if (cloud_token) OPENSSL_cleanse(cloud_token, strlen(cloud_token));
  if (gh_token) OPENSSL_cleanse(gh_token, strlen(gh_token));
  free(size); free(model); free(warm_box); free(repo); free(ref);
  free(prompt); free(cloud_origin); free(cloud_token); free(tok_sid); free(gh_token);
  free(ctx.ndjson_buf.p); free(ctx.worker_buf.p); free(ctx.full_answer.p); free(ctx.stderr_tail.p);
  return true;
}
