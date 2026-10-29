// Replays Ice Cream Man input traces recorded from the Rust build
// (~/work/aethyr-demo/tools/icecream-trace) on the Coax rules
// (verse/icecream/rules.coax) and checks every step's state fingerprint and
// events. Traces the Rust bot drove also check that the Coax bot gives the
// same inputs. verse/icecream/test.sh builds and runs it.
//   replay FILE.trace...
//   replay --checks    (the Rust crate's house and bot checks)
#include "IceCream.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *EVENTS[] = {
    "FudgeFired", "FudgeHit", "FudgeSplat", "Frozen", "Fart", "FartHit", "Jingle", "Empty", "Volley",
    "SprinkleHit", "Grabbed", "PulledOut", "Spotted", "Lost", "Key", "Pickup", "Hid", "Unhid", "DoorLocked",
    "DoorGuarded", "Jumped", "Escaped", "Scooped", "Swing", "ConeFired", "SugarFired", "SugarBurst", "Staff",
    "FriendsCame", "ManHit", "Melted", "MinionCame", "MinionHit", "MinionMelted", "MinionBite", "Earned",
    "WeaponDropped", "ConeDropped", "ConeEaten", "BoostOver", "Button", "PuzzleWrong", "PuzzleSolved",
    "CabinetLocked", "FreeRoam"};
static const char *TOOL_NAMES[] = {"Fudge", "Fart", "Keys", "Sword", "Spear", "Pistol", "Sugar", "Staff"};
static const char *ENDS[] = {"playing", "escaped", "melted", "scooped"};

static uint32_t bits(float f) { uint32_t b; memcpy(&b, &f, 4); return b; }
static float from_bits(const char *s) {
  uint32_t b = strcmp(s, "0") == 0 ? 0 : (uint32_t)strtoul(s, NULL, 16);
  float f;
  memcpy(&f, &b, 4);
  return f;
}

// The step's events as the trace format writes them.
static void events_text(const coax_t_Game *g, char *out, size_t size) {
  size_t at = 0;
  out[0] = 0;
  for (int64_t i = 0; i < g->event_n; i++) {
    const coax_t_Event *e = &g->events.e[i];
    int k = (int)e->kind;
    char one[128];
    int n = snprintf(one, sizeof one, "%s%s", i ? " " : "", EVENTS[k]);
    const char *name = EVENTS[k];
    if (!strcmp(name, "FudgeSplat") || !strcmp(name, "Fart") || !strcmp(name, "SugarBurst") || !strcmp(name, "MinionCame") ||
        !strcmp(name, "MinionHit") || !strcmp(name, "MinionMelted") || !strcmp(name, "ConeDropped")) {
      n += snprintf(one + n, sizeof one - n, ":%08x,%08x", bits(e->a.x), bits(e->a.z));
    } else if (!strcmp(name, "Staff")) {
      n += snprintf(one + n, sizeof one - n, ":%08x,%08x:%08x,%08x", bits(e->a.x), bits(e->a.z), bits(e->b.x), bits(e->b.z));
    } else if (!strcmp(name, "Empty") || !strcmp(name, "Swing") || !strcmp(name, "Earned")) {
      n += snprintf(one + n, sizeof one - n, ":%s", TOOL_NAMES[e->n]);
    } else if (!strcmp(name, "WeaponDropped")) {
      n += snprintf(one + n, sizeof one - n, ":%s:%08x,%08x", TOOL_NAMES[e->n], bits(e->a.x), bits(e->a.z));
    } else if (!strcmp(name, "Pickup")) {
      n += snprintf(one + n, sizeof one - n, ":%s", e->n == 0 ? "Fudge" : "Fart");
    } else if (!strcmp(name, "Key") || !strcmp(name, "FriendsCame") || !strcmp(name, "Button")) {
      n += snprintf(one + n, sizeof one - n, ":%u", e->n);
    }
    if (at + (size_t)n + 1 >= size) break;
    memcpy(out + at, one, (size_t)n + 1);
    at += (size_t)n;
  }
}

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }

