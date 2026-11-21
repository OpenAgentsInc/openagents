/* Split from metaverse/voice-host/host.c (#13848): job completions and inbound WebSocket frame dispatch. */
// ---------------------------------------------------------------- job results

// The words of text, lowercased letters and digits, into w (at most n words
// of 31 bytes). Returns how many.
static int words_of(const char *text, char w[][32], int n) {
  int k = 0, len = 0;
  for (const unsigned char *c = (const unsigned char *)text;; c++) {
    bool in = *c && (isalnum(*c) || *c >= 0x80 || *c == '\'');
    if (in && *c != '\'') { if (len < 31) w[k][len++] = (char)tolower(*c); }
    else if (!in && len) { w[k][len] = 0; len = 0; if (++k == n) return k; }
    if (!*c) return k;
  }
}

// A turn heard over the reply is its echo when most of its words are the
// reply's: the phone's mic caught the speaker and barged in on itself.
// Only real recognition: the offline provider's stand-in transcript ("I
// heard N milliseconds of speech") is what its echo agent says back.
// A barge-in with no words at all (the mic caught the reply, or a noise) is
// never someone talking either.
static bool is_echo(const session *s, const char *text) {
  if (!echo_aware() || !s->cut_at || now_ms() - s->cut_at > 10000) return false;
  if (!text[0]) return true;
  static char t[64][32], r[256][32];
  int nt = words_of(text, t, 64), nr = words_of(s->cut_text, r, 256), hit = 0;
  if (!nt) return false;
  // The last word is often cut off where the barge-in cut the reply ("No
  // form" of "No foreman"): its first three letters are enough.
  for (int i = 0; i < nt; i++)
    for (int j = 0; j < nr; j++)
      if (!strcmp(t[i], r[j]) || (i == nt - 1 && strlen(t[i]) >= 3 && !strncmp(t[i], r[j], 3))) { hit++; break; }
  return hit * 4 >= nt * 3;
}

static void on_reply_sentence(session *s, const job *e);

static utter *quiet_of(session *s) {
  for (utter *u = s->speech; u; u = u->next)
    if (u->quiet) return u;
  return NULL;
}

// The barge-in was someone talking: the quiet reply stops (what the phone
// has buffered, under LEAD_MS, plays out quietly).
static void stop_quiet(session *s, const char *turn) {
  utter **q = &s->speech;
  while (*q && !(*q)->quiet) q = &(*q)->next;
  utter *u = *q;
  if (!u) return;
  if (u == s->speech && s->speaking) {
    oa_voice_msg c = {.kind = OA_MSG_CONTROL};
    c.u.control.kind = OA_CONTROL_SPEECH_END;
    c.u.control.turn = B(u->turn);
    c.u.control.utterance = B(u->id);
    c.u.control.media_epoch = oa_voice_epoch(s->v);
    c.u.control.played_samples = u->sent;
    send_msg(s, &c);
    s->speaking = false;
    oa_voice_playing(s->v, false);
  }
  *q = u->next;
  logf_("%s: %s is someone talking; the quiet reply %s stops at %zu of %zu samples", s->sid, turn, u->id, u->sent, u->n);
  utter_free(u);
}

