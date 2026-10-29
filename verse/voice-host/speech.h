// Speech providers and agents behind one interface. Every call here may
// block on the network, so the host runs them on worker threads only.
//
// Providers: "google" (Speech-to-Text v2 chirp_3, Text-to-Speech Chirp 3 HD)
// and "local" (offline, deterministic, for tests). Agents: "echo" and
// "gemini" (Vertex AI generateContent).
#ifndef OA_VOICE_SPEECH_H
#define OA_VOICE_SPEECH_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef enum { PROVIDER_LOCAL = 0, PROVIDER_GOOGLE = 1 } speech_provider;
typedef enum { AGENT_ECHO = 0, AGENT_GEMINI = 1 } speech_agent;

typedef struct {
  const char *id;            // profile id, e.g. "foreman"
  uint32_t revision;
  const char *provider_voice;
  const char *language;
} speech_profile;

// Configuration read by every call; set once before threads start.
typedef struct {
  char project[64];          // GCP project, default openagentsgemini
  char gcloud_config[512];   // CLOUDSDK_CONFIG for the access token
  char model[64];            // Gemini model, default gemini-2.5-flash
  char cache_dir[512];       // render cache, default ~/.cache/openagents-voice/renders
  bool verbose;
} speech_config;

extern speech_config g_speech;

void speech_init(void);  // curl_global_init + defaults; call once from main
// The profile for an id ("" = the default "foreman"); NULL if unknown.
const speech_profile *speech_find_profile(const char *id);

// True if an access token can be had (google provider usable). Never prints it.
bool speech_google_ready(char *err, size_t errcap);

// Recognize 16 kHz mono PCM. Writes the transcript (may be "" for no speech).
bool speech_recognize(speech_provider p, const int16_t *pcm, size_t n, char *out, size_t cap, char *err, size_t errcap);

// Synthesize text at `rate` Hz. Returns malloc'd samples (*n set), or NULL
// with err. Google renders are cached on disk by SHA-256 over the profile id,
// revision and text (one file per output rate). *cached reports a cache hit.
int16_t *speech_synthesize(speech_provider p, const speech_profile *prof, const char *text, uint32_t rate,
                           size_t *n, bool *cached, char *err, size_t errcap);

// One remembered exchange for the agent's context.
typedef struct { char user[600]; char agent[600]; } speech_exchange;

#define MAX_REPLY 600  // Utterance.text max_bytes

// Called on the calling (worker) thread with each complete sentence of the
// reply, in order, as soon as voice-reply.coax says it has ended.
typedef void (*speech_sentence_fn)(void *ctx, const char *sentence);

// The agent's spoken reply (<= 30 words for gemini) to a final transcript,
// streamed: gemini uses streamGenerateContent (server-sent events), and each
// sentence goes to on_sentence while the rest is still arriving. *cancel
// (may be NULL) going nonzero aborts the stream. out gets the whole reply.
bool speech_agent_stream(speech_agent a, const char *route, const speech_exchange *history, size_t nhistory,
                         const char *transcript, speech_sentence_fn on_sentence, void *ctx, const volatile int *cancel,
                         char *out, size_t cap, char *err, size_t errcap);

// Cuts finished text (a reply the page wrote) into sentences with the same
// rule as a streamed reply, calling on_sentence for each on this thread.
void speech_split(const char *text, speech_sentence_fn on_sentence, void *ctx);
// Is there an agent instruction for this route (the ROUTES table in speech.c)?
bool speech_route_known(const char *route);

// Refreshes an aging access token and opens connections to the Google hosts
// the session will use, so the next real call skips DNS, TCP and TLS.
void speech_warm(bool speech, bool agent);

// The render cache key (exposed for tests and the client).
void speech_render_key(const char *profile, uint32_t revision, const char *text, uint8_t key[32]);
#endif
