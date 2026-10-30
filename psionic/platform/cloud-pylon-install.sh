#!/usr/bin/env bash
# Install an already-built, source-recorded native Pylon. No model or protocol
# logic lives here: this is systemd/filesystem setup for the Coax executable.
# A Nix-produced Linux runtime closure keeps OpenSSL's existing provenance.
set -euo pipefail
if [[ $# != 8 ]]; then
    echo 'usage: sudo cloud-pylon-install.sh ARTIFACTS SLUG MODEL PORT MEMORY_GB IDENTITY REFERENCES SOURCE_REVISION' >&2
    exit 2
fi
artifacts=$1 slug=$2 model=$3 port=$4 memory=$5 identity=$6 references=$7 revision=$8
[[ $EUID == 0 ]] || { echo 'run as root' >&2; exit 2; }
[[ $slug =~ ^[a-z0-9][a-z0-9-]{0,62}$ && $model =~ ^[a-z0-9][a-z0-9.-]{0,62}$ &&
   $port =~ ^[0-9]+$ && $port -ge 1 && $port -le 65535 &&
   $memory =~ ^(8|16|32|64|128|256|512)$ &&
   $revision =~ ^[0-9a-f]{40}$ && $identity =~ ^[a-z0-9.-]+@sha256:[0-9a-f]{64}$ ]] || {
    echo 'invalid deployment arguments' >&2; exit 2;
}
[[ -x $artifacts/pylon && -f $artifacts/nix-runtime.tgz && -f $references ]] || {
    echo 'missing executable, Nix runtime closure, or selfcheck references' >&2; exit 2;
}
(cd "$artifacts" && sha256sum --check SHA256SUMS)
prefix=/opt/openagents/pylon/$revision
state=/var/lib/openagents-pylon/$slug
if ! id openagents-pylon >/dev/null 2>&1; then
    useradd --system --home-dir /var/lib/openagents-pylon --shell /usr/sbin/nologin openagents-pylon
fi
# The deployment builder supplies only the Nix store paths listed in its
# source record. Do not install a package manager or grant the service a shell.
tar -xzf "$artifacts/nix-runtime.tgz" -C /
install -d -m 0755 "$prefix"
install -m 0755 "$artifacts/pylon" "$prefix/pylon.next"
mv -f "$prefix/pylon.next" "$prefix/pylon"
install -m 0644 "$artifacts/SHA256SUMS" "$prefix/SHA256SUMS"
if [[ -f $artifacts/source.json ]]; then install -m 0644 "$artifacts/source.json" "$prefix/source.json"; fi
install -m 0644 "$references" "$prefix/$model.selfcheck"
install -d -m 0700 -o openagents-pylon -g openagents-pylon "$state"
# Never put the label in argv, logs or a committed file. Keep an existing
# identity on reinstall; this remains an explicitly throwaway demo identity
# while the real-key signer audit (#4828) is closed in the application.
if [[ ! -e $state/demo-label ]]; then
    umask 077
    od -An -N32 -tx1 /dev/urandom | tr -d ' \n' > "$state/demo-label"
    chown openagents-pylon:openagents-pylon "$state/demo-label"
fi
chmod 0600 "$state/demo-label"
cat > "/etc/systemd/system/openagents-pylon-$slug.service" <<EOF
[Unit]
Description=OpenAgents Coax Pylon $slug (throwaway demo identity)
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=openagents-pylon
Group=openagents-pylon
WorkingDirectory=$state
Environment=SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ExecStart=$prefix/pylon --relay wss://relay.openagents.com --engine http://127.0.0.1:$port --model $model --slug $slug --label $slug --family gpu --memory-gb $memory --test-key-label-file $state/demo-label --references $prefix/$model.selfcheck --identity-state $state/identity --declare-identity $identity --refresh 60 --ttl 180 --decide-ms 30000
Restart=on-failure
RestartSec=10
TimeoutStopSec=45
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$state
RestrictSUIDSGID=true
MemoryMax=1G
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable "openagents-pylon-$slug.service"
systemctl restart "openagents-pylon-$slug.service"
echo "installed openagents-pylon-$slug; model $identity; source $revision"
