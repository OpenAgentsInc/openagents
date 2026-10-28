#!/usr/bin/env bash
# One-time: put Google's CDN in front of an app's bucket (see deploy.sh) at a
# host name. Makes the bucket's objects public, a CDN backend bucket, a URL
# map, an HTTPS proxy with a Certificate Manager certificate (DNS-authorized,
# so it's issued before DNS moves), an HTTP-to-HTTPS redirect and one global IP.
#   cdn.sh SERVICE HOST      e.g. cdn.sh openagents-web-staging staging.openagents.com
# Then point HOST's A record at the printed IP (DNS only, not proxied).
set -euo pipefail
service="$1" host="$2" project=openagentsgemini
bucket="$project-$service" name="coaxial-$service"
cert="$(echo "$host" | tr . -)"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
g() { gcloud "$@" --project "$project" --quiet; }
exists() { gcloud "$@" --project "$project" > /dev/null 2>&1; }

g storage buckets add-iam-policy-binding "gs://$bucket" --member allUsers --role roles/storage.objectViewer > /dev/null
g storage buckets update "gs://$bucket" --web-main-page-suffix index.html --web-error-page 404.html > /dev/null
exists compute backend-buckets describe "$name" ||
  g compute backend-buckets create "$name" --gcs-bucket-name "$bucket" --enable-cdn --cache-mode USE_ORIGIN_HEADERS
exists compute url-maps describe "$name" || g compute url-maps create "$name" --default-backend-bucket "$name"
exists certificate-manager dns-authorizations describe "$cert" ||
  g certificate-manager dns-authorizations create "$cert" --domain "$host"
exists certificate-manager certificates describe "$cert" ||
  g certificate-manager certificates create "$cert" --domains "$host" --dns-authorizations "$cert"
exists certificate-manager maps describe "$name" || g certificate-manager maps create "$name"
exists certificate-manager maps entries describe "$cert" --map "$name" ||
  g certificate-manager maps entries create "$cert" --map "$name" --hostname "$host" --certificates "$cert"
exists compute target-https-proxies describe "$name" ||
  g compute target-https-proxies create "$name" --url-map "$name" --certificate-map "$name"
exists compute addresses describe "$name" --global || g compute addresses create "$name" --global
exists compute forwarding-rules describe "$name" --global ||
  g compute forwarding-rules create "$name" --global --address "$name" --target-https-proxy "$name" --ports 443
# plain HTTP redirects to HTTPS
if ! exists compute url-maps describe "$name-http"; then
  printf 'name: %s-http\ndefaultUrlRedirect:\n  httpsRedirect: true\n  redirectResponseCode: MOVED_PERMANENTLY_DEFAULT\n' "$name" > /tmp/coaxial-redirect.yaml
  g compute url-maps import "$name-http" --source /tmp/coaxial-redirect.yaml --global
fi
exists compute target-http-proxies describe "$name-http" || g compute target-http-proxies create "$name-http" --url-map "$name-http"
exists compute forwarding-rules describe "$name-http" --global ||
  g compute forwarding-rules create "$name-http" --global --address "$name" --target-http-proxy "$name-http" --ports 80
echo "DNS: $host A $(gcloud compute addresses describe "$name" --global --project "$project" --format='value(address)')"
echo "DNS authorization: $(gcloud certificate-manager dns-authorizations describe "$cert" --project "$project" --format='value(dnsResourceRecord.name,dnsResourceRecord.data)')"
