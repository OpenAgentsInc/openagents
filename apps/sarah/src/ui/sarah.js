/**
 * Sarah voice surface — zero React.
 * SM-2: DOM renderer path; Effect Native component set is the long-term home
 * (gaps filed against EN audio/streaming class). Typed intents for UI state.
 */

const API = "/sarah/api"

/** @typedef {{ type: 'ui/status'; state: string }} StatusIntent */
/** @typedef {{ type: 'ui/transcript'; role: 'user'|'assistant'; text: string; modality: 'text'|'voice' }} TranscriptIntent */
/** @typedef {{ type: 'ui/level'; value: number }} LevelIntent */
/** @typedef {{ type: 'ui/card'; title: string; body: string }} CardIntent */
/** @typedef {StatusIntent|TranscriptIntent|LevelIntent|CardIntent} SarahUiIntent */

const state = {
  prospectRef: null,
  threadId: null,
  live: false,
  mediaStream: null,
  analyser: null,
  raf: 0,
}

const els = {
  status: document.getElementById("status"),
  transcript: document.getElementById("transcript"),
  mic: document.getElementById("mic"),
  micLabel: document.getElementById("mic-label"),
  levelBar: document.getElementById("level-bar"),
  form: document.getElementById("text-form"),
  input: document.getElementById("text-input"),
  cards: document.getElementById("cards"),
}

/** @param {SarahUiIntent} intent */
function dispatch(intent) {
  switch (intent.type) {
    case "ui/status":
      els.status.dataset.state = intent.state
      els.status.textContent = intent.state
      break
    case "ui/transcript": {
      const turn = document.createElement("article")
      turn.className = `turn ${intent.role}`
      turn.innerHTML = `<span class="meta">${intent.role} · ${intent.modality}</span>${escapeHtml(intent.text)}`
      els.transcript.appendChild(turn)
      els.transcript.scrollTop = els.transcript.scrollHeight
      break
    }
    case "ui/level":
      els.levelBar.style.width = `${Math.max(0, Math.min(100, intent.value * 100))}%`
      break
    case "ui/card": {
      const card = document.createElement("div")
      card.className = "card"
      card.innerHTML = `<h3>${escapeHtml(intent.title)}</h3><p>${escapeHtml(intent.body)}</p>`
      els.cards.prepend(card)
      break
    }
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
}

async function ensureSession() {
  if (state.prospectRef) return
  const res = await fetch(`${API}/prospect/session`, { method: "POST" })
  const data = await res.json()
  state.prospectRef = data.prospectRef
  state.threadId = data.threadId
}

async function sendText(message) {
  await ensureSession()
  dispatch({ type: "ui/transcript", role: "user", text: message, modality: "text" })
  dispatch({ type: "ui/status", state: "thinking" })
  const res = await fetch(`${API}/eve/turn`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      message,
      threadId: state.threadId,
      prospectRef: state.prospectRef,
    }),
  })
  const data = await res.json()
  dispatch({
    type: "ui/transcript",
    role: "assistant",
    text: data.reply || data.error || "No reply",
    modality: "text",
  })
  if (data.modelPath === "google_gemma_live") {
    dispatch({
      type: "ui/card",
      title: "Runtime",
      body: `Owned runtime · live model: ${data.model || "gemma-4"}`,
    })
  } else if (data.runtime && data.modelPath === "seed_echo") {
    dispatch({
      type: "ui/card",
      title: "Runtime",
      body: `Owned runtime · model not armed (${data.modelError || "seed echo"})`,
    })
  }
  dispatch({ type: "ui/status", state: state.live ? "live" : "idle" })
}

async function mintRealtimeToken() {
  const res = await fetch(`${API}/realtime/token`, { method: "POST" })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err?.error?.message || `token mint failed (${res.status})`)
  }
  return res.json()
}

function stopLevelMeter() {
  if (state.raf) cancelAnimationFrame(state.raf)
  state.raf = 0
  dispatch({ type: "ui/level", value: 0 })
}

function startLevelMeter(stream) {
  const ctx = new AudioContext()
  const source = ctx.createMediaStreamSource(stream)
  const analyser = ctx.createAnalyser()
  analyser.fftSize = 256
  source.connect(analyser)
  state.analyser = analyser
  const data = new Uint8Array(analyser.frequencyBinCount)
  const tick = () => {
    analyser.getByteFrequencyData(data)
    let sum = 0
    for (const n of data) sum += n
    const avg = sum / (data.length * 255)
    dispatch({ type: "ui/level", value: avg })
    state.raf = requestAnimationFrame(tick)
  }
  tick()
}

async function startVoice() {
  await ensureSession()
  dispatch({ type: "ui/status", state: "arming" })
  try {
    const tokenPayload = await mintRealtimeToken()
    state.mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true })
    startLevelMeter(state.mediaStream)
    state.live = true
    els.mic.dataset.state = "live"
    els.mic.setAttribute("aria-pressed", "true")
    els.micLabel.textContent = "Stop voice"
    dispatch({ type: "ui/status", state: "live" })
    dispatch({
      type: "ui/card",
      title: "Realtime armed",
      body: tokenPayload.url
        ? "Browser holds the provider token; media stays client-side (S-3 path)."
        : "Realtime token minted.",
    })
    // Full WebRTC attach stays provider-direct; this shell proves the monorepo
    // token + mic + VAD-level UX without React.
  } catch (error) {
    dispatch({ type: "ui/status", state: "error" })
    dispatch({
      type: "ui/transcript",
      role: "assistant",
      text:
        error instanceof Error
          ? error.message
          : "Could not start voice. You can still use text.",
      modality: "text",
    })
  }
}

function stopVoice() {
  state.live = false
  els.mic.dataset.state = "idle"
  els.mic.setAttribute("aria-pressed", "false")
  els.micLabel.textContent = "Start voice"
  stopLevelMeter()
  if (state.mediaStream) {
    for (const track of state.mediaStream.getTracks()) track.stop()
    state.mediaStream = null
  }
  dispatch({ type: "ui/status", state: "idle" })
}

els.mic.addEventListener("click", () => {
  if (state.live) stopVoice()
  else void startVoice()
})

els.form.addEventListener("submit", (event) => {
  event.preventDefault()
  const value = els.input.value.trim()
  if (!value) return
  els.input.value = ""
  void sendText(value)
})

dispatch({
  type: "ui/transcript",
  role: "assistant",
  text: "I'm Sarah, an AI sales assistant for OpenAgents. Ask about the product, or start voice when you're ready.",
  modality: "text",
})

void ensureSession().catch(() => {
  dispatch({ type: "ui/status", state: "error" })
})
