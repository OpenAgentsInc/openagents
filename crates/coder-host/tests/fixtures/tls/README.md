# Test-only TLS fixtures

**These certificates and keys are for tests only.** The private keys are
public in this repository, so nothing may trust these certificates outside
`crates/coder-host/tests/wss.rs`. No real key or certificate is stored here.

| File | Contents |
| --- | --- |
| `test-ca.pem` | A test root CA. The tests trust it explicitly. |
| `other-ca.pem` | A second, unrelated test root CA, for the untrusted-issuer case. |
| `localhost.pem` | A leaf certificate for `DNS:localhost`, issued by `test-ca.pem`. |
| `localhost.key` | The P-256 private key for `localhost.pem`, in PKCS #8. |
| `mismatched.key` | A P-256 private key that does not match `localhost.pem`. |

The certificates are valid until 2126. The CA private keys were discarded
after signing.

## Regenerate

Run these commands with OpenSSL 3 in an empty directory, then copy the five
files above into this directory:

```sh
cat > ca.cnf <<'EOF'
[req]
distinguished_name = dn
prompt = no
[dn]
CN = coder-host test-only CA
[ext]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
EOF
cat > leaf.cnf <<'EOF'
[ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost
authorityKeyIdentifier = keyid
subjectKeyIdentifier = hash
EOF
for ca in test-ca other-ca; do
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out $ca.key
  openssl req -x509 -new -key $ca.key -days 36500 -config ca.cnf \
    -extensions ext -subj "/CN=coder-host $ca (test only)" -out $ca.pem
done
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out localhost.key
openssl req -new -key localhost.key -subj "/CN=localhost" -out localhost.csr
openssl x509 -req -in localhost.csr -CA test-ca.pem -CAkey test-ca.key \
  -CAcreateserial -days 36500 -extfile leaf.cnf -extensions ext -out localhost.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out mismatched.key
```
