#!/usr/bin/env bash
# One-time GCP and DNS provisioning for iroh.openagents.com, as run for
# openagents#9968. It is a record of the commands, safe to read top to
# bottom; each step fails if its resource already exists.
#
# Requires: gcloud with the automation service account
#   export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
# and CLOUDFLARE_API_TOKEN (dns_records:edit on openagents.com) in the
# environment. Neither value is printed.
set -euo pipefail

PROJECT=openagentsgemini
REGION=us-central1
ZONE=us-central1-a
NAME=oa-iroh-relay-1
ADDR_NAME=oa-iroh-relay-ip
HOSTNAME=iroh.openagents.com
CF_ZONE_ID=bd33d951ee951a7c18fa4ab2ddcbe3a7   # openagents.com

gcloud compute addresses create "$ADDR_NAME" --project "$PROJECT" --region "$REGION"
IP="$(gcloud compute addresses describe "$ADDR_NAME" --project "$PROJECT" \
  --region "$REGION" --format='value(address)')"

# TCP 80 serves the relay over HTTP and TCP 443 over HTTPS (Let's Encrypt
# TLS-ALPN-01 also answers on 443). UDP 7842 is QUIC address discovery.
# Cloud Run cannot take this: it accepts no inbound UDP.
gcloud compute firewall-rules create iroh-relay-ingress --project "$PROJECT" \
  --network default --direction INGRESS --source-ranges 0.0.0.0/0 \
  --allow tcp:80,tcp:443,udp:7842 --target-tags iroh-relay \
  --description "iroh-relay: HTTP/HTTPS relay and ACME, UDP 7842 QUIC address discovery (openagents#9968)"

# The relay needs no Google API access, so the VM has no service account.
gcloud compute instances create "$NAME" --project "$PROJECT" --zone "$ZONE" \
  --machine-type e2-small --image-family debian-12 --image-project debian-cloud \
  --boot-disk-size 20GB --boot-disk-type pd-balanced --address "$IP" \
  --tags iroh-relay --no-service-account --no-scopes \
  --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
  --labels service=iroh-relay,issue=openagents-9968

# DNS only (grey cloud): Cloudflare's proxy would break UDP 7842 and the
# relay's own ACME challenge.
curl -fsS -X POST -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
  -H "Content-Type: application/json" \
  "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records" \
  --data "{\"type\":\"A\",\"name\":\"$HOSTNAME\",\"content\":\"$IP\",\"ttl\":300,\"proxied\":false,\"comment\":\"iroh-relay on GCE $NAME (openagents#9968)\"}" \
  >/dev/null

# Copy this directory to the VM and install.
gcloud compute scp --project "$PROJECT" --zone "$ZONE" \
  "$(dirname "$0")/iroh-relay.toml" "$(dirname "$0")/iroh-relay.service" \
  "$(dirname "$0")/install.sh" "$NAME:/tmp/"
gcloud compute ssh "$NAME" --project "$PROJECT" --zone "$ZONE" \
  --command 'cd /tmp && sudo ./install.sh'
