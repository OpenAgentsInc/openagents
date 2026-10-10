// The vault page (#11240, nips/openagents/NIP-VAULT.md, tier "only you").
//
// This script does the network, WebAuthn and Nostr calls and fills the page.
// Every key and every cipher is in oa-vault, compiled to WebAssembly
// (crates/oa-vault-web): the vault master key never leaves that module, and
// this script sees only method secrets on their way in, sealed bytes, the
// file list, and a file the person asked to open. The page builds its rows
// from <template> clones and textContent; it parses no HTML.

const root = document.getElementById("vault");
const $ = (id) => document.getElementById(id);
const project = root.dataset.project || "";
const tokenHeader = root.dataset.csrfHeader;
let csrf = root.dataset.csrf;
let wasm = null;
let vault = null;
let state = null;
let pairingSlot = null;
let localModel = null;
const steps = ["vault-unsupported", "vault-new", "vault-code", "vault-locked", "vault-open"];
const MAX_FILE = 10 * 1024 * 1024;
const LOCAL = "http://127.0.0.1:8091";

function show(id) {
  for (const step of steps) $(step).hidden = step !== id;
}

function say(text) {
  $("vault-status").textContent = text;
}

function fail(error) {
  const box = $("vault-error");
  box.textContent = typeof error === "string" ? error : error && error.message ? error.message : "Something went wrong. Try again.";
  box.hidden = false;
  say("");
}

function clearError() {
  $("vault-error").hidden = true;
  $("vault-error").textContent = "";
}

function now() {
  return Math.floor(Date.now() / 1000);
}

function random(n) {
  return crypto.getRandomValues(new Uint8Array(n));
}

