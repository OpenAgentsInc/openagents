/* Split from metaverse/voice-host/host.c (#13848): worker thread pool and speech jobs. */
// ---------------------------------------------------------------- jobs (worker threads)

typedef enum {
  JOB_RECOGNIZE,  // a partial (every ~1 s) or an early recognition (trailing silence)
  JOB_FINAL,      // the whole turn
  JOB_TAIL,       // the audio after the last partial; its text is appended
  JOB_AGENT,      // the streaming agent; posts JOB_SENTENCE events while it runs
  JOB_SENTENCE,   // an event, not work: one complete reply sentence
  JOB_TTS,        // one sentence of the reply
  JOB_WARM,       // token refresh and connection warm-up
  JOB_PARAKEET,   // a whole ended turn through Parakeet's bounded windows (metaverse/voice-asr.coax)
} job_kind;

typedef struct job {
  job_kind kind;
  uint64_t session;          // session serial; results for a closed session are dropped
  uint32_t turn_no;
  char turn[65];
  int16_t *pcm;              // in: audio to recognize; out (TTS): rendered samples
  size_t n;
  size_t cover;              // RECOGNIZE: samples of the turn this covers
  uint32_t cut_silence_ms;   // RECOGNIZE: silence at its cut point
  uint32_t rate;
  const speech_profile *prof;
  uint32_t utter_no, seg;    // TTS: which utterance and sentence
  speech_exchange hist[HISTORY];
  size_t nhist;
  char route[129];           // AGENT: whose instruction to use
  char prefix[MAX_TEXT + 1]; // TAIL: the partial's text the tail is appended to
  char text[MAX_TEXT + 1];   // in: transcript (AGENT) or sentence (TTS); out: transcript (STT)
  char reply[MAX_TEXT + 1];  // out (AGENT)
  volatile int cancel;       // AGENT: set by the main thread to stop the stream; PARAKEET: the client cancelled
  volatile int stop;         // PARAKEET: the result will be stale (barge-in): stop between windows
  uint8_t *speech;           // PARAKEET: each 20 ms frame's speech likelihood, 0..255
  size_t frames;
  int windows, windows_done; // PARAKEET: model calls planned and made
  int64_t bytes;             // PARAKEET: transcript bytes, -1 past MAX_TEXT
  uint64_t epoch;            // PARAKEET: the session's media epoch when the turn ended
  double infer_ms;           // PARAKEET: the engine's own time, summed over windows
  int tries;                 // AGENT: retried once when the call failed before any reply
  bool ok, cached;
  uint64_t started;
  char err[300];
  struct job *next;
} job;

static pthread_mutex_t q_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t q_cond = PTHREAD_COND_INITIALIZER;
static job *todo_head, *todo_tail, *done_head, *done_tail;
static int wake_pipe[2];

static void post_done(job *j) {
  pthread_mutex_lock(&q_lock);
  if (done_tail) done_tail->next = j;
  else done_head = j;
  done_tail = j;
  pthread_mutex_unlock(&q_lock);
  char c = 1;
  (void)!write(wake_pipe[1], &c, 1);
}

// Worker thread: one finished sentence of a streaming reply goes straight to
// the main thread, which starts its synthesis while the rest still arrives.
static void on_sentence(void *ctx, const char *sentence) {
  job *a = ctx;
  job *e = calloc(1, sizeof *e);
  if (!e) return;
  e->kind = JOB_SENTENCE;
  e->session = a->session;
  e->turn_no = a->turn_no;
  snprintf(e->turn, sizeof e->turn, "%s", a->turn);
  snprintf(e->text, sizeof e->text, "%s", sentence);
  e->ok = true;
  e->started = a->started;
  post_done(e);
}

