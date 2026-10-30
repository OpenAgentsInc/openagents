#!/usr/bin/env bash
# Dead Gears' Coax checks at the step budgets they need: the rules
# (verse/deadgears/rules.coax, with ecs.coax and arena.coax) and the page
# (apps/openagents-web/deadgears.coax). Each #test has its own budget; the page's
# full games play 6000 frames on one state (~2.8e9 steps each, measured
# 2026-10-29), over the 2e9 other realms' checks use, so both run at 4e9.
#   verse/deadgears/test.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
coax/bin/coax test verse/deadgears/rules.coax --library --steps 4000000000
coax/bin/coax test apps/openagents-web/deadgears.coax --steps 4000000000
