// Who else is in the realm, and its chat (room.coax, pt-presence-chat),
// included by realm-face.c. Every ROOM_EVERY seconds the page posts where you
// are to /api/playtest/presence/<room> with the player key and takes the
// answer: everyone else here and the lines said since. A line typed in the
// chat goes to /api/playtest/chat/<room>. A browser without a player key yet
// is given one (POST /api/playtest/player), kept as the host's player.
#define PLAYER_URL "/api/playtest/player"
#define ROOM_WAIT 10.0
#define ROOM_EVERY 1.0
static coax_t_RoomState room;
static int32_t room_ask = -1, say_ask = -1, mint_ask = -1;
static double room_at = -10, room_asked_at;
static char room_url_buf[120], say_url_buf[120], mint_record[160];
static uint8_t room_body[256], say_body[1200];
static uint32_t mint_n;
static bool minted;
static int64_t room_here_was = -1;
static bool room_full_was;

static coax_v_t_RoomState room_of(void) { return (coax_v_t_RoomState){&room, 1}; }
static coax_string room_name(void) { return world_holds() ? world_name() : text("haunted-house"); }

static void room_mint(void) {
  if (minted || mint_ask >= 0) return;
  minted = true;
  mint_ask = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
}

static void room_frame(float dt) {
  room_ease(room_of(), dt);
  if (realm.chat) room.unread = 0;
  if (mint_n && !realm.action) {
    memcpy(realm.outbox.e, mint_record, mint_n);
    realm.out_n = mint_n; realm.action = 5; mint_n = 0;
    has_player = true; room_at = now;
  }
  if (!facts_seen) return;
  if (!has_player) { room_mint(); return; }
  if (room_ask >= 0 && now - room_asked_at < ROOM_WAIT) return;
  if (now - room_at < ROOM_EVERY) return;
  int64_t un = room_url(room_name(), false, (coax_v_u8){(uint8_t *)room_url_buf, sizeof room_url_buf});
  coax_t_Vec3 feet = verse_player_feet(player);
  int64_t bn = room_presence_body(feet.x, feet.y, feet.z, player.theta, room.last, (coax_v_u8){room_body, sizeof room_body});
  room_at = now;
  if (un <= 0 || bn <= 0) return;
  room_ask = coaxial_http((uint32_t)(uintptr_t)room_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)room_body, (uint32_t)bn, 1 | 4 | 8);
  room_asked_at = now;
}

// Who is here, recorded when you come in and whenever the number changes.
static void room_seen(void) {
  if (!room.joined) return;
  int64_t here = room.full ? 0 : room.others + 1;
  if (here == room_here_was && room.full == room_full_was) return;
  room_here_was = here; room_full_was = room.full;
  realm_track_presence(tr(), room_name(), here, room.full);
}

static void room_say(coax_string typed) {
  if (!has_player || say_ask >= 0) { toast("Wait a moment, then say it again"); return; }
  int64_t un = room_url(room_name(), true, (coax_v_u8){(uint8_t *)say_url_buf, sizeof say_url_buf});
  coax_string sid = {(const char *)track_state.sid.e, track_state.sid_n};
  int64_t bn = room_chat_body(typed, sid, (coax_v_u8){say_body, sizeof say_body});
  if (un <= 0 || bn <= 0) return;
  say_ask = coaxial_http((uint32_t)(uintptr_t)say_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)say_body, (uint32_t)bn, 1 | 4 | 8);
  realm_track_chat(tr(), room_name(), typed);
}

static int room_status(uint32_t length) {
  int status = 0;
  for (uint32_t i = 0; i < length && inbox[i] >= '0' && inbox[i] <= '9'; i++) status = status * 10 + (inbox[i] - '0');
  return status;
}

static bool room_message(int32_t kind, uint32_t length) {
  if (room_ask >= 0 && kind == 512 + room_ask) {
    room_ask = -1;
    int64_t status = room_take(room_of(), (coax_string){(const char *)inbox, length});
    if (status == 403 && !minted) { has_player = false; room_mint(); }
    room_seen();
    return true;
  }
  if (say_ask >= 0 && kind == 512 + say_ask) {
    say_ask = -1;
    int status = room_status(length);
    if (status == 429) toast("A little slower: wait a moment");
    else if (status != 200) toast("That didn't reach the others");
    room_at = -10;
    return true;
  }
  if (mint_ask >= 0 && kind == 512 + mint_ask) {
    mint_ask = -1;
    const char *nl = memchr(inbox, '\n', length);
    uint32_t at = nl ? (uint32_t)(nl - (const char *)inbox) + 1 : length;
    if (room_status(length) == 200 && length - at > 0 && length - at <= sizeof mint_record) {
      memcpy(mint_record, inbox + at, length - at);
      mint_n = length - at;
    }
    return true;
  }
  return false;
}

static void room_describe(coax_t_RealmView *v) {
  coax_t_Vec3 eye = verse_player_view_eye(player, house.camera), at = verse_player_view_at(player, house.camera);
  v->has_view = verse_grid_view(eye.x, eye.y, eye.z, at.x, at.y, at.z, width, height, (coax_v_f32){v->camera.e, 16});
  v->others = room.others;
  for (int64_t i = 0; i < room.others; i++) {
    v->other_name.e[i] = room_other_name(room_of(), i);
    for (int c = 0; c < 4; c++) v->other_at.e[i * 4 + c] = room.at.e[i * 4 + c];
  }
  v->here = room.joined && !room.full ? room.others + 1 : 0;
  v->room_full = room.full;
  v->unread = room.unread;
  v->lines = room.lines;
  for (int64_t k = 0; k < room.lines; k++) {
    v->line_name.e[k] = room_line_name(room_of(), k);
    v->line_text.e[k] = room_line_text(room_of(), k);
    v->line_mine.e[k] = room_line_mine(room_of(), k);
  }
}
