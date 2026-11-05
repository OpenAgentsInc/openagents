# External combat control v6

Identity: `starter-warlock-v6`, selected with `--defensive` together with
`--efficient`. V5 and earlier identities retain their behavior.

The recorded client error 51 says “Target needs to be in front of you.” An
engaged eligible creature publicly targeting the player, no active cast,
an available Shadow Bolt action and a newly observed instance of that error
can trigger one 100 ms left turn. The same failure is consumed once; a further
turn requires another observed error instance. At most twelve turns are allowed
per fight.

Every turn gets a fresh capture and rechecks the same NPC/spawn/target epoch,
unchanged target health, error serial and health scale, living/UI-clear combat
state, target-of-target and spell availability. Target-health progress consumes
the current error without turning. Corrections never reset the damage-progress
clock or the original 45-second fight deadline.

This is a bounded facing search using actual cast feedback, not target-position
inference. It does not guarantee survival, general obstacle clearance, combat
escape or safe handling of a terminal refusal. It never changes targets,
fabricates progress, extends a deadline or authorizes a new pull. The initial
qualification uses recorded facts and an independent SDK peer; live recovery
of this particular encounter remains unqualified.
