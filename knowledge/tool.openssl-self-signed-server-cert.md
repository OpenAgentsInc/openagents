---
id: tool.openssl-self-signed-server-cert
version: 1
kind: tool
title: Generate and validate a self-signed TLS server certificate
summary: >-
  Create a self-signed server certificate with explicit SAN and extensions,
  protect key material, and validate key/certificate properties independently.
  Useful for development and internal TLS fixtures, not a substitute for
  CA-issued production certificates.
tags: [openssl, tls, x509, python, permissions]
applies_when: >-
  A task requires a self-signed TLS certificate, corresponding private key and
  combined PEM, plus programmatic inspection or validation.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - openssl-selfsigned-cert
  cites:
    - OpenSSL Project, openssl-genpkey manual, DESCRIPTION
    - OpenSSL Project, openssl-req manual, X509 EXTENSIONS
    - IETF RFC 5280, sections 4.2.1.6, 4.2.1.9, 4.2.1.12
    - IETF RFC 9525, section 4.1
    - Python documentation, ssl — TLS/SSL wrapper for socket objects, SSLContext.load_verify_locations
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
Use OpenSSL's `genpkey` for a modern private-key generation interface, then `req -new -x509` to issue a self-signed certificate. Supply a Subject Alternative Name (SAN) explicitly; contemporary hostname verification uses SAN rather than relying on the Common Name. Add appropriate constraints and usages for a leaf TLS server certificate (for example, `CA:FALSE` and `serverAuth`), select the required digest and validity, and keep private-key files private. A combined PEM commonly concatenates the private key followed by the certificate; ensure its permissions are restrictive too. A self-signed certificate is not inherently trusted: trust must be provisioned separately, and its verification only establishes validity relative to an explicitly trusted certificate.

Useful references: OpenSSL Project, *openssl-genpkey* and *openssl-req* manuals, sections “DESCRIPTION” and “X509 EXTENSIONS”; IETF RFC 5280, *Internet X.509 Public Key Infrastructure Certificate and Certificate Revocation List (CRL) Profile*, sections 4.2.1.9 (Basic Constraints), 4.2.1.12 (Extended Key Usage), and 4.2.1.6 (Subject Alternative Name); IETF RFC 9525, *Service Identity in TLS*, section 4.1; Python documentation, *ssl — TLS/SSL wrapper for socket objects*, `SSLContext.load_verify_locations`.

## How to check
Run `openssl x509 -in server.crt -noout -subject -dates -ext subjectAltName -ext basicConstraints -ext keyUsage -ext extendedKeyUsage` and `openssl pkey -in server.key -check -noout`. Confirm the certificate public key matches the private key by comparing `openssl x509 -in server.crt -pubkey -noout` with `openssl pkey -in server.key -pubout`. In Python, load the certificate as a trust anchor with `ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT).load_verify_locations(cafile=...)`; this checks parse/loadability, not hostname identity or a live TLS handshake. For hostname verification, use `ssl.match_hostname` with the decoded certificate or validate a real connection with hostname checking enabled. Check restrictive key modes with `stat -c '%a %n'` on Unix-like systems.
