// Firm reason: 1 (the browser's NIP-07 window.nostr and WebAuthn navigator.credentials APIs, reachable only from page JS)
// /login's and /settings/security's browser-held credentials. Nothing is drawn here; the pages
// (login.coax, security.coax) decide, and every request to the API is theirs.
//
// A Nostr key: the browser's NIP-07 extension signs the NIP-98 event the API checks
// (coax/web/api/auth.c, POST /api/auth/nostr). The page asks with action 72, "URL\nBODY"; this
// answers message 72, "Nostr <base64 event>" or "!WORDS", and says whether an extension is here
// as message 73 ("1" or "0").
//
// A passkey (WebAuthn, coax/web/api/passkeys.c): the page asks with action 74,
// "MODE\nCHALLENGE\nRP_ID\nUSER_ID\nUSER_NAME\nID,ID" (base64url; MODE get, conditional or create;
// the IDs are allowed for get, excluded for create). This answers message 74 with the JSON body
// the API reads, or "!WORDS"; the autofill (conditional) says nothing when it is dismissed or
// replaced. Message 75 says what this browser can do: "0" no passkeys, "1" passkeys, "2" and the
// autofill. User verification is always required.
const SIGN = 72, HAS_KEY = 73, PASSKEY = 74, HAS_PASSKEY = 75;
const signer = () => !!(globalThis.nostr && typeof globalThis.nostr.signEvent === "function");
const send = (kind, text) => globalThis.coaxial?.send(kind, text);
const hex = (bytes) => Array.from(new Uint8Array(bytes), (b) => b.toString(16).padStart(2, "0")).join("");
const base64 = (text) => { let s = ""; for (const b of new TextEncoder().encode(text)) s += String.fromCharCode(b); return btoa(s); };

async function sign(request) {
  const nl = request.indexOf("\n");
  if (nl < 0) return send(SIGN, "!That didn't work. Try again.");
  if (!signer()) { send(HAS_KEY, "0"); return send(SIGN, "!This browser has no Nostr key extension (NIP-07). Add one, then try again."); }
  const url = request.slice(0, nl), body = request.slice(nl + 1);
  try {
    const payload = hex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
    const event = await globalThis.nostr.signEvent({ kind: 27235, created_at: Math.floor(Date.now() / 1000), content: "",
      tags: [["u", url], ["method", "POST"], ["payload", payload]] });
    if (!event || typeof event.sig !== "string") return send(SIGN, "!Your key extension didn't sign. Try again.");
    send(SIGN, "Nostr " + base64(JSON.stringify(event)));
  } catch {
    send(SIGN, "!Your key extension didn't sign. Try again.");
  }
}

// ---- passkeys
const b64u = (buf) => { let s = ""; for (const b of new Uint8Array(buf)) s += String.fromCharCode(b); return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, ""); };
const unb64u = (s) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4)), (c) => c.charCodeAt(0));
let autofill = null;   // the conditional request's AbortController, while it waits
async function canPasskey() {
  if (!globalThis.PublicKeyCredential || !navigator.credentials) return "0";
  try { return (await PublicKeyCredential.isConditionalMediationAvailable?.()) ? "2" : "1"; } catch { return "1"; }
}
// The autofill lists passkeys in a field marked "webauthn": the host's keyboard field (host.js
// makes it when the email field is first focused).
const FIELD = 'input[aria-label="Typed text"]';
let watching = null;
function markField() {
  const f = document.querySelector(FIELD);
  if (f) { f.setAttribute("autocomplete", "username webauthn"); return; }
  if (watching) return;
  watching = new MutationObserver(() => { const g = document.querySelector(FIELD); if (g) { g.setAttribute("autocomplete", "username webauthn"); watching.disconnect(); watching = null; } });
  watching.observe(document.body, { childList: true });
}
const words = (e, create) => e?.name === "NotAllowedError" ? (create ? "!No passkey was made. Try again." : "!No passkey was used. Try again.")
  : e?.name === "InvalidStateError" ? "!This device already has a passkey for this account."
  : e?.name === "SecurityError" ? "!This page can't use passkeys for openagents.com." : "!Your passkey didn't answer. Try again.";
async function passkey(request) {
  const [mode, challenge, rpId, userId, userName, list] = request.split("\n");
  if (autofill) { autofill.abort(); autofill = null; }
  if (!challenge || !rpId) return send(PASSKEY, "!That didn't work. Try again.");
  const creds = (list || "").split(",").filter(Boolean).map((id) => ({ type: "public-key", id: unb64u(id) }));
  if (mode === "create") {
    try {
      const c = await navigator.credentials.create({ publicKey: {
        challenge: unb64u(challenge), rp: { id: rpId, name: "OpenAgents" },
        user: { id: unb64u(userId), name: userName || "OpenAgents account", displayName: userName || "OpenAgents account" },
        pubKeyCredParams: [{ type: "public-key", alg: -7 }, { type: "public-key", alg: -8 }, { type: "public-key", alg: -257 }],
        timeout: 300000, attestation: "none", excludeCredentials: creds,
        authenticatorSelection: { residentKey: "required", requireResidentKey: true, userVerification: "required" },
        extensions: { credProps: true, prf: {} } } });
      const r = c.response;
      send(PASSKEY, JSON.stringify({ challenge, attestation: b64u(r.attestationObject), client_data: b64u(r.clientDataJSON), user_id: userId,
        transports: (r.getTransports?.() || []).join(","), prf: c.getClientExtensionResults?.().prf?.enabled ? "1" : "0" }));
    } catch (e) { send(PASSKEY, words(e, true)); }
    return;
  }
  const ac = new AbortController();
  if (mode === "conditional") { autofill = ac; markField(); }
  try {
    const c = await navigator.credentials.get({ publicKey: { challenge: unb64u(challenge), rpId, timeout: 300000, userVerification: "required", allowCredentials: creds },
      ...(mode === "conditional" ? { mediation: "conditional" } : {}), signal: ac.signal });
    if (autofill === ac) autofill = null;
    const r = c.response;
    send(PASSKEY, JSON.stringify({ challenge, id: c.id, auth_data: b64u(r.authenticatorData), client_data: b64u(r.clientDataJSON),
      signature: b64u(r.signature), user_handle: r.userHandle ? b64u(r.userHandle) : "" }));
  } catch (e) {
    if (autofill === ac) autofill = null;
    if (mode === "conditional") { if (e?.name !== "AbortError") send(PASSKEY, ""); return; }   // dismissed: nothing to say
    send(PASSKEY, words(e, false));
  }
}

addEventListener("coaxial", (e) => {
  const code = e.detail?.code;
  if (code === 0) {
    send(HAS_KEY, signer() ? "1" : "0");
    canPasskey().then((v) => send(HAS_PASSKEY, v));
    // An extension can arrive after the page: ask again once it has had time.
    setTimeout(() => send(HAS_KEY, signer() ? "1" : "0"), 1500);
  } else if (code === SIGN) sign(e.detail.text || "");
  else if (code === PASSKEY) passkey(e.detail.text || "");
});
