#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: the one list of the API binary's C files and Coax units, and the coax c call that makes each unit's C)
# The one list of what the API binary is built from (GitHub #13380, plans/006). Sourced, never run,
# by test.sh, realm-gate.sh, mesh-test.sh and eval-builder.sh, so a unit added here reaches all of
# them. The production image keeps its own list (Dockerfile, with deploy.sh making the Coax units'
# C); test.sh fails when either names a different set from this one.
#   api_sources DIR WORK COAX   DIR is coax/web/api. Makes each Coax unit's C and header in WORK
#                               and sets the array API_SOURCES to every C file of the binary.
#                               Compile with -I"$WORK".
# A new C file goes in API_C; a new Coax unit goes in API_COAX as SOURCE:NAME (SOURCE from DIR, or
# from the repository root when it starts with pay/ or metaverse/), which makes WORK/NAME.generated.{c,h}. A unit
# whose C is wrapped by a NAME-unit.c (it includes the generated C under names of its own) is
# listed in API_COAX_WRAPPED instead, so its generated C is made but not linked twice.
API_C="api.c analytics.c admin.c player.c rooms.c npc.c room.c private.c drops.c villa.c chat.c chat-history.c account-chat.c account-chat-wire-unit.c realms.c
  access.c makers.c inference.c mesh.c mesh-gemini-unit.c mesh-policy.generated.c caller-policy.generated.c nip98.generated.c
  anime.c seats.c nostr.c email.c jobs.c relay.c verse_packs.c worldbuilder.c auth.c auth-seal-unit.c
  lnaddr.c lnaddr-unit.c agents.c agent-wire-unit.c agent-attest-unit.c auth-recover.c auth-life.c
  auth-recover-crypto-unit.c auth-device.c auth-device-crypto-unit.c push.c passkeys.c passkey-verify-unit.c
  wallet.c wallet-vault-unit.c ticket-sign-unit.c service.c worlds.c splice-world-unit.c scripts.c script-job-run.c script-store-unit.c"
API_COAX="npc-personas anime-director-policy seat-policy progress-policy item-policy moderation-policy
  character-policy worldbuilder-policy auth-policy chat-history-wire auth-recover-policy auth-life-policy
  auth-device-policy passkey-policy pay/wallet-policy:wallet-policy service-policy service-wire realm-wire auth-push seat-wire jobs-wire email-wire packs-wire world-policy error-wire mesh-wire
  auth-wire auth-recover-wire auth-life-wire auth-device-wire passkeys-wire chat-wire player-wire rooms-wire lnaddr-wire makers-wire worldbuilder-wire"
API_COAX_WRAPPED="account-chat-wire mesh-gemini auth-seal pay/lnaddr-policy:lnaddr-policy agent-wire agent-attest auth-recover-crypto
  auth-device-crypto passkey-verify pay/wallet-vault-store:wallet-vault ticket-sign
  metaverse/splice-bots/world-api:splice-world script-store"
api_sources() {
  local a="$1" work="$2" coax="$3" root u src name
  root="$(cd "$a/../../.." && pwd)"
  API_SOURCES=()
  for u in $API_C; do API_SOURCES+=("$a/$u"); done
  for u in $API_COAX $API_COAX_WRAPPED; do
    src="${u%%:*}"; name="${u##*:}"
    if [[ "$src" == pay/* || "$src" == metaverse/* ]]; then src="$root/$src.coax"; else src="$a/$src.coax"; fi
    "$coax" c "$src" --run-steps 250000000 --library --header "$work/$name.generated.h" > "$work/$name.generated.c"
    [[ " $(echo $API_COAX) " == *" $u "* ]] && API_SOURCES+=("$work/$name.generated.c")
  done
  return 0
}
