# Directions from current game facts

[guard-guide-game.coax](../../../verse/everglade/guard-guide-game.coax) is an
independent `#authority` root. Its data-only
[GgpFacts](../../../verse/everglade/guard-guide-game-types.coax) carries the
current followed quest and local eligibility facts. It does not load the GG
implementation, walk quest history, compute geometry or perform effects.

The fixed authored mapping is:

| Current followed quest | Stage 1 GO or 2 TALK | Stage 3 RETURN |
| --- | --- | --- |
| 0, waystone | goal 2, waystone | goal 5, board |
| 1, ruins | goal 3, ruins | goal 5, board |
| 2, river | goal 4, river | goal 5, board |

No followed quest means `followed=-1, stage=0`. It projects goal 0 until a
separate explicit Find Work intent sets `find_work=true`, which projects goal
1. Find Work combined with an active quest refuses rather than overriding that
objective. Unknown indices, stage 0 on a named quest, DONE 9 and other invalid
stages project no goal. The page must use its existing HUD `follow(s)` result.

`ggp_offer` returns 1 for an eligible current unmet route and 2 for an eligible
prospective Find Work action with exactly no followed quest. It ignores the
request flags and never creates an intent. Unknown/edited geometry, arrival,
missing targets and an existing guide hint suppress the offer. The ordinary
quest tracker is not a guide hint. `ggp_project` preserves the actual guide
hint flag; separate session policy handles ownership of its own local tip.

Eligibility requires guard 0, playable foreground gameplay, a living player,
and Bram calm, alive, at full health, visible and near. Combat, talking, board,
editing, casting, map or warp each refuse independently. The caller derives
these facts from game state, including armed spell UI and other active edit UI.

A route also requires `world_known`, present target/destination, no arrival,
and unchanged supported geometry. The actual edit payload is
`EgEdits.n/gone/hidden`: all three must be zero. An editor version is not edit
presence. `altered_geometry` also refuses destruction or live spell walls that
are not represented in EgEdits. The page owns finite position/range checks and
goal-target lookup.

`ggp_goal_key` returns unique tags 17–19, 33–35 and 49–51 for those nine active
quest/stage pairs, 0 for no follow and -1 for invalid input. These are local
semantic context tags, not model inputs. `ggp_next_intent` increments a valid
nonnegative counter, saturates at the session limit 2147483647 and preserves
invalid negatives or already-above-limit values;
an exhausted or repeated serial cannot represent a fresh intent.
`ggp_intent` advances through that helper only for an explicit queued input at
a simulation boundary whose captured epoch still equals the current epoch.
A world/map/lifecycle invalidation between the input edge and frame cannot mint
the queued intent in a different context. Session admission still independently
requires a valid positive epoch and current eligible facts.

The client authentication hint accepts at most 16384 bytes from VIEWER_KIND606
and requires exactly one `H|` line containing 64 lowercase hex characters.
Blank lines and K/J/M/A lines are inert; unknown line types, duplicate H and
malformed H refuse. No key is returned, logged or used to sign. Player/account
facts must not be sent through this helper. The server cookie remains authority.

The world-envelope helper accepts at most 65536 bytes, exact `200\n` and a
parsed JSON object. Its deliberately narrow current GET profile requires unique
canonical raw keys: outer `version`, `doc`, optional `publishedBy` string/null
and `publishedAt` number/null; doc `edits` string and optional numeric
`everglade`. Unknown keys, escaped aliases and nested metadata refuse. Version
is a nonnegative integer in the shared Jv integer domain, with no negative sign,
fraction or exponent. Version 0 requires doc absent/null; a positive version
requires the doc object and edits string. Version/edits separators accept spaces
only, matching the unchanged legacy reader. This prevents that reader's global
first-key scan from choosing nested reserved fields.

The helper does not validate the eg-edit content. The page must combine its
result with a successful `eg_edits_answer` using
`ggp_world_ready(parsed,outer)` before setting world-known. A valid outer object
containing invalid edit text is still refused by that second independent read.

```shell
coax/bin/coax test verse/everglade/guard-guide-game.coax --library
coax/bin/coax mutate verse/everglade/guard-guide-game.coax --root-only --jobs 6
psionic/tests/guard-guide/check-game.sh /tmp/guard-guide-game-native
```

The recorded slice passed 21 checks and killed all 323 root mutants. The
independent emitted-C oracle passed 33187 cases each in debug, release and
ASan/UBSan, including all 32768 eligibility masks. These checks qualify the
projection and ABI; the page, transport and player-visible rendering have
separate integration evidence. No model inference or production call is part
of this runner.
