/* Split from metaverse/voice-host/host.c (#13848): turn lifecycle and downlink audio streaming. */
// ---------------------------------------------------------------- the turn path

static void deliver_final(session *s, uint32_t turn_no, const char *turn, const char *text);
static void plan_final(session *s);
static void resume_cut(session *s, uint32_t turn_no, const char *turn, bool quiet);

static void start_turn(session *s, const oa_voice_control *c) {
  char turn[65];
  copy_bytes(turn, sizeof turn, c->turn);
  if (!turn[0] || has_nul(c->turn)) { send_error(s, OA_ERROR_PROTOCOL, "TURN_START needs a turn id", ""); return; }
  if (s->turn_open) { send_error(s, OA_ERROR_PROTOCOL, "TURN_START while a turn is open", turn); return; }
  // A previous turn still waiting on its in-flight recognition is decided
  // now, without it: the whole turn is recognized.
  if (s->final_pending) {
    s->nflight = 0;
    s->done_cover = 0;
    s->done_text[0] = 0;
    plan_final(s);
  }
  warm();
  s->turn_no++;
  // Over the reply (the phone plays a little behind this host's clock, so just
  // after it too): quiet until a barge-in says otherwise.
  if ((s->speaking || (s->spoke_at && now_ms() - s->spoke_at < 600)) && echo_aware()) {
    if (s->nquiet == 8) { memmove(s->quiet, s->quiet + 1, sizeof s->quiet[0] * 7); s->nquiet--; }
    s->quiet[s->nquiet++] = s->turn_no;
  }
  snprintf(s->turn, sizeof s->turn, "%s", turn);
  s->turn_open = true;
  s->tn = 0;
  s->nvoiced = 0;
  s->pend_n = 0;
  s->partial_at = 0;
  s->voiced_end = 0;
  s->nflight = 0;
  s->done_cover = 0;
  s->done_cut_ms = 0;
  s->done_text[0] = 0;
  // pay/voice-meter.coax decides whether this turn may run at all (worst case:
  // the longest turn the host accepts plus the longest reply).
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (!oa_voice_meter_may_run(s->acct->m, cfg.max_turn_ms, MAX_TEXT)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    return;
  }
  // voice-floor.coax: the speaker must hold consent and the floor to be heard.
  if (!oa_voice_floor_may_hear(s->fl, 0)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_UNAUTHORIZED, "this speaker may not be heard now", s->turn);
    return;
  }
  s->turn_refused = false;
  send_turn_event(s, s->turn, OA_TURN_PHASE_STARTED, NULL, 0);
  oa_voice_msg r = {.kind = OA_MSG_ROUTE};
  r.u.route.turn = B(s->turn);
  r.u.route.target = B(oa_voice_floor_route(s->fl, 0) == TARGET_ROUTE ? s->route : "");
  r.u.route.reason = B("default");
  send_msg(s, &r);
  if (s->barge_wait) {
    s->barge_wait = false;
    if (s->cut_pcm && s->cut_at && now_ms() - s->cut_at < 1000) resume_cut(s, s->turn_no, s->turn, true);
  }
}

// Recognize the turn so far: every ~1 s of new audio (a PARTIAL), and early,
// as soon as voice-reply.coax sees trailing silence after speech, so that a
// finished recognition already covers every voiced frame when TURN_END comes.
static size_t flight_max(const session *s) {
  size_t m = 0;
  for (int i = 0; i < s->nflight; i++) if (s->flight[i] > m) m = s->flight[i];
  return m;
}

static void maybe_recognize(session *s) {
  if (!metaverse_voice_asr_partials(g_recognizer)) return;  // a resident local model recognizes each turn once
  if (s->turn_refused || s->tn < OA_VOICE_FRAME_SAMPLES || s->nflight == MAX_FLIGHT) return;
  uint32_t silence = ms_of(s->tn - s->voiced_end);
  size_t cover = s->done_cover > flight_max(s) ? s->done_cover : flight_max(s);
  // Early recognition may overlap one still in flight (a mid-sentence pause
  // started it); the regular cadence waits for the last one to finish.
  bool early = metaverse_voice_reply_recognize_early(silence, ms_of(s->voiced_end), ms_of(cover));
  bool cadence = s->nflight == 0 && s->tn - s->partial_at >= PARTIAL_EVERY;
  if (!early && !cadence) return;
  // pay/voice-meter.coax decides whether re-sending the turn so far fits; the
  // usage is recorded like any other recognized audio.
  uint64_t ms = ms_of(s->tn);
  if (!oa_voice_meter_may_run(s->acct->m, ms, 0)) return;
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->pcm = malloc(s->tn * 2);
  if (!j->pcm) { free(j); return; }
  oa_voice_meter_charge(s->acct->m, ms, 0);  // records usage; false = now past budget
  memcpy(j->pcm, s->tb, s->tn * 2);
  j->n = s->tn;
  j->cover = s->tn;
  j->cut_silence_ms = silence;
  j->kind = JOB_RECOGNIZE;
  j->session = s->serial;
  j->turn_no = s->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", s->turn);
  s->partial_at = s->tn;
  s->flight[s->nflight++] = s->tn;
  submit(j);
}

