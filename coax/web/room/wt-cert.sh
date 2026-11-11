#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around the openssl command every host has; nothing is installed)
# A WebTransport certificate (#13175): a new ECDSA P-256 key (PKCS#8, as wt.c reads it) and a
# self-signed certificate for localhost and 127.0.0.1, valid for DAYS (default 10: Chrome's
# serverCertificateHashes takes only P-256 and under 14 days). Prints the certificate's SHA-256 in
# hex, the hash a page passes to `new WebTransport(url, {serverCertificateHashes})`.
#   coax/web/room/wt-cert.sh CERT.pem KEY.pem [DAYS] [NAME]
set -euo pipefail
cert="${1:?CERT.pem}"; key="${2:?KEY.pem}"; days="${3:-10}"; name="${4:-localhost}"
(( days >= 1 && days <= 13 )) || { echo "wt-cert.sh: DAYS must be 1 to 13" >&2; exit 2; }
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-wtcert.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
openssl ecparam -name prime256v1 -genkey -noout -out "$work/ec.pem" 2>/dev/null
openssl pkcs8 -topk8 -nocrypt -in "$work/ec.pem" -out "$key"
chmod 600 "$key"
printf '[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=%s\n[ext]\nsubjectAltName=DNS:localhost,IP:127.0.0.1,DNS:%s\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n' "$name" "$name" > "$work/req.cnf"
openssl req -new -x509 -key "$key" -sha256 -days "$days" -config "$work/req.cnf" -out "$cert" 2>/dev/null
openssl x509 -in "$cert" -outform der | openssl dgst -sha256 -r | cut -d ' ' -f 1