// Replays one trace; returns the number of steps, or -1 at the first difference.
static long replay(const char *path, coax_v_t_Game g, coax_v_t_Bot b, int *ending) {
  FILE *f = fopen(path, "r");
  if (!f) { printf("FAIL missing trace %s\n", path); return -1; }
  char line[8192], name[256] = "", driver[64] = "", diff[64] = "normal";
  unsigned long long seed = 0;
  int man = 1, arm = 0, header = 1, lineno = 0, bot = 0;
  coax_t_Tuning t = icecream_tuning(1);
  long steps = 0;
  char *want_events = malloc(65536), *have = malloc(65536);
  while (fgets(line, sizeof line, f)) {
    lineno++;
    line[strcspn(line, "\n")] = 0;
    if (header) {
      char word[64], a[256];
      double v;
      if (sscanf(line, "%63s", word) != 1) continue;
      if (!strcmp(word, "icecream-trace")) continue;
      if (!strcmp(word, "name")) { sscanf(line, "name %255s", name); continue; }
      if (!strcmp(word, "seed")) { sscanf(line, "seed %llu", &seed); continue; }
      if (!strcmp(word, "difficulty")) {
        sscanf(line, "difficulty %63s", diff);
        t = icecream_tuning(!strcmp(diff, "easy") ? 0 : !strcmp(diff, "hard") ? 2 : !strcmp(diff, "original") ? 3 : 1);
        continue;
      }
      if (!strcmp(word, "set")) {
        sscanf(line, "set %255s %lf", a, &v);
        coax_t_TuningSet r = icecream_tuning_set(t, str(a), v);
        if (!r.ok) { printf("FAIL %s: set %s refused\n", path, a); fclose(f); return -1; }
        t = r.tuning;
        continue;
      }
      if (!strcmp(word, "man")) { sscanf(line, "man %d", &man); continue; }
      if (!strcmp(word, "arm")) { arm = 1; continue; }
      if (!strcmp(word, "driver")) {
        sscanf(line, "driver %63s", driver);
        header = 0;
        bot = !strcmp(driver, "bot");
        icecream_new(g, seed, man != 0, t);
        if (arm) icecream_arm(g);
        icecream_bot_reset(b);
        continue;
      }
      printf("FAIL %s:%d: unknown header %s\n", path, lineno, word);
      fclose(f);
      return -1;
    }
    if (!strcmp(line, "keep")) {
      if (!icecream_keep_playing(g)) { printf("FAIL %s:%d: keep playing refused\n", path, lineno); fclose(f); return -1; }
      bot = 0;  // the fuzz driver plays free roam
      continue;
    }
    char wx[32], wz[32], flags[16], ax[32], az[32], sel[8], fp[32];
    int used = 0;
    if (sscanf(line, "%31s %31s %15s %31s %31s %7s %31s%n", wx, wz, flags, ax, az, sel, fp, &used) != 7) {
      printf("FAIL %s:%d: bad step\n", path, lineno);
      fclose(f);
      return -1;
    }
    const char *rest = line + used;
    while (*rest == ' ') rest++;
    snprintf(want_events, 65536, "%s", rest);
    coax_t_Input in;
    memset(&in, 0, sizeof in);
    in.walk.x = from_bits(wx);
    in.walk.z = from_bits(wz);
    in.sprint = strchr(flags, 's') != NULL;
    in.jump = strchr(flags, 'j') != NULL;
    in.fire = strchr(flags, 'f') != NULL;
    in.interact = strchr(flags, 'i') != NULL;
    in.aim.x = from_bits(ax);
    in.aim.z = from_bits(az);
    in.select = strcmp(sel, "-") == 0 ? -1 : (int8_t)atoi(sel);
    if (bot) {
      coax_t_Input mine = icecream_bot(g, b);
      if (!icecream_same_input(mine, in)) {
        printf("FAIL %s:%d: the bot's input differs (walk %g,%g aim %g,%g fire %d interact %d select %d)\n", path, lineno,
               mine.walk.x, mine.walk.z, mine.aim.x, mine.aim.z, mine.fire, mine.interact, mine.select);
        fclose(f);
        return -1;
      }
    }
    icecream_step(g, in);
    steps++;
    uint64_t want = strtoull(fp, NULL, 16), got = icecream_fingerprint(g);
    events_text(g.data, have, 65536);
    if (want != got || strcmp(have, want_events)) {
      const coax_t_Game *s = g.data;
      printf("FAIL %s:%d: step %llu differs\n  rust: %016llx %s\n  coax: %016llx %s\n", path, lineno,
             (unsigned long long)s->tick, (unsigned long long)want, want_events, (unsigned long long)got, have);
      printf("  player %g,%g h %g hp %g  man %g,%g state %d timer %g  minions %lld shots %lld drops %lld\n", s->player.pos.x,
             s->player.pos.z, s->player.height, s->player.health, s->man.pos.x, s->man.pos.z, (int)s->man.state, s->man.timer,
             (long long)s->minion_n, (long long)s->shot_n, (long long)s->drop_n);
      fclose(f);
      return -1;
    }
  }
  fclose(f);
  free(want_events);
  free(have);
  *ending = (int)g.data->status;
  return steps;
}

