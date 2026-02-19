# Trusting the Self-Signed Certificate (ws-test)

This guide is for end users who want to use `wss://` with the local ws-test server.
Browsers require the TLS certificate to be trusted, even for localhost.

## Step 0: Generate the certificate (first run only)

Run the server once so it creates a self-signed cert:

```bash
cargo ws
```

You should see a log mentioning a generated self-signed cert.

The cert file is saved here:
- macOS/Linux: `~/.openagents/ws-test/certs/ws-test.local.crt`
- Windows: `C:\Users\<you>\.openagents\ws-test\certs\ws-test.local.crt`

## Step 1: Trust the certificate

### macOS (GUI)
1. Open **Keychain Access**.
2. Drag `ws-test.local.crt` into the **System** keychain.
3. Double-click the cert, expand **Trust**, and set **When using this certificate** to **Always Trust**.
4. Close the window and enter your password.

### macOS (Terminal)
```bash
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain \
  ~/.openagents/ws-test/certs/ws-test.local.crt
```

### Windows
1. Press **Win + R**, type `mmc`, press Enter.
2. File -> **Add/Remove Snap-in...** -> **Certificates** -> **Computer account** -> **Local computer**.
3. In the left tree: **Trusted Root Certification Authorities** -> **Certificates**.
4. Right-click -> **All Tasks** -> **Import...** and select `ws-test.local.crt`.

### Linux (Debian/Ubuntu)
```bash
sudo cp ~/.openagents/ws-test/certs/ws-test.local.crt \
  /usr/local/share/ca-certificates/ws-test.local.crt
sudo update-ca-certificates
```

### Linux (Fedora/RHEL)
```bash
sudo cp ~/.openagents/ws-test/certs/ws-test.local.crt \
  /etc/pki/ca-trust/source/anchors/
sudo update-ca-trust
```

### Firefox (all OS)
Firefox uses its own trust store:
1. Settings -> **Privacy & Security** -> **Certificates** -> **View Certificates** -> **Authorities**.
2. Click **Import...** and select `ws-test.local.crt`.
3. Trust the certificate for websites.

## Step 2: Restart the browser
Close all browser windows and reopen so the trust change applies.

## Step 3: Test in the app
1. Start the server: `cargo ws`
2. Visit `https://hyperion.test/codex` (or your local app URL).
3. You should see the connection go "connected" and ticks arriving.

## Reset the certificate (if needed)
Delete the certs directory and run the server again:

```bash
rm -rf ~/.openagents/ws-test/certs
cargo ws
```
