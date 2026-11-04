# External navigation v2

Identity: `wow-coldridge-waypoint-v2`, selected with `--health-guard`.
The independent combat strip supplies health from the same captured frame;
its serial must match the navigation strip. V1 stays unchanged.

An out-of-combat health decrease after a bounded forward pulse cancels the
waypoint decision. With an unchanged heading, a fresh living/UI-clear sample
permits one 500 ms reverse pulse. This attempts to leave an environmental
hazard along the preceding heading. It is a
local deterministic override; Elephant does not authorize it. The loop records
the reason, exact broker exchanges, fresh health and a terminal outcome.
Other invalid states stop without retreat. No new forward input is admitted
after health loss. An initial health below 85% refuses navigation.

This is not hazard recognition, proof of a safe reverse path, a combat escape
or a guarantee of survival. The return observation remains required; callers
must inspect a terminal failure promptly. Records preserve full RGB captures.
