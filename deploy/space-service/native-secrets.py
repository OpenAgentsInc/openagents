#!/usr/bin/env python3
"""Create private native credentials once; keep their backups in Secret Manager."""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import secrets
import ssl
import socket
import subprocess
import tempfile
from broker import cloud, PROJECT, PREFIX, LABELS


def run(*args):
    subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)


def encode(value):
    return base64.b64encode(value).decode()


def main(scratch):
    name = PREFIX + "-native-secrets"
    current = cloud("secrets", "describe", name, missing=True)
    if current is not None:
        if current.get("labels", {}).get("openagents-issue") != "11814":
            raise RuntimeError("The native credential belongs to another deployment.")
        versions = cloud("secrets", "versions", "list", name)
        if versions:
            if len(versions) != 1 or versions[0]["state"] != "ENABLED" or not versions[0]["name"].endswith("/versions/1"):
                raise RuntimeError("Review the existing native credential versions before changing them.")
            cloud("secrets", "add-iam-policy-binding", name, f"--member=serviceAccount:{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com", "--role=roles/secretmanager.secretAccessor")
            print("Keeping the existing native credentials and signing keys.")
            return
    scratch = Path(scratch).resolve()
    scratch.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix="space-pki-", dir=scratch) as directory:
        root = Path(directory)
        ca_key, ca_cert = root / "ca.key", root / "ca.crt"
        run("openssl", "genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", str(ca_key))
        run("openssl", "req", "-x509", "-new", "-key", str(ca_key), "-sha256", "-days", "3650", "-subj", "/CN=OpenAgents SPACE internal RPC CA",
            "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign", "-out", str(ca_cert))
        bundle = {}
        def entry(secret, values):
            bundle[secret] = {key: encode(value) for key, value in values.items()}
        entry("space-internal-rpc-ca", {"ca.crt": ca_cert.read_bytes()})
        leaves = {
            "space-cluster-api-tls": ("space-cluster-api", ["space-cluster-api", "space-internal-api"]),
            "space-node-api-tls": ("space-node-api", ["space-node-api"]),
            "space-build-manager-tls": ("space-build-manager", ["space-builder-build-manager"]),
            "space-ingress-activation-tls": ("space-ingress-activation", ["space-api"]),
            "space-credmgr-authority-tls": ("space-credmgr-authority", ["space-credmgr-authority"]),
            "space-management-edge-tls": ("space-management-edge", ["space-management-edge"]),
        }
        for secret, (common_name, services) in leaves.items():
            key, csr, cert, ext = (root / (secret + suffix) for suffix in (".key", ".csr", ".crt", ".ext"))
            names = {common_name}
            for service in services:
                names.update((service, service + ".space", service + ".space.svc", service + ".space.svc.cluster.local"))
            sans = ["DNS:" + value for value in sorted(names)]
            addresses = {"space-node-api-tls": ["10.219.14.11", "10.219.14.12"], "space-build-manager-tls": ["10.219.14.12"]}.get(secret, ["10.219.14.10"])
            sans.extend("IP:" + value for value in addresses)
            ext.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=" + ",".join(sans) + "\n")
            run("openssl", "genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", str(key))
            run("openssl", "req", "-new", "-key", str(key), "-subj", "/CN=" + common_name, "-out", str(csr))
            signer = ["-signkey", str(key)] if secret == "space-credmgr-authority-tls" else ["-CA", str(ca_cert), "-CAkey", str(ca_key)]
            run("openssl", "x509", "-req", "-in", str(csr), *signer, "-set_serial", str(secrets.randbits(128) or 1),
                "-days", "365", "-sha256", "-extfile", str(ext), "-out", str(cert))
            run("openssl", "verify", "-CAfile", str(cert if secret == "space-credmgr-authority-tls" else ca_cert), str(cert))
            entry(secret, {"tls.crt": cert.read_bytes(), "tls.key": key.read_bytes()})
        entry("space-credmgr-authority-trust", {"ca.crt": base64.b64decode(bundle["space-credmgr-authority-tls"]["tls.crt"])})
        signing, public = root / "route-signing.pem", root / "route-public.pem"
        run("openssl", "genpkey", "-algorithm", "ED25519", "-out", str(signing))
        run("openssl", "pkey", "-in", str(signing), "-pubout", "-out", str(public))
        issuer, key_id = "oa-space-service-route-authority", "oa-space-service-route-v1"
        entry("space-route-signing", {"signing-key.pem": signing.read_bytes(), "SPACE_API_ROUTE_ED25519_SIGNING_KEY_ID": key_id.encode(), "SPACE_API_ROUTE_ED25519_ISSUER": issuer.encode()})
        verification = {"generation": 1, "issuer": issuer, "keys": [{"key_id": key_id, "public_key_pem": public.read_text()}]}
        entry("space-route-verification", {"verification-bundle.json": json.dumps(verification).encode()})
        entry("space-internal", {key: secrets.token_urlsafe(48).encode() for key in ("SPACE_INTERNAL_TOKEN", "SPACE_INTERNAL_CONTROL_TOKEN", "SPACE_PUBLIC_CURSOR_SECRET", "ecw-credential-key")})
        ssh = root / "sandbox_ed25519"
        run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "OpenAgents SPACE service", "-f", str(ssh))
        entry("space-sandbox-ssh", {"sandbox_ed25519": ssh.read_bytes(), "sandbox_ed25519.pub": ssh.with_suffix(".pub").read_bytes()})
        ca_file = ssl.get_default_verify_paths().cafile
        if not ca_file:
            raise RuntimeError("The public CA trust bundle is unavailable.")
        google_roots = []
        for certificate in re.findall(rb"-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----", Path(ca_file).read_bytes(), re.S):
            subject = subprocess.check_output(["openssl", "x509", "-noout", "-subject", "-nameopt", "RFC2253"], input=certificate, stderr=subprocess.DEVNULL)
            if b"CN=GTS Root " in subject:
                google_roots.append(certificate + b"\n")
        if not google_roots:
            raise RuntimeError("The Google trust roots are unavailable.")
        trust = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        trust.load_verify_locations(cadata=b"".join(google_roots).decode())
        with socket.create_connection(("oa-space-service-keys-ezxz4mgdsq-uc.a.run.app", 443), timeout=10) as connection:
            with trust.wrap_socket(connection, server_hostname="oa-space-service-keys-ezxz4mgdsq-uc.a.run.app"):
                pass
        entry("space-key-broker-ca", {"ca.crt": b"".join(google_roots)})
        token = subprocess.check_output(["gcloud", "secrets", "versions", "access", "1", "--secret=" + PREFIX + "-key-token", "--project=" + PROJECT, "--quiet"], stderr=subprocess.DEVNULL)
        entry("space-key-broker-token", {"token": token})
        value = {"schema": "openagents.space.native-secrets.v1", "namespace": "space", "secrets": bundle, "rpc_ca_signing_key_pem": encode(ca_key.read_bytes())}
        encoded = json.dumps(value).encode()
        if len(encoded) > 65536:
            raise RuntimeError("The native credential bundle exceeds Secret Manager's size limit.")
        if current is None:
            cloud("secrets", "create", name, "--replication-policy=automatic", f"--labels={LABELS}")
        cloud("secrets", "versions", "add", name, "--data-file=-", data=encoded)
    cloud("secrets", "add-iam-policy-binding", name, f"--member=serviceAccount:{PREFIX}-control@{PROJECT}.iam.gserviceaccount.com", "--role=roles/secretmanager.secretAccessor")
    print(json.dumps({"secret": name, "version": 1, "native_secrets": len(bundle), "private_local_files_removed": True}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scratch", required=True)
    main(parser.parse_args().scratch)