// The bot plays a run for up to `seconds`; returns the game's status.
static int play(coax_v_t_Game g, coax_v_t_Bot b, uint64_t seed, int man, int seconds) {
  icecream_new(g, seed, man != 0, icecream_tuning(1));
  icecream_bot_reset(b);
  for (int i = 0; i < seconds * 30 && g.data->status == 0; i++) icecream_step(g, icecream_bot(g, b));
  return (int)g.data->status;
}

// Ports of the Rust crate's own checks: every house of 40 seeds at each
// difficulty, and the bot escapes.
static int checks(coax_v_t_Game g, coax_v_t_Bot b) {
  int bad = 0;
  for (uint64_t seed = 0; seed < 40; seed++)
    for (int d = 0; d < 3; d++)
      if (!icecream_check_house(g, d, seed)) { printf("FAIL house seed %llu difficulty %d\n", (unsigned long long)seed, d); bad++; }
  for (uint64_t seed = 0; seed < 20; seed++)
    if (play(g, b, seed, 0, 400) != 1) { printf("FAIL the bot alone does not escape seed %llu\n", (unsigned long long)seed); bad++; }
  if (play(g, b, 7, 1, 600) != 1 || g.data->tick >= 200 * 30) { printf("FAIL the bot does not escape him on seed 7 in 200 s\n"); bad++; }
  play(g, b, 11, 1, 90);
  uint64_t a = icecream_fingerprint(g);
  play(g, b, 11, 1, 90);
  uint64_t again = icecream_fingerprint(g);
  play(g, b, 12, 1, 90);
  uint64_t other = icecream_fingerprint(g);
  if (a != again || a == other) { printf("FAIL the same seed does not give the same run\n"); bad++; }
  printf("houses (120) and bot runs (22) %s\n", bad ? "FAILED" : "passed");
  return bad;
}

int main(int argc, char **argv) {
  if (argc < 2) { fprintf(stderr, "usage: replay FILE.trace... | --checks\n"); return 2; }
  coax_v_t_Game g = {calloc(1, sizeof(coax_t_Game)), 1};
  coax_v_t_Bot b = {calloc(1, sizeof(coax_t_Bot)), 1};
  if (!strcmp(argv[1], "--checks")) {
    int bad = checks(g, b);
    free(g.data);
    free(b.data);
    return bad ? 1 : 0;
  }
  int bad = 0;
  long total = 0;
  for (int i = 1; i < argc; i++) {
    int ending = 0;
    long n = replay(argv[i], g, b, &ending);
    if (n < 0) { bad++; continue; }
    total += n;
    const char *base = strrchr(argv[i], '/');
    printf("  %-18s %6ld steps, %s, matches the Rust build\n", base ? base + 1 : argv[i], n, ENDS[ending]);
  }
  printf("%ld steps replayed, %d trace(s) differ\n", total, bad);
  free(g.data);
  free(b.data);
  return bad ? 1 : 0;
}
