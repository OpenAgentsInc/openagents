#!/bin/bash
# Install the private LNURL backend and its webhook-only TLS listener.
set -euo pipefail
umask 077
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq docker.io python3 ca-certificates curl
systemctl enable --now docker
install -d -m 700 /etc/openagents-lnurl /var/lib/openagents-lnurl/caddy
python3 - <<'PY'
import json, pathlib, urllib.request
headers = {'Metadata-Flavor': 'Google'}
def metadata(path):
    return urllib.request.urlopen(urllib.request.Request('http://metadata.google.internal/computeMetadata/v1/' + path, headers=headers)).read().decode()
token = json.loads(metadata('instance/service-accounts/default/token'))['access_token']
request = urllib.request.Request('https://secretmanager.googleapis.com/v1/projects/openagentsgemini/secrets/openagents-lnurl-runtime/versions/latest:access', headers={'Authorization': 'Bearer ' + token})
import base64
raw = base64.b64decode(json.load(urllib.request.urlopen(request))['payload']['data'])
pathlib.Path('/etc/openagents-lnurl/runtime.env').write_bytes(raw)
pathlib.Path('/etc/openagents-lnurl/image').write_text(metadata('instance/attributes/lnurl-image'))
import subprocess
subprocess.run(['docker', 'login', '-u', 'oauth2accesstoken', '--password-stdin', 'us-central1-docker.pkg.dev'], input=token.encode(), check=True, stdout=subprocess.DEVNULL)
PY
chmod 600 /etc/openagents-lnurl/*
cat > /etc/openagents-lnurl/Caddyfile <<'CADDY'
{
    email operations@openagents.com
    log default {
        level ERROR
    }
}
lnurl-hooks.openagents.com {
    @hook {
        path /webhook
        method POST
    }
    handle @hook {
        request_body {
            max_size 64KB
        }
        reverse_proxy 127.0.0.1:8181 {
            header_up Host openagents.com
            transport http {
                dial_timeout 3s
                response_header_timeout 10s
            }
        }
    }
    handle {
        respond 404
    }
}
http://:8180 {
    reverse_proxy 127.0.0.1:8181 {
        header_up Host openagents.com
    }
}
CADDY
# Docker's restart policy restores the services after a host restart.
docker rm -f lnurl cloud-sql-proxy lnurl-webhook >/dev/null 2>&1 || true
docker run -d --name cloud-sql-proxy --restart unless-stopped --network host \
    gcr.io/cloud-sql-connectors/cloud-sql-proxy:2.26.2 \
    --address=127.0.0.1 --port=5432 --quiet \
    openagentsgemini:us-central1:openagents-production-pg
for i in $(seq 1 60); do
    if (echo > /dev/tcp/127.0.0.1/5432) 2>/dev/null; then break; fi
    sleep 2
done
image=$(cat /etc/openagents-lnurl/image)
docker pull "$image"
docker run -d --name lnurl --restart unless-stopped --network host \
    --env-file /etc/openagents-lnurl/runtime.env "$image"
docker run -d --name lnurl-webhook --restart unless-stopped --network host \
    -v /etc/openagents-lnurl/Caddyfile:/etc/caddy/Caddyfile:ro \
    -v /var/lib/openagents-lnurl/caddy:/data caddy:2.10.2
rm -f /root/.docker/config.json
echo 'Lightning backend and webhook listener installed.'
