// Firm reason: 1 (the browser's NIP-07 window.nostr and navigator.share APIs, reachable only from page JS)
// /settings/lightning's browser pieces. Nothing is drawn here and nothing is sent to the API; the
// page (lightning.coax) decides, and every request is its own.
//
// Message 78 says what this browser has: "KEY SHARE", each "1" or "0" (a NIP-07 extension, the
// share sheet). Action 79 asks the extension for its public key; message 79 answers the 64 hex
// digits or "!WORDS". Action 76 asks it to sign a kind 0 profile, "CREATED_AT\nCONTENT" (the
// content the API offered, unchanged); message 76 answers "Event <the signed event as JSON>" or
// "!WORDS". Action 77 opens the share sheet with the address.
const SIGN = 76, SHARE = 77, CAN = 78, PUBKEY = 79;
const signer = () => !!(globalThis.nostr && typeof globalThis.nostr.signEvent === "function" && typeof globalThis.nostr.getPublicKey === "function");
const sharer = () => typeof navigator.share === "function";
const send = (kind, text) => globalThis.coaxial?.send(kind, text);
const can = () => send(CAN, (signer() ? "1" : "0") + " " + (sharer() ? "1" : "0"));
const NO_KEY = "!This browser has no Nostr key extension (NIP-07). Add one, then try again.";

async function pubkey() {
  if (!signer()) { can(); return send(PUBKEY, NO_KEY); }
  try {
    const k = String(await globalThis.nostr.getPublicKey() || "").toLowerCase();
    send(PUBKEY, /^[0-9a-f]{64}$/.test(k) ? k : "!Your key extension didn't answer. Try again.");
  } catch { send(PUBKEY, "!Your key extension didn't answer. Try again."); }
}

async function sign(request) {
  const nl = request.indexOf("\n");
  const at = Number(request.slice(0, nl));
  if (nl < 0 || !Number.isSafeInteger(at)) return send(SIGN, "!That didn't work. Try again.");
  if (!signer()) { can(); return send(SIGN, NO_KEY); }
  try {
    const event = await globalThis.nostr.signEvent({ kind: 0, created_at: at, tags: [], content: request.slice(nl + 1) });
    if (!event || typeof event.sig !== "string") return send(SIGN, "!Your key extension didn't sign. Try again.");
    send(SIGN, "Event " + JSON.stringify(event));
  } catch { send(SIGN, "!Your key extension didn't sign. Try again."); }
}

addEventListener("coaxial", (e) => {
  const code = e.detail?.code, text = e.detail?.text || "";
  if (code === 0) { can(); setTimeout(can, 1500); }   // an extension can arrive after the page
  else if (code === PUBKEY) pubkey();
  else if (code === SIGN) sign(text);
  else if (code === SHARE && sharer()) navigator.share({ title: "My Lightning Address", text }).catch(() => {});
});