// The cut reply again, for this turn, from the sample the phone had played
// up to: its rendered audio from there, then the sentences not rendered yet.
// Three echoes in a row and the reply stops. Quiet: right at the barge-in,
// before any words are known, at QUIET_RESUME gain and keeping the cut for
// is_echo; once they say echo it goes on at full gain with no gap.
static void resume_cut(session *s, uint32_t turn_no, const char *turn, bool quiet) {
  utter *going = quiet ? NULL : quiet_of(s);
  if (going) {
    going->quiet = false;
    logf_("%s: %s is the reply's echo; %s goes on at full volume", s->sid, turn, going->id);
    clear_cut(s);
    return;
  }
  if (++s->cut_resumes > 3 || !s->cut_pcm) { clear_cut(s); send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0); return; }
  size_t from = s->cut_played < s->cut_n ? s->cut_played : s->cut_n;
  // The sentences of the reply, and where the unrendered ones start.
  const char *t = s->cut_text;
  size_t len = strlen(t), rest_at = s->cut_rest < len ? s->cut_rest : len;
  while (t[rest_at] == ' ') rest_at++;
  utter *u = calloc(1, sizeof *u);
  size_t n = s->cut_n - from;
  if (!u || (n && !(u->pcm = malloc(n * 2)))) { free(u); clear_cut(s); send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0); return; }
  if (n) memcpy(u->pcm, s->cut_pcm + from, n * 2);
  u->n = u->cap = n;
  u->no = ++s->utt_no;
  snprintf(u->id, sizeof u->id, "u%u", u->no);
  snprintf(u->turn, sizeof u->turn, "%s", turn);
  u->turn_no = turn_no;
  u->t_end = u->t_final = u->t_sentence = u->t_audio = now_ms();
  u->plan = "resume";
  // The caption: the words from about where it was cut.
  size_t c = s->cut_n ? (size_t)((double)rest_at * (double)from / (double)s->cut_n) : 0;
  while (c > 0 && t[c - 1] != ' ') c--;
  snprintf(u->text, sizeof u->text, "%s", t + c);
  u->base = rest_at - c;
  for (const char *a = t + rest_at; *a && u->nseg < MAX_SEGS - 1;) {
    const char *b = a;
    while (*b && !(strchr(".?!", *b) && (b[1] == ' ' || !b[1]))) b++;
    if (*b) b++;
    char sentence[MAX_TEXT + 1];
    snprintf(sentence, sizeof sentence, "%.*s", (int)(b - a), a);
    u->seg[u->nseg].end = (size_t)(b - t) - c;
    synthesize(s, u, u->nseg, sentence);
    a = b;
    while (*a == ' ') a++;
  }
  u->text_done = true;
  u->quiet = quiet;
  logf_("%s: %s %s at %zu of %zu samples (%d sentence(s) to render): %s", s->sid, turn,
        quiet ? "barged in; the reply goes on quietly" : "is the reply's echo; resuming", from, s->cut_n, u->nseg, u->text);
  if (!quiet) { free(s->cut_pcm); s->cut_pcm = NULL; s->cut_at = 0; }
  utter **q = &s->speech;
  while (*q) q = &(*q)->next;
  *q = u;
  pump_speech(s, now_ms());
}

