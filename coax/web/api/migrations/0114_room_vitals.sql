-- What an Everglade player brings back into a room (#12715, epic #12718; spec section 7): their
-- HP, their WANTED level and their character (look, class), kept by who they are, the account
-- when logged in (so another device gets it) else the player.
--   - hp, hp_max: written by oa-room (POST /api/playtest/room-vitals, signed with the seat-ticket
--     key), when a seat leaves and while it plays; a ticket carries hp back for 10 minutes.
--   - wanted, look, class: written by the page (POST /api/playtest/room-state, the player key);
--     wanted comes back for 5 minutes, the look and class always.
-- Times are wall-clock milliseconds. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_vitals (
  who text PRIMARY KEY,
  hp int, hp_max int, hp_at bigint,
  wanted int, wanted_at bigint,
  look text, class text,
  at bigint NOT NULL
);