// Worker thread: the turn's windows, one bounded model call each, in order.
// metaverse/voice-asr.coax places every cut, converts the samples, joins the
// text and says whether to go on; this only moves bytes and calls the engine.
static void parakeet_run(job *j) {
  static __thread float x[327520];
  char piece[2 * MAX_TEXT + 2];
  uint8_t text[MAX_TEXT + 1];
  int64_t used = 0, start = 0;
  bool ok = true;
  int done = 0;
  coax_v_u8 speech = {j->speech, (int64_t)j->frames};
  while (metaverse_voice_asr_continue(j->cancel || j->stop, ok, done, j->windows)) {
    int64_t end = metaverse_voice_asr_cut(start, (int64_t)j->n, j->windows - done, speech);
    if (end < 0 || end - start > (int64_t)(sizeof x / sizeof x[0])) {
      snprintf(j->err, sizeof j->err, "no bounded plan for the turn");
      ok = false;
      break;
    }
    int64_t w = end - start;
    coax_v_s16 in = {j->pcm + start, w};
    coax_v_f32 out = {x, w};
    double ms = 0;
    ok = metaverse_voice_asr_convert(in, out) && speech_parakeet_window(x, (size_t)w, piece, sizeof piece, &ms, j->err, sizeof j->err);
    j->infer_ms += ms;
    done++;
    start = end;
    if (!ok) break;
    coax_v_u8 dst = {text, MAX_TEXT};
    used = metaverse_voice_asr_append(dst, used, (coax_string){piece, (int64_t)strlen(piece)});
    if (used < 0) { ok = false; break; }
  }
  speech_parakeet_clear();
  text[used > 0 ? used : 0] = 0;
  snprintf(j->text, sizeof j->text, "%s", (const char *)text);
  j->bytes = used;
  j->windows_done = done;
  j->ok = ok;
}

static void job_run(job *j) {
  switch (j->kind) {
  case JOB_RECOGNIZE:
  case JOB_FINAL:
  case JOB_TAIL:
    j->ok = speech_recognize(g_recognizer, j->pcm, j->n, j->text, sizeof j->text, j->err, sizeof j->err);
    clip_utf8(j->text, MAX_TEXT);
    break;
  case JOB_AGENT:
    j->ok = speech_agent_stream(g_agent, j->route, j->hist, j->nhist, j->text, on_sentence, j, &j->cancel, j->reply,
                                sizeof j->reply, j->err, sizeof j->err);
    clip_utf8(j->reply, MAX_TEXT);
    break;
  case JOB_TTS:
    free(j->pcm);
    j->pcm = speech_synthesize(g_synth, j->prof, j->text, j->rate, &j->n, &j->cached, j->err, sizeof j->err);
    j->ok = j->pcm != NULL;
    break;
  case JOB_WARM:
    speech_warm(g_recognizer == PROVIDER_GOOGLE || g_synth == PROVIDER_GOOGLE, g_agent == AGENT_GEMINI);
    break;
  case JOB_PARAKEET: parakeet_run(j); break;
  case JOB_SENTENCE: break;
  }
}

static void *worker(void *arg) {
  (void)arg;
  for (;;) {
    pthread_mutex_lock(&q_lock);
    while (!todo_head) pthread_cond_wait(&q_cond, &q_lock);
    job *j = todo_head;
    todo_head = j->next;
    if (!todo_head) todo_tail = NULL;
    pthread_mutex_unlock(&q_lock);
    j->next = NULL;
    job_run(j);
    post_done(j);
  }
  return NULL;
}

static void submit(job *j) {
  j->started = now_ms();
  pthread_mutex_lock(&q_lock);
  if (todo_tail) todo_tail->next = j;
  else todo_head = j;
  todo_tail = j;
  pthread_cond_signal(&q_cond);
  pthread_mutex_unlock(&q_lock);
}

static void job_free(job *j) {
  free(j->pcm);
  free(j->speech);
  free(j);
}

// Warm-up is shared by every session (one connection cache), so at most
// one every 20 s.
static uint64_t last_warm;
static void warm(void) {
  if (g_recognizer != PROVIDER_GOOGLE && g_synth != PROVIDER_GOOGLE && g_agent != AGENT_GEMINI) return;
  uint64_t now = now_ms();
  if (last_warm && now - last_warm < 20000) return;
  last_warm = now;
  job *j = calloc(1, sizeof *j);
  if (!j) return;
  j->kind = JOB_WARM;
  submit(j);
}