static void deliver_final(session *s, uint32_t turn_no, const char *turn, const char *text) {
  s->t_final = now_ms();
  if (turn_no == s->echo_turn) {
    logf_("%s: %s was the reply's echo, already resumed: %s", s->sid, turn, text);
    return;
  }
  if (take_quiet(s, turn_no)) {
    logf_("%s: %s heard over the reply without a barge-in, so not taken: %s", s->sid, turn, text);
    send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  if (is_echo(s, text)) {
    logf_("%s: %s heard %s", s->sid, turn, text);
    resume_cut(s, turn_no, turn, false);
    return;
  }
  stop_quiet(s, turn);
  clear_cut(s);
  if (g_verbose) logf_("%s: %s final (%s) in %llu ms: %s", s->sid, turn, s->plan, (unsigned long long)(s->t_final - s->t_end), text);
  else logf_("%s: %s final (%s) in %llu ms (%zu bytes)", s->sid, turn, s->plan, (unsigned long long)(s->t_final - s->t_end), strlen(text));
  send_turn_event(s, turn, OA_TURN_PHASE_FINAL, text, 0);
  send_meter(s);
  // The transcript goes where the floor binds this speaker; never chosen from the words.
  if (!text[0] || oa_voice_floor_route(s->fl, 0) != TARGET_ROUTE) {
    send_turn_event(s, turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  // Facing a character: the page answers (on_page_speech), not the route's agent.
  if (s->facing[0]) {
    stop_agent(s);
    s->page_turn_no = turn_no;
    snprintf(s->page_turn, sizeof s->page_turn, "%s", turn);
    return;
  }
  job *j = calloc(1, sizeof *j);
  if (!j) { send_error(s, OA_ERROR_INTERNAL, "out of memory", turn); return; }
  j->kind = JOB_AGENT;
  j->session = s->serial;
  j->turn_no = turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", turn);
  snprintf(j->text, sizeof j->text, "%s", text);
  memcpy(j->hist, s->hist, sizeof s->hist);
  j->nhist = s->nhist;
  snprintf(j->route, sizeof j->route, "%s", s->route);
  stop_agent(s);
  s->agent = j;
  submit(j);
}

static void on_job_done(job *j) {
  if (j->kind == JOB_WARM) { job_free(j); return; }
  session *s = session_by_serial(j->session);
  if (!s) { job_free(j); return; }
  if (j->kind == JOB_RECOGNIZE) {
    if (g_verbose) logf_("%s: %s recognized %u ms (cut in %u ms of silence) in %llu ms: %s", s->sid, j->turn, ms_of(j->cover),
                         j->cut_silence_ms, (unsigned long long)(now_ms() - j->started), j->ok ? j->text : j->err);
    if (j->turn_no == s->turn_no) {
      for (int i = 0; i < s->nflight; i++)
        if (s->flight[i] == j->cover) { s->flight[i] = s->flight[--s->nflight]; break; }
      if (j->ok && j->cover > s->done_cover) {
        s->done_cover = j->cover;
        s->done_cut_ms = j->cut_silence_ms;
        snprintf(s->done_text, sizeof s->done_text, "%s", j->text);
      }
      // Two or more of the reply's own words while the turn is still open:
      // the echo, so the reply goes on now rather than after the turn ends.
      // Three or more and few of them the reply's: someone is talking.
      static char w[64][32];
      int nw = j->ok ? words_of(j->text, w, 64) : 0;
      bool echo = nw >= 2 && is_echo(s, j->text);
      if (j->ok && s->turn_open && s->echo_turn != j->turn_no && echo) {
        logf_("%s: %s heard %s so far", s->sid, j->turn, j->text);
        s->echo_turn = j->turn_no;
        resume_cut(s, j->turn_no, j->turn, false);
      } else if (j->ok && s->turn_open && j->text[0]) {
        if (nw >= 3 && !echo) stop_quiet(s, j->turn);
        send_turn_event(s, j->turn, OA_TURN_PHASE_PARTIAL, j->text, 0);
      }
      if (s->final_pending) plan_final(s);
      else if (s->turn_open) maybe_recognize(s);
    }
    job_free(j);
    return;
  }
  if (j->kind == JOB_PARAKEET) {
    for (int i = 0; i < s->nasr; i++)
      if (s->asr[i] == j) { s->asr[i] = s->asr[--s->nasr]; break; }
    int64_t v = metaverse_voice_asr_admit(j->turn_no, s->dropped_upto, j->epoch, oa_voice_epoch(s->v), j->cancel != 0,
                                      j->windows, j->ok, j->bytes);
    static const char *verdicts[] = {"final", "no speech", "stale", "cancelled", "failed", "too long"};
    logf_("%s: %s parakeet %s: %d of %d window(s), %zu samples, inference %.0f ms, recognition %llu ms after TURN_END",
          s->sid, j->turn, v >= 0 && v < 6 ? verdicts[v] : "?", j->windows_done, j->windows, j->n, j->infer_ms,
          (unsigned long long)(now_ms() - s->t_end));
    if (v == 0) deliver_final(s, j->turn_no, j->turn, j->text);
    else if (v == 1) deliver_final(s, j->turn_no, j->turn, "");
    else if (v == 4) {
      send_error(s, OA_ERROR_PROVIDER, j->err[0] ? j->err : "parakeet failed", j->turn);
      send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    } else if (v == 5) {
      send_error(s, OA_ERROR_TOO_LONG, "the transcript is longer than 600 bytes", j->turn);
      send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    }
    job_free(j);
    return;
  }
  if (j->turn_no <= s->dropped_upto) {  // barged or cancelled
    if (s->agent == j) s->agent = NULL;
    job_free(j);
    return;
  }
  switch (j->kind) {
  case JOB_FINAL:
  case JOB_TAIL:
    if (!j->ok) {
      send_error(s, OA_ERROR_PROVIDER, j->err, j->turn);
      send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0);
      break;
    }
    if (j->kind == JOB_TAIL && j->prefix[0]) {
      char text[2 * MAX_TEXT + 2];
      snprintf(text, sizeof text, "%s%s%s", j->prefix, j->text[0] ? " " : "", j->text);
      clip_utf8(text, MAX_TEXT);
      deliver_final(s, j->turn_no, j->turn, text);
    } else {
      deliver_final(s, j->turn_no, j->turn, j->text);
    }
    break;
  case JOB_SENTENCE:
    on_reply_sentence(s, j);
    break;
  case JOB_AGENT: {
    if (!j->ok && !j->reply[0] && !j->cancel && j->tries == 0 && s->agent == j) {
      // A dropped connection before any reply: once more, before saying so.
      job *again = malloc(sizeof *again);
      if (again) {
        *again = *j;
        again->tries = 1; again->ok = false; again->err[0] = 0; again->next = NULL;
        logf_("%s: %s agent failed (%s); trying again", s->sid, j->turn, j->err);
        s->agent = again;
        submit(again);
        break;
      }
    }
    if (s->agent == j) s->agent = NULL;
    utter *u = utter_by_turn(s, j->turn_no);
    logf_("%s: %s agent reply in %llu ms (%zu bytes)", s->sid, j->turn, (unsigned long long)(now_ms() - j->started), strlen(j->reply));
    if (g_verbose && j->ok) logf_("%s: reply: %s", s->sid, j->reply);
    if (!j->ok) send_error(s, OA_ERROR_PROVIDER, j->err, j->turn);
    else {
      if (s->nhist == HISTORY) { memmove(s->hist, s->hist + 1, sizeof s->hist[0] * (HISTORY - 1)); s->nhist--; }
      snprintf(s->hist[s->nhist].user, sizeof s->hist[0].user, "%s", j->text);
      snprintf(s->hist[s->nhist].agent, sizeof s->hist[0].agent, "%s", j->reply);
      s->nhist++;
    }
    send_meter(s);
    if (!u) { send_turn_event(s, j->turn, OA_TURN_PHASE_ENDED, NULL, 0); break; }
    if (!u->text_done && u->overflow[0]) { u->seg[u->nseg].end = strlen(u->text); synthesize(s, u, u->nseg, u->overflow); }
    u->text_done = true;
    pump_speech(s, now_ms());
    break;
  }
  case JOB_TTS:
    if (j->ok) logf_("%s: %s sentence %u: %zu samples in %llu ms%s", s->sid, j->turn, j->seg, j->n,
                     (unsigned long long)(now_ms() - j->started), j->cached ? " (cached)" : "");
    on_reply_audio(s, j);
    break;
  default: break;
  }
  job_free(j);
}

// ---------------------------------------------------------------- frames in

static void on_hello(session *s, const oa_voice_hello *h) {
  if (h->protocol != OA_VOICE_PROTOCOL) { send_error(s, OA_ERROR_PROTOCOL, "unsupported protocol", ""); s->state = S_CLOSING; return; }
  uint32_t in = h->input_sample_rate ? h->input_sample_rate : OA_VOICE_RATE;
  uint32_t out = h->output_sample_rate ? h->output_sample_rate : 24000;
  if (in != 16000 && in != 24000 && in != 44100 && in != 48000) { send_error(s, OA_ERROR_PROTOCOL, "unsupported input_sample_rate", ""); s->state = S_CLOSING; return; }
  if (out != 16000 && out != 24000 && out != 48000) { send_error(s, OA_ERROR_PROTOCOL, "unsupported output_sample_rate", ""); s->state = S_CLOSING; return; }
  if (h->codec != OA_CODEC_UNSPECIFIED && h->codec != OA_CODEC_PCM_S16LE) { send_error(s, OA_ERROR_PROTOCOL, "only PCM s16le is supported", ""); s->state = S_CLOSING; return; }
  char profile[65];
  copy_bytes(profile, sizeof profile, h->profile);
  s->prof = speech_find_profile(profile);
  if (!s->prof) { send_error(s, OA_ERROR_PROTOCOL, "unknown voice profile", ""); s->state = S_CLOSING; return; }
  copy_bytes(s->route, sizeof s->route, h->route);
  if (!s->route[0]) snprintf(s->route, sizeof s->route, "foreman");
  if (!speech_route_known(s->route)) { send_error(s, OA_ERROR_PROTOCOL, "unknown route", ""); s->state = S_CLOSING; return; }
  if (h->has_speaker) copy_bytes(s->speaker, sizeof s->speaker, h->speaker.id);
  if (!s->speaker[0]) snprintf(s->speaker, sizeof s->speaker, "dev");
  // Dev host: Hello.token is not checked yet (never logged either).
  s->acct = account_for(s->speaker);
  if (!s->acct) { send_error(s, OA_ERROR_LIMIT, "voice is busy: too many speakers today", ""); s->state = S_CLOSING; return; }
  s->v = calloc(1, oa_voice_size());
  s->fl = calloc(1, oa_voice_floor_size());
  if (!s->acct || !s->v || !s->fl) { send_error(s, OA_ERROR_INTERNAL, "out of memory", ""); s->state = S_CLOSING; return; }
  s->in_rate = in;
  s->out_rate = out;
  s->observe = h->observe;
  oa_voice_init(s->v, in);
  oa_voice_floor_init(s->fl);
  // Opening a session is this speaker's consent to be heard by its route; the
  // floor records it and binds the speaker's turns to Hello.route.
  oa_voice_floor_consent(s->fl, 0, true);
  oa_voice_floor_bind(s->fl, 0, TARGET_ROUTE);
  oa_voice_floor_grant(s->fl, 0);
  oa_voice_msg m = {.kind = OA_MSG_WELCOME};
  m.u.welcome.protocol = OA_VOICE_PROTOCOL;
  m.u.welcome.session = B(s->sid);
  m.u.welcome.input_sample_rate = in;
  m.u.welcome.output_sample_rate = out;
  m.u.welcome.provider = B(provider_name());
  m.u.welcome.has_meter = true;
  oa_voice_meter_report_get(s->acct->m, &m.u.welcome.meter);
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  m.u.welcome.max_turn_ms = cfg.max_turn_ms;
  send_msg(s, &m);
  s->state = S_READY;
  logf_("%s: hello speaker=%s route=%s in=%u out=%u profile=%s%s", s->sid, s->speaker, s->route, in, out, s->prof->id,
        s->observe ? " observe" : "");
}

// The client's media epoch outlives a connection: a phone that cut a reply
// and reconnected is ahead of this new session, and drops every reply
// stamped with an older epoch as stale. Any frame the client sends that
// carries a later epoch moves this session up to it.
static void adopt_epoch(session *s, uint64_t client_epoch) {
  if (client_epoch <= oa_voice_epoch(s->v)) return;
  uint64_t e = oa_voice_set_epoch(s->v, client_epoch);
  logf_("%s: client epoch %llu, epoch now %llu", s->sid, (unsigned long long)client_epoch, (unsigned long long)e);
}

static void on_control(session *s, const oa_voice_control *c) {
  if (c->kind != OA_CONTROL_BARGE_IN && c->kind != OA_CONTROL_CANCEL) adopt_epoch(s, c->media_epoch);
  switch (c->kind) {
  case OA_CONTROL_TURN_START: start_turn(s, c); break;
  case OA_CONTROL_TURN_END: {
    if (!s->turn_open || c->turn.len != strlen(s->turn) || memcmp(c->turn.ptr, s->turn, c->turn.len)) {
      send_error(s, OA_ERROR_PROTOCOL, "TURN_END for a turn that is not open", "");
      break;
    }
    // Flush a partial frame by padding it with silence.
    uint32_t fs = oa_voice_input_frame_samples(s->v);
    if (s->pend_n && !s->turn_refused) {
      memset(s->pend + s->pend_n, 0, (fs - s->pend_n) * 2);
      uplink_frame(s, s->pend);
      s->pend_n = 0;
    }
    end_turn(s, false);
    break;
  }
  case OA_CONTROL_BARGE_IN:
    (void)take_quiet(s, s->turn_no);   // loud enough to interrupt: someone talking (or an echo is_echo will catch)
    if (c->played_samples) s->played_samples = c->played_samples;
    cut_speech(s, c->media_epoch, true);
    // Most barge-ins on a phone's loudspeaker are the reply's own echo:
    // rather than a gap until the words show which, it goes on quietly now.
    // The phone may send the barge-in before the turn it starts.
    s->barge_wait = false;
    if (s->cut_pcm && s->cut_at && echo_aware()) {
      if (s->turn_open) resume_cut(s, s->turn_no, s->turn, true);
      else s->barge_wait = true;
    }
    break;
  case OA_CONTROL_CANCEL:
    for (int i = 0; i < s->nasr; i++) s->asr[i]->cancel = 1;
    if (s->turn_open) {
      s->turn_open = false;
      oa_voice_cancel_turn(s->v);
      send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    }
    cut_speech(s, c->media_epoch, false);
    s->dropped_upto = s->turn_no;
    drop_final(s);
    break;
  case OA_CONTROL_PLAYBACK:
    // What the phone says it has played of the current reply: in a verbose
    // run, every report (about four a second while a reply plays), so a
    // silent client shows up as a stuck count or as no reports at all.
    if (g_verbose)
      logf_("%s: playback %.*s played %llu samples", s->sid, (int)c->utterance.len, (const char *)c->utterance.ptr,
            (unsigned long long)c->played_samples);
    s->played_samples = c->played_samples;
    break;
  case OA_CONTROL_PING: {
    oa_voice_msg m = {.kind = OA_MSG_CONTROL};
    m.u.control.kind = OA_CONTROL_PONG;
    m.u.control.at_ms = c->at_ms;
    send_msg(s, &m);
    break;
  }
  case OA_CONTROL_PONG: break;
  default: send_error(s, OA_ERROR_PROTOCOL, "unexpected control kind", "");
  }
}

// The page says whom this speaker faces (Route: target, reason "facing"),
// by explicit binding, never from the words; "" faces nobody again.
static void on_route(session *s, const oa_voice_route *r) {
  if (has_nul(r->target) || r->target.len >= sizeof s->facing) { send_error(s, OA_ERROR_PROTOCOL, "bad Route target", ""); return; }
  copy_bytes(s->facing, sizeof s->facing, r->target);
  if (!s->facing[0]) s->page_turn_no = 0;
  logf_("%s: facing %s", s->sid, s->facing[0] ? s->facing : "nobody");
}

static void collect_sentence(void *ctx, const char *sentence) {
  job *e = ctx;
  session *s = session_by_serial(e->session);
  if (!s) return;
  snprintf(e->text, sizeof e->text, "%s", sentence);
  on_reply_sentence(s, e);
}

// The page's answer to the final transcript it was given (a client Speech:
// utterance.text, turn): spoken as the character it faces, sentence by
// sentence through the same synthesis and meter as the agent's replies.
static void on_page_speech(session *s, const oa_voice_speech *sp) {
  oa_voice_bytes t = sp->has_utterance ? sp->utterance.text : (oa_voice_bytes){0};
  if (!s->page_turn_no || sp->turn.len != strlen(s->page_turn) || memcmp(sp->turn.ptr, s->page_turn, sp->turn.len)) {
    send_error(s, OA_ERROR_PROTOCOL, "Speech for a turn the page doesn't answer", "");
    return;
  }
  uint32_t turn_no = s->page_turn_no;
  s->page_turn_no = 0;
  if (turn_no <= s->dropped_upto) return;   // barged or cancelled meanwhile
  if (!t.len || has_nul(t) || t.len > MAX_TEXT) {
    send_error(s, OA_ERROR_PROTOCOL, "bad Speech text", s->page_turn);
    send_turn_event(s, s->page_turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  static job e;
  memset(&e, 0, sizeof e);
  e.kind = JOB_SENTENCE;
  e.session = s->serial;
  e.turn_no = turn_no;
  snprintf(e.turn, sizeof e.turn, "%s", s->page_turn);
  e.ok = true;
  e.started = now_ms();
  char text[MAX_TEXT + 1];
  copy_bytes(text, sizeof text, t);
  speech_split(text, collect_sentence, &e);
  utter *u = utter_by_turn(s, turn_no);
  logf_("%s: %s %s answers (%zu bytes)", s->sid, s->page_turn, s->facing[0] ? s->facing : "the page", strlen(text));
  if (!u) { send_turn_event(s, s->page_turn, OA_TURN_PHASE_ENDED, NULL, 0); return; }
  snprintf(u->who, sizeof u->who, "%s", s->facing);
  if (!u->text_done && u->overflow[0]) { u->seg[u->nseg].end = strlen(u->text); synthesize(s, u, u->nseg, u->overflow); }
  u->text_done = true;
  pump_speech(s, now_ms());
}

static void on_frame(session *s, const uint8_t *p, size_t n) {
  oa_voice_msg m;
  memset(&m, 0, sizeof m);
  if (n > OA_VOICE_MAX_FRAME_BYTES || !oa_voice_decode(p, n, &m)) {
    send_error(s, OA_ERROR_PROTOCOL, "malformed frame", "");
    if (s->state == S_HELLO) s->state = S_CLOSING;
    return;
  }
  if (s->state == S_HELLO) {
    if (m.kind != OA_MSG_HELLO) { send_error(s, OA_ERROR_PROTOCOL, "the first frame must be Hello", ""); s->state = S_CLOSING; return; }
    on_hello(s, &m.u.hello);
    return;
  }
  switch (m.kind) {
  case OA_MSG_AUDIO: adopt_epoch(s, m.u.audio.media_epoch); on_uplink_audio(s, &m.u.audio); break;
  case OA_MSG_CONTROL: on_control(s, &m.u.control); break;
  case OA_MSG_DECISION: {
    // The seam for an external decider: applied to this session's engine.
    // Dropped (late, stale epoch, low confidence) decisions change nothing.
    bool kept = oa_voice_decide(s->v, &m.u.decision);
    if (g_verbose) logf_("%s: decision stream=%u action=%u %s", s->sid, m.u.decision.stream, m.u.decision.action, kept ? "applied" : "dropped");
    break;
  }
  case OA_MSG_FEATURES: break;  // client-side features: accepted, not used yet
  case OA_MSG_ROUTE: on_route(s, &m.u.route); break;
  case OA_MSG_SPEECH: on_page_speech(s, &m.u.speech); break;
  case OA_MSG_HELLO: send_error(s, OA_ERROR_PROTOCOL, "Hello was already received", ""); break;
  default: send_error(s, OA_ERROR_PROTOCOL, "a client may not send this frame", ""); break;
  }
}

