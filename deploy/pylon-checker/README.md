# Pylon checker

The continuous decision checker (`openagents pylon check decide --watch`,
issue #12191). It reads the relay's beacons and checks every online pylon
that serves the same pinned Clef model as its reference server: once when
the pylon first appears, then every 15 minutes, and a struck pylon again
after an hour. Each round publishes the round's receipt and a NIP-32
verdict (`check-pass`, `check-fail`, `check-inconclusive`) signed with the
checker key; the gateway admits and re-admits contributor pylons from
those verdicts.

Not deployed. This directory holds the unit and these steps; nothing
installs it automatically.

## Where it runs

Next to a reference Clef server running the same pinned artifact as the
pylons it checks, on loopback (`http://127.0.0.1:18096`). The decision GPUs
(`deploy/clef-decisions/`) already run one; any machine where
`openagents pylon start` serves Clef-Flash works too. The reference must
not be a pylon that takes outside jobs: its answers are the yardstick.

The reference server's model must match the pylons' beacons exactly
(`clef-flash@sha256:…`); pylons serving another artifact are skipped.

## Install (owner or operator, by hand)

```sh
# 1. The CLI on the host.
curl -fsSL https://openagents.com/cli/install.sh | bash # or build `openagents` from main

# 2. The checker's keys, made once in their own home.
OPENAGENTS_PYLON_HOME=~/.openagents/pylon-checker openagents pylon whoami

# 3. Optional settings.
mkdir -p ~/.config/openagents
cat > ~/.config/openagents/pylon-checker.env <<ENV
PYLON_CHECKER_BIN=$(command -v openagents)
PYLON_CHECKER_REFERENCE=http://127.0.0.1:18096
PYLON_CHECKER_POOL=everglade
PYLON_CHECKER_EVERY=900
ENV

# 4. The unit.
install -m 0644 deploy/pylon-checker/openagents-pylon-checker.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now openagents-pylon-checker.service
loginctl enable-linger "$USER"     # keep it running after logout
journalctl --user -u openagents-pylon-checker -f
```

Readers trust a checker only when they know its key: add the checker
npub (step 2 prints it) to the gateway's and the aggregator's
`OPENAGENTS_PYLON_CHECKERS`.

## What a pylon sees

- **Private items.** Items come from `checker-seed` in the checker's home,
  mixed with a round counter (`checker-round`) that survives restarts. The
  public item generator alone cannot rebuild them. Keep `checker-seed`
  secret and backed up with the keys; a new seed only means new items.
- **A new buyer key every round.** The jobs and the round's receipt come
  from a key made for that round, so a pylon cannot pick out the checker.
  A pylon that admits only allowlisted keys therefore refuses checker
  rounds and stays unadmitted. That is intended: contributor pylons in the
  community pool run with `--allow-any`. To check an allowlisted pylon by
  hand, use `check decide --pylon NPUB --own-buyer`.
- **Offline or draining pylons are left alone** and checked when they are
  back online.

## Cost

A round sends eight decision jobs to the pylon and recomputes up to eight
on the reference (about 0.2 s each on a Mac, less on a GPU). At the
default interval that is 32 rounds a day per pylon.
