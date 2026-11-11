// Firm reason: 1 (the browser's NIP-07 window.nostr extension API, reachable only from page JS)
// /login's key login: the browser's NIP-07 extension signs the NIP-98 event the API
// checks (coax/web/api/auth.c, POST /api/auth/nostr). login.coax asks with action 72,
// "URL\nBODY"; this answers message 72, "Nostr <base64 event>" or "!WORDS", and says
// whether an extension is here as message 73 ("1" or "0"). Nothing is drawn here.
const SIGN = 72, HAS_KEY = 73;
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

addEventListener("coaxial", (e) => {
  const code = e.detail?.code;
  if (code === 0) {
    send(HAS_KEY, signer() ? "1" : "0");
    // An extension can arrive after the page: ask again once it has had time.
    setTimeout(() => send(HAS_KEY, signer() ? "1" : "0"), 1500);
  } else if (code === SIGN) sign(e.detail.text || "");
});
