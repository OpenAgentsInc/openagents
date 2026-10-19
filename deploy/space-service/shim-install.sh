#!/usr/bin/env bash
# Configure an owned, drained worker for the pinned containerd shim.
set -euo pipefail
space_shim=${1:?Provide the verified shim binary.}
[[ $(id -u) == 0 ]] && mountpoint -q /space
case $(hostname -s) in oa-space-service-worker|oa-space-service-builder) ;; *) echo 'Choose an owned SPACE worker.' >&2; exit 1 ;; esac
if systemctl is-active --quiet oa-space-k3s; then
  echo 'Drain and stop the node agent before installing its runtime handler.' >&2
  exit 1
fi
printf '%s  %s\n' c35eb61afc6999c88bd3edafc872f37a2383a966989d6bd0f79b5d8cb8210241 "$space_shim" | sha256sum --check
space_shim_config=/space/k3s/agent/etc/containerd/config-v3.toml.tmpl
install -d -m 0700 /space/config
cat >/space/config/space-containerd-config.new <<'EOF'
{{ template "base" . }}
[plugins."io.containerd.cri.v1.runtime".containerd.runtimes.space]
  runtime_type = "io.containerd.space.v2"
  pod_annotations = ["space.pplx.net/*"]
EOF
if [[ -e $space_shim_config ]] && ! cmp -s /space/config/space-containerd-config.new "$space_shim_config"; then
  echo 'The saved container runtime configuration differs. Review its checkpoints before changing it.' >&2
  exit 1
fi
if [[ -e /usr/local/lib/space/containerd-shim-space-v2 ]]; then
  printf '%s  %s\n' c35eb61afc6999c88bd3edafc872f37a2383a966989d6bd0f79b5d8cb8210241 /usr/local/lib/space/containerd-shim-space-v2 | sha256sum --check
fi
space_shim_installer=/space/config/install-shim-de87b436.sh
curl --fail --silent --show-error --location --retry 3 \
  https://raw.githubusercontent.com/perplexityai/SPACE/de87b436c35dac056564bdd5b2a113c9046e189b/infra/space/cluster/install-shim.sh \
  -o "$space_shim_installer"
printf '%s  %s\n' d1dd1a52965115800b4c7df111ca89bd0e76b81e64ba985ef6ef0126753cb49d "$space_shim_installer" | sha256sum --check
bash "$space_shim_installer" --binary "$space_shim" --version de87b436c35dac056564bdd5b2a113c9046e189b
install -d -m 0755 "$(dirname "$space_shim_config")"
install -m 0644 /space/config/space-containerd-config.new "$space_shim_config"
rm /space/config/space-containerd-config.new
printf '%s\n' 'Pinned shim and K3s runtime template installed. Restart the drained agent before admitting workloads.'
