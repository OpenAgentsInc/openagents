#!/usr/bin/env python3
"""Generate the synthetic attestation vectors for crates/att-platforms.

Every root here is synthetic and pinned only by the tests. None of these
files is evidence from real hardware. Run from this directory:

    python3 generate.py

It needs the `cryptography` package. The output is deterministic except for
signatures, which are fresh each run; the Rust tests verify whatever is here.
"""
import datetime
import hashlib
import json
import os

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature
from cryptography.x509.oid import NameOID, ObjectIdentifier

OUT = os.path.dirname(os.path.abspath(__file__))
START = datetime.datetime(2020, 1, 1, tzinfo=datetime.timezone.utc)
END = datetime.datetime(2099, 1, 1, tzinfo=datetime.timezone.utc)


def name(cn):
    return x509.Name([x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Synthetic"), x509.NameAttribute(NameOID.COMMON_NAME, cn)])


def cert(subject_key, subject, issuer_key, issuer, serial, algorithm, ca=False, extensions=(), rsa_padding=None):
    builder = (
        x509.CertificateBuilder()
        .subject_name(name(subject))
        .issuer_name(name(issuer))
        .public_key(subject_key.public_key())
        .serial_number(serial)
        .not_valid_before(START)
        .not_valid_after(END)
        .add_extension(x509.BasicConstraints(ca=ca, path_length=None), critical=True)
    )
    for oid, value in extensions:
        builder = builder.add_extension(x509.UnrecognizedExtension(oid, value), critical=False)
    if rsa_padding is not None:
        return builder.sign(issuer_key, algorithm, rsa_padding=rsa_padding)
    return builder.sign(issuer_key, algorithm)


def der(c):
    return c.public_bytes(serialization.Encoding.DER)


def pem(c):
    return c.public_bytes(serialization.Encoding.PEM)


def key_pem(k):
    return k.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())


def write(path, data):
    full = os.path.join(OUT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, "wb") as f:
        f.write(data)


def crl(issuer_key, issuer, revoked, algorithm, next_update, rsa_padding=None):
    builder = (
        x509.CertificateRevocationListBuilder()
        .issuer_name(name(issuer))
        .last_update(START)
        .next_update(next_update)
    )
    for serial in revoked:
        builder = builder.add_revoked_certificate(
            x509.RevokedCertificateBuilder().serial_number(serial).revocation_date(START).build()
        )
    if rsa_padding is not None:
        return builder.sign(issuer_key, algorithm, rsa_padding=rsa_padding).public_bytes(serialization.Encoding.DER)
    return builder.sign(issuer_key, algorithm).public_bytes(serialization.Encoding.DER)


def der_len(n):
    if n < 0x80:
        return bytes([n])
    body = n.to_bytes((n.bit_length() + 7) // 8, "big")
    return bytes([0x80 | len(body)]) + body


def tlv(tag, body):
    return bytes([tag]) + der_len(len(body)) + body


def oid_der(dotted):
    parts = [int(p) for p in dotted.split(".")]
    body = bytes([40 * parts[0] + parts[1]])
    for p in parts[2:]:
        chunk = [p & 0x7F]
        p >>= 7
        while p:
            chunk.append(0x80 | (p & 0x7F))
            p >>= 7
        body += bytes(reversed(chunk))
    return tlv(0x06, body)


# ---- Intel TDX (DCAP) ----
root = ec.generate_private_key(ec.SECP256R1())
inter = ec.generate_private_key(ec.SECP256R1())
pck = ec.generate_private_key(ec.SECP256R1())
tcb_signer = ec.generate_private_key(ec.SECP256R1())
fmspc = bytes.fromhex("00906ed50000")
# SGX extensions: SEQUENCE { SEQUENCE { OID fmspc, OCTET STRING } }.
sgx = tlv(0x30, tlv(0x30, oid_der("1.2.840.113741.1.13.1.4") + tlv(0x04, fmspc)))
root_c = cert(root, "Synthetic SGX Root CA", root, "Synthetic SGX Root CA", 1, hashes.SHA256(), ca=True)
inter_c = cert(inter, "Synthetic PCK Platform CA", root, "Synthetic SGX Root CA", 2, hashes.SHA256(), ca=True)
pck_c = cert(pck, "Synthetic PCK Certificate", inter, "Synthetic PCK Platform CA", 3, hashes.SHA256(),
             extensions=[(ObjectIdentifier("1.2.840.113741.1.13.1"), sgx)])
tcb_c = cert(tcb_signer, "Synthetic TCB Signing", root, "Synthetic SGX Root CA", 4, hashes.SHA256())
write("tdx/root.der", der(root_c))
write("tdx/pck_chain.pem", pem(pck_c) + pem(inter_c) + pem(root_c))
write("tdx/pck_key.pem", key_pem(pck))
write("tdx/tcb_signing_chain.pem", pem(tcb_c) + pem(root_c))
write("tdx/pck_crl.der", crl(inter, "Synthetic PCK Platform CA", [], hashes.SHA256(), END))
write("tdx/pck_crl_revoked.der", crl(inter, "Synthetic PCK Platform CA", [3], hashes.SHA256(), END))
write("tdx/pck_crl_stale.der", crl(inter, "Synthetic PCK Platform CA", [], hashes.SHA256(), START + datetime.timedelta(days=1)))


def tcb_info(next_update, status):
    info = {
        "id": "TDX", "version": 3, "issueDate": "2026-01-01T00:00:00Z", "nextUpdate": next_update,
        "fmspc": fmspc.hex(), "tcbLevels": [
            {"tcb": {"tdxtcbcomponents": [{"svn": 2}] + [{"svn": 0}] * 15}, "tcbStatus": status},
            {"tcb": {"tdxtcbcomponents": [{"svn": 0}] * 16}, "tcbStatus": "OutOfDate"},
        ],
    }
    body = json.dumps(info, separators=(",", ":")).encode()
    r, s = decode_dss_signature(tcb_signer.sign(body, ec.ECDSA(hashes.SHA256())))
    sig = r.to_bytes(32, "big") + s.to_bytes(32, "big")
    return b'{"tcbInfo":' + body + b',"signature":"' + sig.hex().encode() + b'"}'


write("tdx/tcb_info.json", tcb_info("2099-01-01T00:00:00Z", "UpToDate"))
write("tdx/tcb_info_stale.json", tcb_info("2020-02-01T00:00:00Z", "UpToDate"))
write("tdx/tcb_info_revoked.json", tcb_info("2099-01-01T00:00:00Z", "Revoked"))

# ---- AMD SEV-SNP ----
pss = padding.PSS(mgf=padding.MGF1(hashes.SHA384()), salt_length=48)
ark = rsa.generate_private_key(public_exponent=65537, key_size=2048)
ask = rsa.generate_private_key(public_exponent=65537, key_size=2048)
vcek = ec.generate_private_key(ec.SECP384R1())
chip = bytes(range(64))


def integer(n):
    return tlv(0x02, n.to_bytes(1, "big") if n < 0x80 else b"\x00" + n.to_bytes(1, "big"))


amd = "1.3.6.1.4.1.3704.1."
ark_c = cert(ark, "ARK-Synthetic", ark, "ARK-Synthetic", 11, hashes.SHA384(), ca=True, rsa_padding=pss)
ask_c = cert(ask, "SEV-Synthetic", ark, "ARK-Synthetic", 12, hashes.SHA384(), ca=True, rsa_padding=pss)
vcek_c = cert(vcek, "SEV-VCEK", ask, "SEV-Synthetic", 13, hashes.SHA384(), rsa_padding=pss, extensions=[
    (ObjectIdentifier(amd + "3.1"), integer(3)),   # blSPL
    (ObjectIdentifier(amd + "3.2"), integer(0)),   # teeSPL
    (ObjectIdentifier(amd + "3.3"), integer(8)),   # snpSPL
    (ObjectIdentifier(amd + "3.8"), integer(115)),  # ucodeSPL
    (ObjectIdentifier(amd + "4"), tlv(0x04, chip)),  # hwID
])
write("snp/ark.pem", pem(ark_c))
write("snp/ask.pem", pem(ask_c))
write("snp/vcek.der", der(vcek_c))
write("snp/vcek_key.pem", key_pem(vcek))
write("snp/chip_id.hex", chip.hex().encode())
write("snp/crl.der", crl(ark, "ARK-Synthetic", [], hashes.SHA384(), END, rsa_padding=pss))
write("snp/crl_revoked.der", crl(ark, "ARK-Synthetic", [12], hashes.SHA384(), END, rsa_padding=pss))

# ---- NVIDIA NRAS ----
nv_root = rsa.generate_private_key(public_exponent=65537, key_size=2048)
nv_inter = rsa.generate_private_key(public_exponent=65537, key_size=2048)
nv_leaf = ec.generate_private_key(ec.SECP384R1())
nv_root_c = cert(nv_root, "Synthetic NVIDIA Attestation CA", nv_root, "Synthetic NVIDIA Attestation CA", 21, hashes.SHA256(), ca=True)
nv_inter_c = cert(nv_inter, "Synthetic NRAS GPU Intermediate", nv_root, "Synthetic NVIDIA Attestation CA", 22, hashes.SHA256(), ca=True)
nv_leaf_c = cert(nv_leaf, "Synthetic NRAS GPU Signer", nv_inter, "Synthetic NRAS GPU Intermediate", 23, hashes.SHA256())
numbers = nv_leaf.public_key().public_numbers()


def b64url(b):
    import base64
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def b64(b):
    import base64
    return base64.b64encode(b).decode()


jwks = {"keys": [{
    "kty": "EC", "crv": "P-384", "kid": "synthetic-nras-kid",
    "x": b64url(numbers.x.to_bytes(48, "big")), "y": b64url(numbers.y.to_bytes(48, "big")),
    "x5c": [b64(der(nv_leaf_c)), b64(der(nv_inter_c))],
}]}
write("nras/anchor.der", der(nv_inter_c))
write("nras/jwks.json", json.dumps(jwks, indent=1).encode())
write("nras/signer_key.pem", key_pem(nv_leaf))

print("wrote synthetic vectors to", OUT)
for path in ["tdx/root.der", "snp/ark.pem", "nras/anchor.der"]:
    with open(os.path.join(OUT, path), "rb") as f:
        data = f.read()
    print(path, hashlib.sha256(data).hexdigest())
