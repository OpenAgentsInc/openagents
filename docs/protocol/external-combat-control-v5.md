# External combat control v5

Identity: `starter-warlock-v5`, selected with `--efficient`. Reuses V4's
combat, corpse ownership, loot and lifetime admission. Visual survey scans
use bounded 300 ms right turns and move to a circuit leg after seven scans;
the next scan request begins travel. Adjacent views overlap at the qualified
client's observed turn rate/FOV. This is not obstacle inference.

Observation-only waits follow an already observed cast for at most 1500 ms,
or wait 2000 ms for an OOC resource tick when no target is selected. Work
already elapsed consumes that interval. Input actions still settle for
500 ms and every non-wait action still receives a new capture/recheck.
Loot observes at the existing 500 ms cadence. This reduces redundant lossless
captures without holding a key during waiting, encoding or inference.

After at least three kills, measured throughput below 120 XP/minute also
suggests another area. This is a recorded heuristic, not a leveling optimum.
V3 compact evidence is independently selected with `--compact`; V2 full-scene
lossless evidence remains the default. Earlier profile identities stay intact.

An explicit `--park-x X --park-y Y` declares a previously scouted clear point
within the survey fence. After draining a normal completion/budget it clears
the selected target, rechecks and follows the Aethyr waypoint adapter there
before exiting. It can still defend against an eligible attacker encountered
on the way. The normal blocked-leg recovery remains bounded; the waypoint
does not prove obstacle-free or permanently safe terrain. Hard stops, changed
windows, owner input and unavailable/menu/dead state still stop immediately.
Parking is only enabled with `--efficient`; absent coordinates preserve V4.

The recorder samples filesystem availability every sixteen observations.
At 768 MiB remaining, or an unreadable availability value, it requests drain
and parking rather than beginning another pull. This is an operating reserve,
not a guarantee against concurrent writers exhausting the filesystem.

An unchanged selected creature, new UI error, no active cast and no combat
for at least five seconds identifies a failed pull attempt, not a fight.
V5 abandons that attempt, clears the selected target, and uses one bounded
back-step plus three bounded right-side steps before reacquiring visually.
It never applies this recovery while casting, in combat, after damage, or
after a creature-lifetime change. Real fights retain their original deadlines.

After a verified kill, a healthy combat/cast tail gets up to 1500 ms to
settle. Continued combat/casting, or health below 35%, records `loot-deferred`
and abandons that pending corpse so defense/recovery can proceed. This is
neither a receipt nor a promise to return to the corpse. In combat, only a
selected dead creature matching the last own kill may be cleared; its exact
NPC, spawn and target epoch must match a fresh recheck.

V5 permits recovery below the legacy 35% health floor while out of combat,
and defense against an eligible creature whose public target is the player
while already in combat. The normal 85% health / 60% mana new-pull thresholds
remain. Legacy profiles retain their health-floor behavior. This does not
guarantee survival against multiple attackers or provide emergency escape.