static void submit_recognition(session *s, job_kind kind, const int16_t *pcm, size_t n, const char *prefix) {
  uint64_t ms = ms_of(n);
  // Decide before the audio goes to the provider; then record it.
  if (!oa_voice_meter_may_run(s->acct->m, ms, 0)) {
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  oa_voice_meter_charge(s->acct->m, ms, 0);  // records usage; false = now past budget
  job *j = calloc(1, sizeof *j);
  if (j) j->pcm = malloc(n * 2 + 2);
  if (!j || !j->pcm) { free(j); send_error(s, OA_ERROR_INTERNAL, "out of memory", s->turn); return; }
  memcpy(j->pcm, pcm, n * 2);
  j->n = n;
  j->kind = kind;
  j->session = s->serial;
  j->turn_no = s->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", s->turn);
  if (prefix) snprintf(j->prefix, sizeof j->prefix, "%s", prefix);
  submit(j);
}

// Parakeet: the ended turn goes to the resident model once, cut into bounded
// windows by metaverse/voice-asr.coax; no speech and a turn too long for its
// calls are decided there before any model call.
static void parakeet_final(session *s) {
  s->final_pending = false;
  s->plan = "parakeet";
  int64_t windows = metaverse_voice_asr_plan((int64_t)s->fin_n, (int64_t)s->fin_nvoiced);
  uint64_t epoch = oa_voice_epoch(s->v);
  if (windows <= 0) {
    int64_t v = metaverse_voice_asr_admit(s->turn_no, s->dropped_upto, epoch, epoch, false, windows, true, 0);
    logf_("%s: %s %s without a model call (%zu samples, %zu voiced frames)", s->sid, s->turn,
          v == 1 ? "no speech" : "refused", s->fin_n, s->fin_nvoiced);
    if (v == 5) {
      send_error(s, OA_ERROR_TOO_LONG, "the turn is longer than four Parakeet calls", s->turn);
      send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    } else {
      deliver_final(s, s->turn_no, s->turn, "");
    }
    drop_final(s);
    return;
  }
  uint64_t ms = ms_of(s->fin_n);
  if (!oa_voice_meter_may_run(s->acct->m, ms, 0)) {
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    drop_final(s);
    return;
  }
  if (s->nasr == (int)(sizeof s->asr / sizeof s->asr[0])) {
    send_error(s, OA_ERROR_LIMIT, "too many turns waiting for recognition", s->turn);
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    drop_final(s);
    return;
  }
  oa_voice_meter_charge(s->acct->m, ms, 0);
  job *j = calloc(1, sizeof *j);
  if (!j) { send_error(s, OA_ERROR_INTERNAL, "out of memory", s->turn); drop_final(s); return; }
  j->kind = JOB_PARAKEET;
  j->pcm = s->fin;
  j->n = s->fin_n;
  j->speech = s->fin_sp;
  j->frames = s->fin_n / OA_VOICE_FRAME_SAMPLES;
  s->fin = NULL;
  s->fin_sp = NULL;
  j->windows = (int)windows;
  j->epoch = epoch;
  j->session = s->serial;
  j->turn_no = s->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", s->turn);
  s->asr[s->nasr++] = j;
  if (g_verbose) logf_("%s: %s to parakeet: %zu samples in %d window(s)", s->sid, s->turn, j->n, j->windows);
  submit(j);
  drop_final(s);
}

// Decides the ended turn's final transcript with voice-reply.coax.
static void plan_final(session *s) {
  if (g_recognizer == PROVIDER_PARAKEET) { parakeet_final(s); return; }
  size_t voiced = s->fin_voiced ? s->fin_voiced : s->fin_n;  // no voiced frame seen: all of it must be covered
  int64_t p = metaverse_voice_reply_final_plan(ms_of(voiced), ms_of(s->done_cover), s->done_text[0] != 0,
                                              s->done_cut_ms, ms_of(flight_max(s)));
  if (p == 3) {  // wait for the in-flight recognition, then decide again
    s->final_pending = true;
    s->plan = "wait";
    return;
  }
  s->final_pending = false;
  if (p == 1) {
    s->plan = "partial";
    char text[MAX_TEXT + 1];
    snprintf(text, sizeof text, "%s", s->done_text);
    drop_final(s);
    deliver_final(s, s->turn_no, s->turn, text);
    return;
  }
  if (p == 2 && s->done_cover < s->fin_n) {
    s->plan = "tail";
    submit_recognition(s, JOB_TAIL, s->fin + s->done_cover, s->fin_n - s->done_cover, s->done_text);
  } else {
    s->plan = "whole";
    submit_recognition(s, JOB_FINAL, s->fin, s->fin_n, NULL);
  }
  drop_final(s);
}

static void end_turn(session *s, bool too_long) {
  if (!s->turn_open) return;
  s->turn_open = false;
  if (s->turn_refused) return;
  if (too_long) send_error(s, OA_ERROR_TOO_LONG, "the turn exceeded max_turn_ms", s->turn);
  if (s->tn < OA_VOICE_FRAME_SAMPLES) {
    send_turn_event(s, s->turn, OA_TURN_PHASE_ENDED, NULL, 0);
    return;
  }
  s->t_end = now_ms();
  drop_final(s);
  s->fin = s->tb;  // the turn's audio, kept until its final transcript is decided
  s->fin_n = s->tn;
  s->fin_voiced = s->voiced_end;
  s->fin_sp = s->sp;
  s->fin_nvoiced = s->nvoiced;
  s->tb = NULL;
  s->sp = NULL;
  s->tn = s->tcap = 0;
  s->nvoiced = 0;
  plan_final(s);
}

static void uplink_frame(session *s, const int16_t *frame) {
  int16_t out16k[OA_VOICE_FRAME_SAMPLES];
  oa_voice_features f;
  uint32_t ev = oa_voice_uplink(s->v, frame, out16k, &f);
  observe_features(s, &f);
  observe_decision(s, ev);
  // A DROP decision (from the policy or an observer) keeps this frame from
  // recognition. Turn boundaries stay the client's (its engine decided them).
  if (ev & OA_EV_DROPPED) return;
  if (s->tn + OA_VOICE_FRAME_SAMPLES > s->tcap) {
    size_t c = s->tcap ? s->tcap * 2 : OA_VOICE_RATE * 4;
    int16_t *q = realloc(s->tb, c * 2);
    if (!q) return;
    s->tb = q;
    uint8_t *p = realloc(s->sp, c / OA_VOICE_FRAME_SAMPLES);
    if (!p) return;
    s->sp = p;
    s->tcap = c;
  }
  memcpy(s->tb + s->tn, out16k, sizeof out16k);
  float sp = f.speech < 0 ? 0 : f.speech > 1 ? 1 : f.speech;
  s->sp[s->tn / OA_VOICE_FRAME_SAMPLES] = (uint8_t)(sp * 255.0f + 0.5f);
  s->tn += OA_VOICE_FRAME_SAMPLES;
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (f.speech >= cfg.speech_threshold) { s->voiced_end = s->tn; s->nvoiced++; }
}

static void on_uplink_audio(session *s, const oa_voice_audio *a) {
  if (!s->turn_open) { send_error(s, OA_ERROR_PROTOCOL, "uplink audio outside a turn", ""); return; }
  if (s->turn_refused) return;
  if (a->turn.len != strlen(s->turn) || memcmp(a->turn.ptr, s->turn, a->turn.len)) {
    send_error(s, OA_ERROR_PROTOCOL, "uplink audio for a turn that is not open", s->turn);
    return;
  }
  if ((a->codec != OA_CODEC_PCM_S16LE && a->codec != OA_CODEC_UNSPECIFIED) || (a->sample_rate && a->sample_rate != s->in_rate) ||
      (a->data.len & 1)) {
    send_error(s, OA_ERROR_PROTOCOL, "uplink audio must be PCM s16le at the Hello's input rate", s->turn);
    return;
  }
  uint32_t fs = oa_voice_input_frame_samples(s->v);
  const uint8_t *d = a->data.ptr;
  size_t n = a->data.len / 2;
  for (size_t i = 0; i < n; i++) {
    s->pend[s->pend_n++] = (int16_t)(d[2 * i] | d[2 * i + 1] << 8);
    if (s->pend_n == fs) {
      uplink_frame(s, s->pend);
      s->pend_n = 0;
    }
  }
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (ms_of(s->tn) > cfg.max_turn_ms) { end_turn(s, true); return; }
  maybe_recognize(s);
}

// Whether turn_no was quiet (see session.quiet), forgetting it.
static bool take_quiet(session *s, uint32_t turn_no) {
  for (int i = 0; i < s->nquiet; i++)
    if (s->quiet[i] == turn_no) { s->quiet[i] = s->quiet[--s->nquiet]; return true; }
  return false;
}

static void clear_cut(session *s) {
  free(s->cut_pcm);
  s->cut_pcm = NULL; s->cut_n = 0; s->cut_at = 0; s->cut_resumes = 0;
}

// Barge-in or cancel: cut playback, move the epoch forward, stop the agent,
// and discard every reply still being made for turns up to now.
static void cut_speech(session *s, uint64_t client_epoch, bool barge) {
  utter *u = s->speech;
  bool was = s->speaking && u;
  char turn[65] = "";
  if (u) snprintf(turn, sizeof turn, "%s", u->turn);
  if (barge && was && u->text[0]) {
    free(s->cut_pcm);
    snprintf(s->cut_text, sizeof s->cut_text, "%s", u->text);
    s->cut_pcm = u->pcm; u->pcm = NULL;   // kept for resume_cut; utter_free_all frees the rest
    s->cut_n = u->n; s->cut_played = (size_t)s->played_samples; s->cut_rest = u->appended ? u->seg[u->appended - 1].end : u->base; s->cut_at = now_ms();
  } else if (!barge) {
    clear_cut(s);
  }
  uint64_t e = oa_voice_cut(s->v);
  if (client_epoch > e) e = oa_voice_set_epoch(s->v, client_epoch);
  oa_voice_floor_bump(s->fl);
  s->dropped_upto = s->turn_no - (s->turn_open ? 1 : 0);
  for (int i = 0; i < s->nasr; i++) s->asr[i]->stop = 1;   // stale now: stop between windows
  if (!s->turn_open) drop_final(s);
  stop_agent(s);
  utter_free_all(s);
  if (was) send_turn_event(s, turn, OA_TURN_PHASE_INTERRUPTED, NULL, s->played_samples);
  logf_("%s: %s, epoch now %llu", s->sid, barge ? "barge-in" : "cancel", (unsigned long long)e);
}

// ---------------------------------------------------------------- downlink

static void log_latency(session *s, const utter *u, uint64_t first_sent) {
  logf_("%s: latency %s plan=%s recognition=%llu agent_first_sentence=%llu tts_first=%llu total=%llu ms",
        s->sid, u->turn, u->plan ? u->plan : "?", (unsigned long long)(u->t_final - u->t_end),
        (unsigned long long)(u->t_sentence - u->t_final), (unsigned long long)(u->t_audio - u->t_sentence),
        (unsigned long long)(first_sent - u->t_end));
}

static bool utter_done(const utter *u) { return u->text_done && u->appended == u->nseg; }
static void send_speech(session *s, const utter *u);

static void start_next_utterance(session *s) {
  utter *u = s->speech;
  if (!u || s->speaking || !u->n) return;
  s->speaking = true;
  s->play_clock_us = now_us();
  s->speak_floor_epoch = oa_voice_floor_epoch(s->fl);
  s->played_samples = 0;
  oa_voice_playing(s->v, true);
  u->started = true;
  send_speech(s, u);
}

// Speech for an utterance: first when its audio starts, then again (same
// utterance id, same epoch) each time the streamed reply grows a sentence,
// carrying the full text known so far, so a caption shows the whole reply.
// A client treats a repeated id as a text update, not a new utterance.
static void send_speech(session *s, const utter *u) {
  oa_voice_msg m = {.kind = OA_MSG_SPEECH};
  m.u.speech.has_utterance = true;
  m.u.speech.utterance.id = B(u->id);
  m.u.speech.utterance.speaker = B(u->who[0] ? u->who : s->route);
  m.u.speech.utterance.text = B(u->text);
  m.u.speech.utterance.interruptible = true;
  m.u.speech.has_speaker = true;
  m.u.speech.speaker.id = B(u->who[0] ? u->who : s->route);
  m.u.speech.speaker.role = OA_SPEAKER_ROLE_AGENT;
  m.u.speech.speaker.profile = B(s->prof->id);
  m.u.speech.speaker.profile_revision = s->prof->revision;
  m.u.speech.sample_rate = s->out_rate;
  m.u.speech.media_epoch = oa_voice_epoch(s->v);
  m.u.speech.turn = B(u->turn);
  send_msg(s, &m);
}

static void end_utterance(session *s, utter *u) {
  if (u->started) s->spoke_at = now_ms();
  if (u->started)
    logf_("%s: %s %s spoken: %zu samples in %d sentence(s), gaps %llu ms", s->sid, u->turn, u->id, u->n, u->nseg,
          (unsigned long long)(u->gap_us / 1000));
  if (u->started) {
    oa_voice_msg c = {.kind = OA_MSG_CONTROL};
    c.u.control.kind = OA_CONTROL_SPEECH_END;
    c.u.control.turn = B(u->turn);
    c.u.control.utterance = B(u->id);
    c.u.control.media_epoch = oa_voice_epoch(s->v);
    c.u.control.played_samples = u->n;
    send_msg(s, &c);
  }
  send_turn_event(s, u->turn, OA_TURN_PHASE_ENDED, NULL, u->n);
  // A quiet reply that played out before its words decided: nothing left to
  // resume, though is_echo still knows the reply's words.
  if (u->quiet) { free(s->cut_pcm); s->cut_pcm = NULL; }
  s->speech = u->next;
  utter_free(u);
  s->speaking = false;
  oa_voice_playing(s->v, false);
}

// Sends reply audio up to LEAD_MS ahead of the client's playback clock, in
// chunks of at most 200 ms (and at most Audio.data's 9600 bytes). Audio from
// later sentences continues the same Utterance with no gap when it is ready
// in time; the playback clock carries across sentences.
static void pump_speech(session *s, uint64_t now) {
  (void)now;
  for (;;) {
    utter *u = s->speech;
    if (!u) return;
    if (!s->speaking) {
      // A reply that ended with no audio at all (every sentence failed).
      if (utter_done(u) && !u->n) { end_utterance(s, u); continue; }
      start_next_utterance(s);
      if (!s->speaking) return;
    }
    if (!oa_voice_floor_epoch_ok(s->fl, s->speak_floor_epoch)) { utter_free_all(s); return; }
    uint32_t chunk = s->out_rate / 5;
    if (chunk > OA_VOICE_MAX_AUDIO_BYTES / 2) chunk = OA_VOICE_MAX_AUDIO_BYTES / 2;
    uint64_t t = now_us();
    while (u->sent < u->n && s->play_clock_us < t + LEAD_MS * 1000u) {
      uint32_t k = (uint32_t)(u->n - u->sent < chunk ? u->n - u->sent : chunk);
      int16_t pcm[OA_VOICE_MAX_AUDIO_BYTES / 2];
      memcpy(pcm, u->pcm + u->sent, k * 2);
      if (u->quiet)
        for (uint32_t i = 0; i < k; i++) pcm[i] = (int16_t)(pcm[i] * QUIET_RESUME);
      oa_voice_features f;
      uint32_t seq = oa_voice_downlink_sequence(s->v);
      uint32_t ev = oa_voice_downlink(s->v, pcm, k, s->out_rate, &f);  // gain/duck/pause from decisions
      observe_features(s, &f);
      observe_decision(s, ev);
      oa_voice_msg m = {.kind = OA_MSG_AUDIO};
      m.u.audio.turn = B(u->turn);
      m.u.audio.sequence = seq;
      m.u.audio.sample_rate = s->out_rate;
      m.u.audio.codec = OA_CODEC_PCM_S16LE;
      m.u.audio.data.ptr = (const uint8_t *)pcm;
      m.u.audio.data.len = k * 2;
      m.u.audio.media_epoch = oa_voice_epoch(s->v);
      m.u.audio.utterance = B(u->id);
      send_msg(s, &m);
      if (u->sent == 0) log_latency(s, u, now_ms());
      u->sent += k;
      if (s->play_clock_us < t) {  // an underrun: playback restarts now
        if (u->sent) u->gap_us += t - s->play_clock_us;
        s->play_clock_us = t;
      }
      s->play_clock_us += (uint64_t)k * 1000000u / s->out_rate;
    }
    if (u->sent < u->n || !utter_done(u)) return;
    // Everything is sent; the utterance ends when its last sample has played.
    if (t < s->play_clock_us) return;
    end_utterance(s, u);
  }
}

static utter *utter_by_no(session *s, uint32_t no) {
  for (utter *u = s->speech; u; u = u->next)
    if (u->no == no) return u;
  return NULL;
}
static utter *utter_by_turn(session *s, uint32_t turn_no) {
  for (utter *u = s->speech; u; u = u->next)
    if (u->turn_no == turn_no) return u;
  return NULL;
}

static void synthesize(session *s, utter *u, int seg, const char *text) {
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->kind = JOB_TTS;
  j->session = s->serial;
  j->turn_no = u->turn_no;
  snprintf(j->turn, sizeof j->turn, "%s", u->turn);
  snprintf(j->text, sizeof j->text, "%s", text);
  j->rate = s->out_rate;
  j->prof = s->prof;
  j->utter_no = u->no;
  j->seg = (uint32_t)seg;
  u->nseg = seg + 1;
  submit(j);
}

// A complete sentence of the reply: charge it, start rendering it now.
static void on_reply_sentence(session *s, const job *e) {
  utter *u = utter_by_turn(s, e->turn_no);
  if (!u) {
    u = calloc(1, sizeof *u);
    if (!u) return;
    u->no = ++s->utt_no;
    snprintf(u->id, sizeof u->id, "u%u", u->no);
    snprintf(u->turn, sizeof u->turn, "%s", e->turn);
    u->turn_no = e->turn_no;
    u->t_end = s->t_end;
    u->t_final = s->t_final;
    u->t_sentence = now_ms();
    u->plan = s->plan;
    utter **t = &s->speech;
    while (*t) t = &(*t)->next;
    *t = u;
  }
  if (u->text_done) return;
  size_t len = strlen(e->text);
  // Decide before synthesis; then record the characters.
  if (!oa_voice_meter_may_run(s->acct->m, 0, len)) {
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", u->turn);
    send_meter(s);
    u->text_done = true;
    stop_agent(s);
    return;
  }
  oa_voice_meter_charge(s->acct->m, 0, len);
  size_t have = strlen(u->text);
  snprintf(u->text + have, sizeof u->text - have, "%s%s", have ? " " : "", e->text);
  clip_utf8(u->text, MAX_TEXT);
  if (u->started) send_speech(s, u);
  if (u->nseg < MAX_SEGS - 1) { u->seg[u->nseg].end = strlen(u->text); synthesize(s, u, u->nseg, e->text); }
  else {
    size_t o = strlen(u->overflow);
    snprintf(u->overflow + o, sizeof u->overflow - o, "%s%s", o ? " " : "", e->text);
  }
}

static void on_reply_audio(session *s, job *j) {
  utter *u = utter_by_no(s, j->utter_no);
  if (!u || (int)j->seg >= u->nseg) return;  // cut by barge-in or cancel
  if (!j->ok) send_error(s, OA_ERROR_PROVIDER, j->err, u->turn);
  u->seg[j->seg].ready = true;
  u->seg[j->seg].pcm = j->ok ? j->pcm : NULL;
  u->seg[j->seg].n = j->ok ? j->n : 0;
  if (j->ok) j->pcm = NULL;
  if (!u->t_audio && j->ok) u->t_audio = now_ms();
  while (u->appended < u->nseg && u->seg[u->appended].ready) {
    size_t k = u->seg[u->appended].n;
    if (u->n + k > u->cap) {
      size_t c = u->cap ? u->cap : (size_t)s->out_rate * 4;
      while (c < u->n + k) c *= 2;
      int16_t *q = realloc(u->pcm, c * 2);
      if (!q) break;
      u->pcm = q;
      u->cap = c;
    }
    if (k) memcpy(u->pcm + u->n, u->seg[u->appended].pcm, k * 2);
    u->n += k;
    free(u->seg[u->appended].pcm);
    u->seg[u->appended].pcm = NULL;
    u->appended++;
  }
  pump_speech(s, now_ms());
}

