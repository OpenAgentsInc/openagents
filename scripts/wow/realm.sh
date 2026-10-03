#!/usr/bin/env bash
# Operate the private WoW gym without installing system services.
set -euo pipefail
umask 077
root="${WOW_GYM_ROOT:-$HOME/wow-gym}"
here="$(cd "$(dirname "$0")" && pwd)"
core_rev=0e3ff01e76d4758e8a7c3108b2717cc785ed56fa
export WOW_GYM_ROOT="$root"
mkdir -p "$root/logs" "$root/tmp"
case "${1:-}" in
build)
  if [ ! -d "$root/core/.git" ]; then git clone https://github.com/vmangos/core.git "$root/core"; fi
  git -C "$root/core" checkout --detach "$core_rev"
  cp "$here/shell.nix" "$root/shell.nix"
  nix-shell "$root/shell.nix" --run 'cmake -S "$WOW_GYM_ROOT/core" -B "$WOW_GYM_ROOT/build" -G Ninja -DBUILD_EXTRACTORS=ON -DSUPPORTED_CLIENT_BUILD=5875 -DENABLE_CPPTRACE=OFF -DDEBUG_SYMBOLS=OFF -DBUILD_FOR_HOST_CPU=OFF -DMYSQL_INCLUDE_DIR="$MYSQL_HOME/include/mysql" -DMYSQL_CONFIG="$MYSQL_HOME/bin/mysql_config" -DCMAKE_INSTALL_PREFIX="$WOW_GYM_ROOT/install" && cmake --build "$WOW_GYM_ROOT/build" -j 8 && cmake --install "$WOW_GYM_ROOT/build"'
  ;;
extract)
  cd "$root/client"
  "$root/install/bin/Extractors/MapExtractor" --silent -i . -o .
  "$root/install/bin/Extractors/VMapExtractor" --silent
  mkdir -p vmaps
  "$root/install/bin/Extractors/VMapAssembler" --silent Buildings vmaps
  mkdir -p mmaps 5875
  if [ -d dbc ]; then
    mkdir -p 5875/dbc
    cp -a dbc/. 5875/dbc/
  fi
  "$root/install/bin/Extractors/MoveMapGenerator" --threads 8 --silent --configInputPath "$root/install/bin/Extractors/config.json" --offMeshInput "$root/install/bin/Extractors/offmesh.txt"
  ;;
start)
  if ! { [ -f "$root/mysql.pid" ] && kill -0 "$(cat "$root/mysql.pid")" 2>/dev/null; }; then
    nix-shell "$root/shell.nix" --run 'nohup mariadbd --no-defaults --datadir="$WOW_GYM_ROOT/mysql" --socket="$WOW_GYM_ROOT/mysql.sock" --pid-file="$WOW_GYM_ROOT/mysql.pid" --bind-address=127.0.0.1 --port=13306 --sql-mode="" --tmpdir="$WOW_GYM_ROOT/tmp" --log-error="$WOW_GYM_ROOT/logs/mysql.log" > /dev/null 2>&1 &'
  fi
  nix-shell "$root/shell.nix" --run 'for i in $(seq 1 30); do mariadb-admin --no-defaults --socket="$WOW_GYM_ROOT/mysql.sock" -u "$(id -un)" ping > /dev/null 2>&1 && exit 0; sleep 1; done; exit 1'
  for binary in realmd mangosd; do
    if [ -f "$root/$binary.pid" ] && kill -0 "$(cat "$root/$binary.pid")" 2>/dev/null; then continue; fi
    (cd "$root"; nohup "$root/install/bin/$binary" -c "$root/$binary.conf" < /dev/null > "$root/logs/$binary.log" 2>&1 & echo $! > "$root/$binary.pid")
  done
  ;;
stop)
  for binary in mangosd realmd; do
    if [ -f "$root/$binary.pid" ]; then kill -TERM "$(cat "$root/$binary.pid")" 2>/dev/null || true; fi
  done
  for i in $(seq 1 30); do
    live=0
    for binary in mangosd realmd; do
      if [ -f "$root/$binary.pid" ] && kill -0 "$(cat "$root/$binary.pid")" 2>/dev/null; then live=1; fi
    done
    [ "$live" -eq 0 ] && break
    sleep 1
  done
  [ "$live" -eq 0 ] || { echo "realm is still saving; database left running" >&2; exit 1; }
  if [ -f "$root/mysql.pid" ]; then
    db_pid="$(cat "$root/mysql.pid")"
    kill -TERM "$db_pid" 2>/dev/null || true
    for i in $(seq 1 30); do kill -0 "$db_pid" 2>/dev/null || break; sleep 1; done
    if kill -0 "$db_pid" 2>/dev/null; then echo "database is still stopping" >&2; exit 1; fi
  fi
  ;;
status)
  for binary in mysql realmd mangosd; do
    if [ -f "$root/$binary.pid" ] && kill -0 "$(cat "$root/$binary.pid")" 2>/dev/null; then echo "$binary running"; else echo "$binary stopped"; fi
  done
  df -h "$root"
  ;;
backup)
  mkdir -p "$root/backups"
  nix-shell "$root/shell.nix" --run 'mariadb-dump --no-defaults --socket="$WOW_GYM_ROOT/mysql.sock" -u "$(id -un)" --single-transaction --databases realmd characters mangos logs | gzip > "$WOW_GYM_ROOT/backups/realm-$(date -u +%Y%m%dT%H%M%SZ).sql.gz"'
  ;;
*) echo "usage: WOW_GYM_ROOT=/private/path $0 build|extract|start|stop|status|backup" >&2; exit 2 ;;
esac