function b64(bytes) {
  let text = "";
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  for (let i = 0; i < view.length; i += 0x8000) {
    text += String.fromCharCode.apply(null, view.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

function unb64(text) {
  const raw = atob(text);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

function b64url(bytes) {
  return b64(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function size(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function when(seconds) {
  try {
    return new Date(seconds * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
  } catch {
    return "";
  }
}

function platformName() {
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  if (/mac/i.test(platform)) return "Mac";
  if (/iphone|ipad|ios/i.test(platform)) return "iPhone or iPad";
  if (/android/i.test(platform)) return "Android";
  if (/win/i.test(platform)) return "Windows";
  if (/linux/i.test(platform)) return "Linux";
  return "this browser";
}

// The newest index epoch this browser has seen, so an older one is refused.
function seenEpoch(id) {
  try {
    return Number(localStorage.getItem(`oa-vault-epoch:${id}`)) || 0;
  } catch {
    return 0;
  }
}

function remember(id, epoch) {
  try {
    if (epoch > seenEpoch(id)) localStorage.setItem(`oa-vault-epoch:${id}`, String(epoch));
  } catch {
    // Private windows may refuse storage; the vault still works.
  }
}

function forget(id) {
  try {
    localStorage.removeItem(`oa-vault-epoch:${id}`);
  } catch {
    // Nothing kept.
  }
}

async function api(method, path, body, raw) {
  const headers = {};
  if (method !== "GET") headers[tokenHeader] = csrf;
  const init = { method, headers, credentials: "same-origin", cache: "no-store" };
  if (raw) {
    headers["content-type"] = "application/octet-stream";
    init.body = raw;
  } else if (body !== undefined) {
    headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  const response = await fetch(path, init);
  const fresh = response.headers.get(tokenHeader);
  if (fresh) csrf = fresh;
  if (!response.ok) {
    let text = "Something went wrong. Try again.";
    try {
      text = (await response.json()).error || text;
    } catch {
      // Not JSON.
    }
    const error = new Error(text);
    error.status = response.status;
    throw error;
  }
  return response;
}

async function loadWasm() {
  const glue = await import(root.dataset.glue);
  const response = await fetch("/vault/assets/oa_vault_web_bg.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error("The vault couldn't load. Reload the page.");
  const bytes = new Uint8Array(await response.arrayBuffer());
  const digest = `sha384-${b64(await crypto.subtle.digest("SHA-384", bytes))}`;
  if (digest !== root.dataset.wasm) throw new Error("The vault's code didn't match what this page expects. Reload the page.");
  await glue.default({ module_or_path: bytes });
  wasm = glue;
}

async function fetchState() {
  const response = await api("GET", "/vault/api/state");
  state = (await response.json()).vault;
}

function slotsOf(method) {
  return state ? state.slots.filter((slot) => slot.method === method) : [];
}

function prfMaybe() {
  return typeof PublicKeyCredential !== "undefined" && !!navigator.credentials;
}

async function prfLikely() {
  if (!prfMaybe()) return false;
  try {
    if (typeof PublicKeyCredential.getClientCapabilities === "function") {
      const capabilities = await PublicKeyCredential.getClientCapabilities();
      if (capabilities && "extension:prf" in capabilities) return !!capabilities["extension:prf"];
    }
  } catch {
    // Unknown: offer it and see.
  }
  return true;
}

function nostrSigner() {
  return window.nostr && window.nostr.nip44 && typeof window.nostr.nip44.encrypt === "function" ? window.nostr : null;
}

// A new passkey that can give a key (WebAuthn PRF), and that key.
async function newPasskey() {
  const salt = random(32);
  const credential = await navigator.credentials.create({
    publicKey: {
      rp: { id: location.hostname, name: "OpenAgents" },
      user: { id: random(16), name: `${root.dataset.account || "You"} · vault`, displayName: "OpenAgents vault" },
      challenge: random(32),
      pubKeyCredParams: [
        { type: "public-key", alg: -7 },
        { type: "public-key", alg: -257 },
      ],
      authenticatorSelection: { residentKey: "preferred", userVerification: "required" },
      timeout: 120000,
      extensions: { prf: { eval: { first: salt } } },
    },
  });
  if (!credential) throw new Error("No passkey was made.");
  const prf = credential.getClientExtensionResults().prf;
  if (!prf || prf.enabled === false) {
    throw new Error("This passkey can't lock files in this browser. Try another passkey, or use your Nostr key.");
  }
  const id = new Uint8Array(credential.rawId);
  let output = prf.results && prf.results.first;
  if (!output) {
    say("Touch your passkey once more to finish.");
    const used = await usePasskey([{ id, salt }]);
    output = used.output;
  }
  return { id, salt, output: new Uint8Array(output) };
}

// Ask one of `keys` ({id, salt}) for its key.
async function usePasskey(keys) {
  const byId = {};
  for (const key of keys) byId[b64url(key.id)] = { first: key.salt };
  const assertion = await navigator.credentials.get({
    publicKey: {
      challenge: random(32),
      rpId: location.hostname,
      allowCredentials: keys.map((key) => ({ type: "public-key", id: key.id })),
      userVerification: "required",
      timeout: 120000,
      extensions: { prf: { evalByCredential: byId } },
    },
  });
  const prf = assertion && assertion.getClientExtensionResults().prf;
  const output = prf && prf.results && prf.results.first;
  if (!output) throw new Error("This passkey didn't give a key in this browser. Use your recovery code.");
  return { id: new Uint8Array(assertion.rawId), output: new Uint8Array(output) };
}

function passkeySlotFor(made) {
  return JSON.parse(vault.passkeySlot(`Passkey · ${platformName()}`, now(), location.hostname, made.id, made.salt, made.output));
}

async function nostrSlot() {
  const signer = nostrSigner();
  if (!signer) throw new Error("No Nostr signer is available in this browser.");
  const pubkey = await signer.getPublicKey();
  const secret = wasm.randomHex();
  const sealed = await signer.nip44.encrypt(pubkey, secret);
  return JSON.parse(vault.nostrSlot("Nostr key", now(), pubkey, sealed, secret));
}

// ---- Setup ----

let setupSlots = [];
let recovery = null;

async function setupView() {
  show("vault-new");
  say("");
  const prf = await prfLikely();
  const nostr = !!nostrSigner();
  $("vault-setup-passkey").hidden = !prf;
  $("vault-setup-nostr").hidden = !nostr;
  $("vault-setup-none").hidden = prf || nostr;
}

async function startSetup(make) {
  clearError();
  try {
    vault = wasm.Vault.create();
    setupSlots = [await make()];
    say("Making your recovery code…");
    await new Promise((resolve) => setTimeout(resolve, 30));
    recovery = JSON.parse(vault.recoverySlot(now()));
    showWords(recovery.words);
    say("");
  } catch (error) {
    if (vault) vault.free();
    vault = null;
    fail(error);
  }
}

let checks = [];

function showWords(words) {
  const list = $("vault-words");
  list.replaceChildren();
  for (const word of words.split(" ")) {
    const item = document.createElement("li");
    item.textContent = word;
    list.append(item);
  }
  const picks = new Set();
  while (picks.size < 3) picks.add(Math.floor(Math.random() * 24));
  checks = [...picks].sort((a, b) => a - b);
  ["a", "b", "c"].forEach((name, i) => {
    $(`vault-check-${name}-label`).textContent = `Word ${checks[i] + 1}`;
    $(`vault-check-${name}`).value = "";
  });
  show("vault-code");
}

async function finishSetup() {
  clearError();
  const words = recovery.words.split(" ");
  const typed = ["a", "b", "c"].map((name) => $(`vault-check-${name}`).value.trim().toLowerCase());
  if (typed.some((word, i) => word !== words[checks[i]])) {
    fail("Those words don't match your code. Check what you wrote down.");
    return;
  }
  try {
    say("Saving your vault…");
    await api("POST", "/vault/api/create", {
      vault: vault.id,
      slots: [...setupSlots, recovery.slot],
      index: b64(vault.indexBlob()),
    });
    $("vault-words").replaceChildren();
    recovery = null;
    remember(vault.id, 1);
    await fetchState();
    await openView();
    say("Your vault is ready.");
  } catch (error) {
    fail(error);
  }
}

// ---- Unlock ----

function lockedView() {
  show("vault-locked");
  const passkeys = slotsOf("passkey-prf").filter((slot) => slot.params.rp_id === location.hostname);
  $("vault-unlock-passkey").hidden = !(passkeys.length && prfMaybe());
  $("vault-unlock-nostr").hidden = !(slotsOf("nostr").length && nostrSigner());
  $("vault-recovery").open = $("vault-unlock-passkey").hidden && $("vault-unlock-nostr").hidden;
  say("");
}

async function afterUnlock() {
  const index = unb64(state.index.blob);
  vault.loadIndex(index, seenEpoch(state.id));
  remember(state.id, vault.epoch);
  await openView();
}

async function unlockWith(open) {
  clearError();
  try {
    say("Unlocking…");
    // The vault may have changed since the page loaded or was locked.
    await fetchState();
    if (!state) {
      await setupView();
      return;
    }
    vault = await open();
    await afterUnlock();
    say("Unlocked.");
  } catch (error) {
    if (vault) vault.free();
    vault = null;
    fail(error);
  }
}

async function unlockPasskey() {
  const slots = slotsOf("passkey-prf").filter((slot) => slot.params.rp_id === location.hostname);
  const used = await usePasskey(slots.map((slot) => ({ id: unb64(slot.params.credential_id), salt: unb64(slot.params.prf_salt) })));
  const id = b64(used.id);
  const slot = slots.find((s) => s.params.credential_id === id);
  if (!slot) throw new Error("That passkey isn't one of this vault's.");
  return wasm.Vault.unlock(state.id, JSON.stringify(slot), used.output);
}

async function unlockNostr() {
  const signer = nostrSigner();
  const pubkey = await signer.getPublicKey();
  const slot = slotsOf("nostr").find((s) => s.params.pubkey === pubkey);
  if (!slot) throw new Error("This Nostr key isn't one of this vault's.");
  const secret = await signer.nip44.decrypt(pubkey, slot.params.sealed);
  return wasm.Vault.unlockNostr(state.id, JSON.stringify(slot), secret);
}

function unlockRecovery() {
  const field = $("vault-recovery-words");
  const words = field.value;
  const slot = slotsOf("recovery")[0];
  if (!slot) throw new Error("This vault has no recovery code.");
  say("Checking your recovery code…");
  const opened = wasm.Vault.unlockRecovery(state.id, JSON.stringify(slot), words);
  field.value = "";
  return opened;
}

async function unlockPairing(fragment) {
  $("vault-pairing-note").hidden = false;
  history.replaceState(null, "", location.pathname + location.search);
  await unlockWith(async () => {
    const paired = wasm.Vault.unlockPairing(state.id, JSON.stringify(state.slots), fragment);
    pairingSlot = paired.slot;
    return paired.take();
  });
  $("vault-pairing-note").hidden = true;
  if (vault) say("Unlocked with your other device. Add a passkey or your Nostr key below so this device can unlock on its own.");
}

// ---- The open vault ----

function rows() {
  return JSON.parse(vault.rows(project));
}

async function openView() {
  show("vault-open");
  renderFiles();
  renderDevices();
  $("vault-add-passkey").hidden = !(await prfLikely());
  $("vault-add-nostr").hidden = !nostrSigner() || slotsOf("nostr").length > 0;
  await probeLocal();
}

function safeType(bytes, media) {
  const head = bytes.subarray(0, 12);
  const starts = (sig) => sig.every((b, i) => head[i] === b);
  if (starts([0x25, 0x50, 0x44, 0x46, 0x2d])) return "application/pdf";
  if (starts([0x89, 0x50, 0x4e, 0x47])) return "image/png";
  if (starts([0xff, 0xd8, 0xff])) return "image/jpeg";
  if (starts([0x47, 0x49, 0x46, 0x38])) return "image/gif";
  if (starts([0x52, 0x49, 0x46, 0x46]) && head[8] === 0x57 && head[9] === 0x45) return "image/webp";
  if ((media || "").startsWith("text/") || media === "application/json" || media === "text/csv") return "text/plain; charset=utf-8";
  return null;
}

async function fetchPlain(row) {
  const response = await api("GET", `/vault/api/objects/${row.object}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  return vault.open(row.object, bytes);
}

async function openFile(row, download) {
  clearError();
  try {
    say(`Opening ${row.name}…`);
    const plain = await fetchPlain(row);
    const type = safeType(plain, row.media);
    const url = URL.createObjectURL(new Blob([plain], { type: download || !type ? "application/octet-stream" : type }));
    if (download || !type) {
      const link = document.createElement("a");
      link.href = url;
      link.download = row.name;
      document.body.append(link);
      link.click();
      link.remove();
    } else {
      window.open(url, "_blank", "noopener");
    }
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    say(download || !type ? `Saved ${row.name}.` : `Opened ${row.name}.`);
  } catch (error) {
    fail(error);
  }
}

// Write the staged index; on a conflict, reload and run `stage` again once.
async function writeIndex(deleted, stage) {
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      await api("POST", "/vault/api/index", { after: vault.epoch, blob: b64(vault.pendingBlob()), delete: deleted });
      vault.commit();
      remember(vault.id, vault.epoch);
      return;
    } catch (error) {
      vault.discard();
      if (error.status !== 409 || attempt === 1) throw error;
      await fetchState();
      vault.loadIndex(unb64(state.index.blob), seenEpoch(state.id));
      await stage();
    }
  }
}

async function store(kind, name, media, about, route, bytes) {
  let sealed = null;
  const stage = async () => {
    sealed = vault.add(kind, name, media, project, JSON.stringify(about), route, bytes, now());
    await api("PUT", `/vault/api/objects/${sealed.id}`, undefined, sealed.bytes());
  };
  await stage();
  await writeIndex([], stage);
}

async function addFiles(files) {
  clearError();
  for (const file of files) {
    if (file.size > MAX_FILE) {
      fail(`${file.name} is larger than 10 MB.`);
      continue;
    }
    try {
      say(`Locking ${file.name}…`);
      const bytes = new Uint8Array(await file.arrayBuffer());
      await store("file", file.name, file.type || "", [], "", bytes);
      say(`Added ${file.name}.`);
    } catch (error) {
      fail(error);
    }
  }
  $("vault-file").value = "";
  renderFiles();
}

async function removeFile(row) {
  if (!confirm(`Delete “${row.name}”? Nobody can open it after this, including you.`)) return;
  clearError();
  try {
    say(`Deleting ${row.name}…`);
    vault.remove(row.object);
    await writeIndex([row.object], async () => vault.remove(row.object));
    say(`Deleted ${row.name}. It can't be opened any more.`);
    renderFiles();
  } catch (error) {
    fail(error);
  }
}

function renderFiles() {
  const all = rows();
  const files = all.filter((row) => row.kind === "file");
  const answers = all.filter((row) => row.kind === "answer");
  fill($("vault-files"), files, true);
  fill($("vault-answers"), answers, false);
  $("vault-files-empty").hidden = files.length > 0;
  $("vault-answers-empty").hidden = answers.length > 0;
  $("vault-ask").hidden = files.length === 0;
}

function fill(list, items, pickable) {
  list.replaceChildren();
  const template = $("vault-file-row");
  for (const row of items) {
    const item = template.content.firstElementChild.cloneNode(true);
    item.querySelector("[data-name]").textContent = row.name;
    const route = row.route === "device" ? " · Answered on this device" : row.route === "fast" ? " · Answered by Google Gemini" : "";
    item.querySelector("[data-meta]").textContent = `${size(row.size)} · ${when(row.created_at)}${route}`;
    const pick = item.querySelector("[data-pick]");
    pick.dataset.object = row.object;
    pick.setAttribute("aria-label", `Ask about ${row.name}`);
    if (!pickable) pick.parentElement.replaceWith(item.querySelector("[data-name]"));
    item.querySelector('[data-act="open"]').addEventListener("click", () => openFile(row, false));
    item.querySelector('[data-act="download"]').addEventListener("click", () => openFile(row, true));
    item.querySelector('[data-act="delete"]').addEventListener("click", () => removeFile(row));
    list.append(item);
  }
}

const METHODS = {
  "passkey-prf": "Passkey",
  device: "Device key",
  nostr: "Nostr key",
  recovery: "Recovery code",
  pairing: "Link for another device",
};

function renderDevices() {
  const list = $("vault-devices");
  list.replaceChildren();
  const template = $("vault-device-row");
  for (const slot of state.slots) {
    const item = template.content.firstElementChild.cloneNode(true);
    item.querySelector("[data-name]").textContent = slot.label;
    let meta = `${METHODS[slot.method] || slot.method} · added ${when(slot.created_at)}`;
    if (slot.method === "pairing") meta = `${METHODS.pairing} · works until ${new Date(slot.params.expires_at * 1000).toLocaleTimeString()}`;
    item.querySelector("[data-meta]").textContent = meta;
    const rest = state.slots.filter((other) => other.slot !== slot.slot);
    const button = item.querySelector('[data-act="remove"]');
    if (slot.method !== "pairing" && !wasm.enough(JSON.stringify(rest))) {
      button.remove();
    } else {
      button.addEventListener("click", () => removeSlot(slot));
    }
    list.append(item);
  }
}

async function addSlot(make) {
  clearError();
  try {
    const slot = await make();
    await api("POST", "/vault/api/slots", { slot });
    if (pairingSlot) {
      await api("POST", `/vault/api/slots/${pairingSlot}/delete`).catch(() => {});
      pairingSlot = null;
    }
    await fetchState();
    await openView();
    say(`Added ${slot.label}.`);
  } catch (error) {
    fail(error);
  }
}

async function removeSlot(slot) {
  if (!confirm(`Remove “${slot.label}”? It won't unlock your vault any more.`)) return;
  clearError();
  try {
    await api("POST", `/vault/api/slots/${slot.slot}/delete`);
    await fetchState();
    renderDevices();
    say(`Removed ${slot.label}.`);
  } catch (error) {
    fail(error);
  }
}

async function pair() {
  clearError();
  try {
    const made = JSON.parse(vault.pairingSlot(now(), 600));
    await api("POST", "/vault/api/slots", { slot: made.slot });
    const link = `${location.origin}/settings/vault#pair=${made.fragment}`;
    const qr = JSON.parse(wasm.qr(link));
    const svg = $("vault-qr");
    svg.replaceChildren();
    svg.setAttribute("viewBox", `-2 -2 ${qr.size + 4} ${qr.size + 4}`);
    const back = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    back.setAttribute("x", "-2");
    back.setAttribute("y", "-2");
    back.setAttribute("width", String(qr.size + 4));
    back.setAttribute("height", String(qr.size + 4));
    back.setAttribute("fill", "#fff");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", qr.path);
    path.setAttribute("fill", "#000");
    svg.append(back, path);
    $("vault-pair-link").value = link;
    $("vault-pair-box").hidden = false;
    await fetchState();
    renderDevices();
    say("Open the link on your other device within 10 minutes.");
  } catch (error) {
    fail(error);
  }
}

// ---- Answers ----

async function probeLocal() {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 1500);
  localModel = null;
  try {
    const response = await fetch(`${LOCAL}/v1/models`, {
      signal: controller.signal,
      mode: "cors",
      credentials: "omit",
      cache: "no-store",
      targetAddressSpace: "loopback",
    });
    if (response.ok) {
      // Only a Psionic server counts: the plaintext goes nowhere else.
      const models = await response.json();
      const psionic = ((models && models.data) || []).find((model) => model && model.owned_by === "psionic");
      localModel = psionic ? psionic.id : null;
    }
  } catch {
    localModel = null;
  } finally {
    clearTimeout(timer);
  }
  // Chrome asks before a site reaches apps on this computer; if the person
  // said no, say where to change it rather than asking them to start Psionic.
  let blocked = false;
  if (!localModel && navigator.permissions) {
    for (const name of ["loopback-network", "local-network-access"]) {
      try {
        if ((await navigator.permissions.query({ name })).state === "denied") blocked = true;
      } catch {
        // This browser doesn't know that permission.
      }
    }
  }
  const device = $("vault-route-device");
  device.disabled = !localModel;
  $("vault-route-device-off").hidden = !!localModel || blocked;
  $("vault-route-device-blocked").hidden = !!localModel || !blocked;
  if (localModel && !$("vault-route-fast").checked) device.checked = true;
  if (!localModel) device.checked = false;
}

function picked() {
  return [...document.querySelectorAll("#vault-files [data-pick]:checked")].map((box) => box.dataset.object);
}

function utf8(bytes) {
  return new TextDecoder("utf-8", { fatal: false }).decode(bytes);
}

const INSTRUCTIONS = "Answer the person's question from the files they shared in this message. If the files don't hold the answer, say so plainly. Keep the answer short and exact; quote figures as they appear.";

async function askDevice(question, files) {
  const parts = [question];
  for (const file of files) {
    if (safeType(file.plain, file.row.media) !== "text/plain; charset=utf-8") {
      throw new Error("On this device reads text files for now. Pick text files, or pick Fast for PDFs and images.");
    }
    parts.push(`File "${file.row.name}":\n${utf8(file.plain).slice(0, 60000)}`);
  }
  const response = await fetch(`${LOCAL}/v1/chat/completions`, {
    method: "POST",
    mode: "cors",
    credentials: "omit",
    headers: { "content-type": "application/json" },
    targetAddressSpace: "loopback",
    body: JSON.stringify({
      model: localModel,
      messages: [
        { role: "system", content: INSTRUCTIONS },
        { role: "user", content: parts.join("\n\n") },
      ],
      max_tokens: 512,
      temperature: 0,
      stream: false,
    }),
  });
  if (!response.ok) throw new Error("The model on this computer didn't answer. Check that Psionic is still running.");
  const reply = await response.json();
  const text = reply && reply.choices && reply.choices[0] && reply.choices[0].message && reply.choices[0].message.content;
  if (!text) throw new Error("The model on this computer answered nothing.");
  return text.trim();
}

async function askFast(question, files) {
  const response = await api("POST", "/vault/api/answer", {
    question,
    files: files.map((file) => ({ name: file.row.name, media: file.row.media || "", data: b64(file.plain) })),
  });
  return (await response.json()).answer;
}

async function ask() {
  clearError();
  const ids = picked();
  const question = $("vault-question").value.trim();
  const route = $("vault-route-device").checked ? "device" : $("vault-route-fast").checked ? "fast" : "";
  if (!ids.length) return fail("Tick one or more files to ask about.");
  if (ids.length > 4) return fail("Ask about four files at most at once.");
  if (!question) return fail("Type a question first.");
  if (!route) return fail("Pick who reads the files: On this device, or Fast.");
  const byId = Object.fromEntries(rows().map((row) => [row.object, row]));
  try {
    say(route === "device" ? "Opening the files and asking the model on this computer…" : "Opening the files and asking Google Gemini…");
    const files = [];
    for (const id of ids) files.push({ row: byId[id], plain: await fetchPlain(byId[id]) });
    const answer = route === "device" ? await askDevice(question, files) : await askFast(question, files);
    $("vault-answer-label").textContent =
      route === "device"
        ? "Answered on this device. The files didn't leave this computer."
        : "Answered by Google Gemini. Google saw these files to answer.";
    $("vault-answer-text").textContent = answer;
    $("vault-answer").hidden = false;
    $("vault-answer").dataset.route = route;
    say("Saving the answer to your vault…");
    const name = `Answer: ${question.slice(0, 80)}`;
    await store("answer", name, "text/plain", ids, route, new TextEncoder().encode(`${question}\n\n${answer}\n`));
    renderFiles();
    say("Answered. The answer is saved in your vault, locked like your files.");
  } catch (error) {
    fail(error);
  }
}

// ---- Delete and lock ----

async function deleteVault() {
  if (!confirm("Delete your vault and every file in it? Nobody can open them after this, including you.")) return;
  clearError();
  try {
    await api("POST", "/vault/api/delete");
    forget(state.id);
    if (vault) vault.free();
    vault = null;
    await fetchState();
    await setupView();
    say("Your vault is deleted.");
  } catch (error) {
    fail(error);
  }
}

function lock() {
  if (vault) vault.free();
  vault = null;
  $("vault-answer").hidden = true;
  $("vault-answer-text").textContent = "";
  $("vault-pair-box").hidden = true;
  lockedView();
  say("Locked.");
}

// ---- Start ----

function wire() {
  $("vault-setup-passkey").addEventListener("click", () => startSetup(async () => passkeySlotFor(await newPasskey())));
  $("vault-setup-nostr").addEventListener("click", () => startSetup(nostrSlot));
  $("vault-finish").addEventListener("click", finishSetup);
  $("vault-unlock-passkey").addEventListener("click", () => unlockWith(unlockPasskey));
  $("vault-unlock-nostr").addEventListener("click", () => unlockWith(unlockNostr));
  $("vault-unlock-recovery").addEventListener("click", () => unlockWith(async () => unlockRecovery()));
  $("vault-file").addEventListener("change", (event) => addFiles([...event.target.files]));
  $("vault-ask-go").addEventListener("click", ask);
  $("vault-add-passkey").addEventListener("click", () => addSlot(async () => passkeySlotFor(await newPasskey())));
  $("vault-add-nostr").addEventListener("click", () => addSlot(nostrSlot));
  $("vault-pair").addEventListener("click", pair);
  $("vault-pair-copy").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText($("vault-pair-link").value);
      say("Link copied.");
    } catch {
      $("vault-pair-link").select();
    }
  });
  $("vault-lock").addEventListener("click", lock);
  $("vault-delete").addEventListener("click", deleteVault);
  window.addEventListener("pagehide", () => {
    if (vault) vault.free();
    vault = null;
  });
}

async function start() {
  if (!window.crypto || !crypto.subtle || typeof WebAssembly === "undefined") {
    show("vault-unsupported");
    say("");
    return;
  }
  wire();
  try {
    await loadWasm();
    await fetchState();
  } catch (error) {
    fail(error);
    return;
  }
  if (!state) {
    await setupView();
    return;
  }
  lockedView();
  const fragment = location.hash.startsWith("#pair=") ? location.hash.slice(6) : "";
  if (fragment) await unlockPairing(fragment);
}

if (root && root.dataset.wasm) start();
