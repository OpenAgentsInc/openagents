# Parallel WoW episodes

The initial measured limit is two concurrent gym episodes across all helper
hosts. `worlds/northshire.json` declares `GYM1` through `GYM20`, but the extra
accounts do not raise the concurrency limit. Every episode holds a capacity
slot, an account lease, and a character-name lease on the realm host. An SSH
stream holds `flock` in `~/wow-gym/leases`; closing the stream releases the lock.
Local helpers on that same host use the same kernel locks directly.

Build Voyager and the helper, provide the private credentials, then run:

```sh
export VOYAGER_WOW_ACCOUNTS="$HOME/wow-gym/accounts.json"
export VOYAGER_WOW_BRIDGE="$HOME/work/openagents-target-agent1/release/wow-bridge"
voyager run --world northshire-pool --episodes 4 --parallel 2
```

`northshire-pool` repeats the delivery quest, which has no shared kill target.
The pool queues its four episodes onto two workers and writes
`pool-metrics.json`. Each episode retains its own trace and quest metrics. The
full `northshire` curriculum remains available for solo combat measurements.

On CoderOS, use `VOYAGER_WOW_LEASE_LOCAL=1`. Its default lease directory is
`$HOME/wow-gym/leases`, the same directory Mac helpers reach through the
manifest's `wow.lease_host` (`coderos-4080`). `VOYAGER_WOW_LEASE_DIR` can override
that local directory for an isolated test realm; every host using one realm
must coordinate through the same directory. SSH must work noninteractively. `realm.sh` installs `lease.sh` under the private
realm root. For a Boat-only SSH key, use an `authorized_keys` forced command
`restrict,command="env WOW_LEASE_ACCOUNTS=GYM1,GYM2 /home/christopherdavid/wow-gym/lease.sh --ssh"`
and assign only those accounts in its manifest. The forced command accepts
lease requests only and never evaluates client shell text.
A coordinator outage refuses new work. Losing a held lease retires an active
helper within its 250 ms authority polling interval after SSH reports failure;
SSH keepalives bound transport-loss detection to about ten seconds.

A character's name is the template's first ten letters plus a stable two-letter
suffix for its account. Only canonical pool accounts `GYM1` through `GYM20` are
admitted. Reset deletes and recreates that character once, then waits for the
roster to confirm the mutation before proceeding. Cleanup confirms deletion.
After a crash, the kernel releases leases; the next admitted episode recreates
and verifies the character before giving programs control. If the server still
considers the character online, reset fails safely; retry after logout settles.
Do not unlink lock files, because an existing holder locks the old inode.

Trusted setup holds the separate `GYMSETUP` account lease. Its realm grant is
level 4 (`SEC_BASIC_ADMIN`), the minimum the pinned server requires for quest
seeding. Ordinary accounts have no grant. WoW skill stores are
partitioned by world digest and account, so simultaneous episodes cannot race
on a skill index. Episode artifacts remain separate even when the manifest and
account are reused. Credentials stay in private files; never bake them into
host images, Boat templates, world files, or traces.

## Placement and isolation

Linux pool hosts need Rust 1.97.1 for Voyager and 1.98.1 for `wow-bridge`, the
private credential file, and access to the realm's advertised authentication
and world endpoints (TCP 3724 and 8085), plus SSH to the lease coordinator.
There is no renderer or Wine dependency. Realm data stays on CoderOS.

A Boat sandbox needs that same private route and coordinator access before it
can run an episode. Use the repository's [Boat template procedure](../deployment/boat-template.md),
then enroll the sandbox into the tailnet with an ephemeral node credential and
supply scoped SSH access and a private account file at runtime. Test both realm
ports and a coordinator lease from inside the sandbox. Do not publish the realm
or assume a hosted HTTPS port forwards this raw protocol. If the sandbox cannot
run a Tailscale tunnel, use an approved private network gateway instead.
Boat tailnet enrollment remains an owner provisioning step; no credentials or
public realm endpoint were created for it.

The pinned vanilla realm provides shared overworld maps and dungeon instances.
Northshire is shared: separate accounts isolate inventory, quest progress, and
character resets, but they share creature spawns and loot contention. This
integration does not implement modern phasing or assign private overworld
instances. Keep simultaneous combat curricula in disjoint routes or zones;
measure contention before raising capacity toward 20–50 agents. The two-worker
delivery benchmark does not establish a safe 20-player combat limit.

For private spectating, point the owner's 1.12.1 client's `realmlist.wtf` at the
realm's Tailscale address, or run benilla with `WOW_DATA` pointing at the owner's
private `Data` directory and that realm. Use a separate spectator account and
character. Never give Voyager that account or include game assets in artifacts.
