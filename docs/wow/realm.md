# Private WoW gym realm

The initial realm runs on `coderos-4080` (`100.74.238.61`) over Tailscale.
Its private root is `~/wow-gym`; it installs no system services and does not
change Coder or the owner's game client. The [integration proposal](2026-10-03-benilla-gym-integration.md)
explains the agent architecture.

## Build and extract

Run these commands on the Linux host from this repository. The scripts pin
vmangos to `0e3ff01e76d4758e8a7c3108b2717cc785ed56fa`, Nixpkgs to the
repository's CoderOS revision, and database archive `db-4641790.zip` by SHA-256.

```sh
export WOW_GYM_ROOT="$HOME/wow-gym"
scripts/wow/realm.sh build
```

Copy your private 1.12.1 build 5875 English client `Data` folder into
`$WOW_GYM_ROOT/client/Data`. On the Mac, the verified installation is
`~/work/Stonetavern-Classic-1.12.1-v1.8`. Copy only its data; its Windows loaders
and client modifications do not participate in the headless agent.

```sh
rsync -a ~/work/Stonetavern-Classic-1.12.1-v1.8/Data coderos-4080:wow-gym/client/
```

Check free space before extraction. Allow space for the client, build, database,
vmaps, mmaps, and backups; mmap generation can take hours. Native Linux
extractors need no Wine.

```sh
scripts/wow/realm.sh extract > "$WOW_GYM_ROOT/logs/extract.log" 2>&1
nix-shell "$WOW_GYM_ROOT/shell.nix" --run 'python3 scripts/wow/initialize.py'
scripts/wow/realm.sh start
scripts/wow/realm.sh status
```

Initialization refuses an existing database. It creates 20 ordinary accounts
and a separate `GYMSETUP` GM account. Credentials are generated into
`accounts.json`, with private permissions; realm configuration also contains
private database credentials. Never commit these files, MPQs, extracted data,
or SQL backups.

## Connect and stop

Authentication listens on the host's Tailscale address at port 3724; the realm
advertises that same address at port 8085. The database listens only on loopback
at port 13306. Warden, the interactive server console, remote administration, and SOAP are disabled. Check
Tailscale ACLs and the host firewall if another tailnet device cannot connect;
do not expose these ports publicly.

Use the gym bridge with credentials read from a private file. For visual
spectating, point benilla's `WOW_DATA` at your own client `Data` directory and
connect to the private realm. Do not edit your normal client's realm settings
for a smoke test.

```sh
scripts/wow/realm.sh stop
```

The stop command sends SIGTERM for a clean server save. Wait for the processes
to exit before restarting. Inspect `logs/mangosd.log`, `logs/realmd.log`, and
`logs/mysql.log` when startup fails. Repeating `start` skips running realm
processes; it does not recreate accounts or overwrite the database.

## Backup and recover

```sh
scripts/wow/realm.sh backup
```

Backups live under `backups/` with private permissions. To restore, stop the
realm processes, keep the private database running, and feed the selected SQL
backup to the private socket with `mariadb --no-defaults --socket=... -u "$USER"`.
Keep an untouched backup before a restore. Rebuild binaries from the pinned
source and rerun extraction only when changing the extractor/data revision.
Account recreation and episode reset must use the gym account pool; never
reset spectator or unrelated player accounts.

## Verification on October 3, 2026

The native build and all map/vmap extraction passed. DBC extraction produced
158 tables; map extraction produced 2,429 terrain tiles. The Northshire mmap
(`0004832.mmtile`) was generated, with full-realm navigation generation running
under `logs/mmaps.log`. Authentication and world entry from the Mac over
Tailscale passed: a disposable Human Warrior entered map 0 and decoded 51
entities. The character was deleted through the character-select protocol.
A private SQL backup passed `gzip -t`, and the database and realm restarted.
The setup account has a realm-scoped `account_access` grant; ordinary gym
accounts have none.
