# Rust Cross-Platform Architecture Exploration
**Date:** November 11, 2025
**Context:** Considering pivot from Swift (iOS/macOS only) to Rust + Tauri (cross-platform)

## Stream of Consciousness Capture

> I actually think I want to go cross-platform. I think I want to go cross-platform from day one. I think I want to use Ollama so we can bootstrap the compute network because it's going to get me to network effect fast. I want to do Rust for full portability. I'll explore the Tauri mobile app options. I'll explore putting just Rust binaries inside of the native apps. Can I deploy a demo Tauri Rust app? Easiest for people to contribute: Tauri, React. Maayyybe Expo. FUCKING VERCEL AI SDK? Perhaps if the backend is Rust. OR. NAH. SIMPLEST FUCKING THING IS? OPENROUTER KEY SHAD UI TAURI No, definitely not Electron. Put more shit into Rust or?

## Core Motivations

### 1. Cross-Platform from Day One
**Problem:** Current Swift architecture locks us into Apple ecosystem only.
- iOS 16.0+, macOS 13.0+
- ~1.5 billion Apple users
- Excludes Windows (~1 billion), Linux (~50 million developers), Android (~3 billion)

**Opportunity:** Rust + Tauri reaches ALL platforms.
- Desktop: macOS, Windows, Linux
- Mobile: iOS, Android (Tauri mobile in beta)
- Web: WASM target

### 2. Network Effects via Ollama
**Problem:** Compute marketplace needs critical mass fast.
**Solution:** Ollama already has:
- 50k+ GitHub stars
- Active community
- Local model inference
- Simple API (`POST http://localhost:11434/api/generate`)
- Pre-trained models (Llama 3, Mistral, etc.)

**Benefit:** Bootstrap compute network by integrating with existing Ollama users.

### 3. Contributor Accessibility
**Problem:** Swift is niche compared to web technologies.
- Swift developers: ~1 million
- React developers: ~15 million
- Rust developers: ~2 million (but growing fast)

**Solution:** Tauri + React + Rust
- Frontend: React (accessible to millions)
- Backend: Rust (performance + safety)
- Desktop: Tauri (modern, secure, fast)

### 4. Phased Rollout Philosophy
**Start simple:** Just chat visualization.
- One clear thing: chat with coding agents from desktop app
- No orchestration, no plugins, no marketplace (yet)
- Prove the concept, then expand

---

## Architecture Options

### Option A: Full Tauri App (Recommended for Cross-Platform)

```
┌─────────────────────────────────────────────────────┐
│                   Tauri Desktop App                  │
│  ┌────────────────────────────────────────────────┐ │
│  │          Frontend (React + shadcn/ui)          │ │
│  │  - Chat timeline                               │ │
│  │  - Message rendering                           │ │
│  │  - Input composer                              │ │
│  │  - Settings UI                                 │ │
│  └────────────────────────────────────────────────┘ │
│                        ▼                             │
│  ┌────────────────────────────────────────────────┐ │
│  │         Tauri Commands (Rust Backend)          │ │
│  │  - Agent client protocol (ACP)                 │ │
│  │  - Session management                          │ │
│  │  - Tinyvex (SQLite wrapper)                    │ │
│  │  - Ollama integration                          │ │
│  └────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
                         ▼
        ┌────────────────────────────────┐
        │   External Agents (Codex, Claude)  │
        │   + Ollama (Local Inference)       │
        └────────────────────────────────┘
```

**Stack:**
- **Frontend:** React + TypeScript + shadcn/ui + Tailwind CSS
- **Backend:** Rust (Tauri commands)
- **Desktop:** Tauri v2 (macOS, Windows, Linux)
- **Mobile:** Tauri mobile (iOS, Android) - beta, but usable
- **Database:** SQLite via rusqlite
- **LLM:** Ollama (local) + OpenRouter (cloud)

**Pros:**
- ✅ Cross-platform from day one (macOS, Windows, Linux)
- ✅ Mobile in beta (can ship iOS/Android later)
- ✅ React ecosystem (shadcn/ui, Vercel AI SDK)
- ✅ Large contributor pool (React + Rust developers)
- ✅ Ollama integration is straightforward (HTTP API)
- ✅ Modern, secure (no Electron bloat)
- ✅ Fast (Rust backend, no IPC overhead)

**Cons:**
- ⚠️ Tauri mobile is beta (may have rough edges)
- ⚠️ Less "native" feel than SwiftUI
- ⚠️ Foundation Models (Apple Intelligence) harder to integrate
- ⚠️ Rebuilding existing Swift codebase (~25k LOC)

**Estimated Effort:**
- **Phase 1 (Chat MVP):** 4-6 weeks
  - Basic Tauri app with React frontend
  - ACP client in Rust
  - Ollama integration
  - Simple chat timeline
- **Phase 2 (Feature Parity):** 8-12 weeks
  - Session management, history
  - Settings, preferences
  - Orchestration (if needed)

---

### Option B: Rust Core + Native UIs

```
┌─────────────────────────────────────────────────────┐
│              Rust Core Library (Shared)              │
│  - Agent Client Protocol (ACP)                       │
│  - Session management                                │
│  - Tinyvex (SQLite)                                  │
│  - Ollama client                                     │
│  - Orchestration logic                               │
└─────────────────────────────────────────────────────┘
           ▼                ▼                 ▼
    ┌──────────┐    ┌──────────┐     ┌──────────┐
    │ SwiftUI  │    │  Tauri   │     │  Flutter │
    │ (iOS/Mac)│    │ (Desktop)│     │ (Mobile) │
    └──────────┘    └──────────┘     └──────────┘
```

**Stack:**
- **Core:** Rust (compiled to static library)
- **iOS/macOS:** Swift + SwiftUI (calls into Rust via FFI)
- **Desktop:** Tauri + React (calls into Rust via Tauri commands)
- **Android:** Kotlin/Flutter (calls into Rust via FFI)

**Pros:**
- ✅ Best of both worlds (native UIs + shared logic)
- ✅ Keep existing Swift UI investment
- ✅ Cross-platform via shared Rust core
- ✅ Foundation Models work on iOS/macOS

**Cons:**
- ⚠️ **Complex:** Maintaining 3+ UI codebases
- ⚠️ FFI overhead and complexity
- ⚠️ Debugging across language boundaries is hard
- ⚠️ Slow iteration (changes require coordinating multiple codebases)

**Estimated Effort:**
- **Phase 1 (Rust Core):** 8-10 weeks
  - Extract business logic to Rust
  - FFI bindings for Swift
  - Migrate incrementally
- **Phase 2 (Tauri UI):** 4-6 weeks
  - Build Tauri app using Rust core
- **Total:** 12-16 weeks (slower than Option A)

---

### Option C: Hybrid (Current Swift + Tauri Coexist)

```
┌─────────────────────┐       ┌─────────────────────┐
│   Swift App (v0.3)  │       │   Tauri App (v0.4)  │
│   - iOS/macOS       │       │   - Cross-platform  │
│   - Native UX       │       │   - React UI        │
│   - Foundation Models│      │   - Ollama         │
└─────────────────────┘       └─────────────────────┘
           ▼                             ▼
    ┌──────────────────────────────────────┐
    │  Shared WebSocket Bridge (Existing)  │
    │  - JSON-RPC 2.0                      │
    │  - ACP over WebSocket                │
    └──────────────────────────────────────┘
```

**Stack:**
- **Existing:** Keep Swift app for iOS/macOS users
- **New:** Build Tauri app for Windows/Linux users
- **Bridge:** Both apps talk to same WebSocket server

**Pros:**
- ✅ Keep existing Swift investment
- ✅ Expand reach with Tauri
- ✅ Gradual migration (low risk)
- ✅ iOS/macOS users get native experience

**Cons:**
- ⚠️ Maintaining two codebases (Swift + Tauri)
- ⚠️ Feature parity challenges
- ⚠️ More testing surface area

**Estimated Effort:**
- **Phase 1 (Tauri App):** 6-8 weeks
  - Build Tauri app from scratch
  - Connect to existing WebSocket bridge
- **Ongoing:** Maintain feature parity

---

## Technology Deep Dive

### Tauri vs Electron

| Feature | Tauri | Electron |
|---------|-------|----------|
| **Bundle Size** | 3-5 MB | 50-100 MB |
| **Memory** | 50-100 MB | 200-400 MB |
| **Backend** | Rust | Node.js |
| **Security** | Sandboxed by default | Requires manual hardening |
| **Native APIs** | Easy via Rust crates | Node bindings |
| **Startup Time** | < 1 second | 2-5 seconds |
| **Ecosystem** | Growing | Mature |

**Verdict:** Tauri is clearly superior for desktop apps. Electron is bloated, slow, and insecure by default.

---

### Ollama Integration

**Ollama API Example:**
```bash
# Generate text
curl http://localhost:11434/api/generate -d '{
  "model": "llama3",
  "prompt": "Why is the sky blue?",
  "stream": false
}'

# List models
curl http://localhost:11434/api/tags
```

**Rust Integration:**
```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

async fn generate(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let req = GenerateRequest {
        model: "llama3".to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let res: GenerateResponse = client
        .post("http://localhost:11434/api/generate")
        .json(&req)
        .send()
        .await?
        .json()
        .await?;

    Ok(res.response)
}
```

**Benefits:**
- ✅ Simple HTTP API (no complex SDKs)
- ✅ Models run locally (privacy-preserving)
- ✅ Large model library (Llama 3, Mistral, Gemma, etc.)
- ✅ Active community (50k+ stars)
- ✅ Compute marketplace synergy (users already running Ollama)

---

### React + shadcn/ui + Vercel AI SDK

**Stack:**
- **shadcn/ui:** Beautiful, accessible components (built on Radix UI + Tailwind)
- **Vercel AI SDK:** Stream AI responses, handle loading states, error recovery
- **React Query:** Server state management

**Example Chat UI:**
```tsx
import { useChat } from '@ai-sdk/react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'

export function ChatInterface() {
  const { messages, input, handleInputChange, handleSubmit } = useChat({
    api: '/api/chat', // Tauri command endpoint
  })

  return (
    <div className="flex flex-col h-screen">
      <ScrollArea className="flex-1 p-4">
        {messages.map((message) => (
          <MessageBubble key={message.id} message={message} />
        ))}
      </ScrollArea>

      <form onSubmit={handleSubmit} className="p-4 border-t">
        <div className="flex gap-2">
          <Input
            value={input}
            onChange={handleInputChange}
            placeholder="Ask your agent..."
          />
          <Button type="submit">Send</Button>
        </div>
      </form>
    </div>
  )
}
```

**Tauri Backend:**
```rust
#[tauri::command]
async fn chat(prompt: String) -> Result<String, String> {
    // Call Ollama
    generate(&prompt).await.map_err(|e| e.to_string())
}
```

**Benefits:**
- ✅ Vercel AI SDK handles streaming, loading states
- ✅ shadcn/ui is beautiful and accessible
- ✅ React ecosystem is massive (easy to hire/contribute)
- ✅ Tauri commands are simple (just annotate Rust functions)

---

## Trade-Off Analysis

### Swift vs Rust + Tauri

| Dimension | Swift (Current) | Rust + Tauri |
|-----------|----------------|--------------|
| **Platforms** | iOS, macOS only | macOS, Windows, Linux, (iOS/Android beta) |
| **Reach** | ~1.5B Apple users | ~5B desktop/mobile users |
| **Native Feel** | ⭐⭐⭐⭐⭐ (SwiftUI) | ⭐⭐⭐ (Web-based UI) |
| **Performance** | ⭐⭐⭐⭐⭐ (Native) | ⭐⭐⭐⭐ (Rust + WebView) |
| **Contributor Pool** | ~1M Swift devs | ~15M React + 2M Rust devs |
| **Foundation Models** | ⭐⭐⭐⭐⭐ (Native) | ⭐⭐ (Via plugin/FFI) |
| **Ollama Integration** | ⭐⭐⭐ (HTTP API) | ⭐⭐⭐⭐⭐ (Native Rust) |
| **App Store** | Easy (native) | Possible (web-based) |
| **Code Reuse** | iOS ↔ macOS | All platforms |
| **Learning Curve** | Medium (Swift) | Medium-High (Rust + React) |
| **Maturity** | ⭐⭐⭐⭐⭐ (Stable) | ⭐⭐⭐⭐ (Tauri v2 stable, mobile beta) |

---

## Decision Framework

### Ask These Questions:

#### 1. Who is the primary audience?
- **Apple users only?** → Swift is fine
- **All desktop users?** → Tauri strongly recommended
- **Mobile-first?** → Consider React Native or Flutter

#### 2. What is the MVP?
- **Chat visualization only?** → Tauri is faster to MVP
- **Full agent IDE?** → Either works, but Tauri reaches more users

#### 3. What is the network effect strategy?
- **Apple ecosystem?** → Swift + Foundation Models
- **Ollama compute network?** → Rust + Tauri (easier integration)

#### 4. What is the contributor strategy?
- **Small core team?** → Swift (simpler, one codebase)
- **Open source community?** → Tauri + React (more contributors)

#### 5. What is the business model?
- **App Store sales?** → Swift (easier approval)
- **Subscription SaaS?** → Either (web backend)
- **Compute marketplace?** → Tauri (broader reach)

---

## Recommended Path: Pragmatic Hybrid

### Phase 0: Validate Tauri Quickly (1 week)
**Goal:** Can we ship a basic Tauri app that feels good?

**Tasks:**
1. Create minimal Tauri app with React + shadcn/ui
2. Integrate with Ollama (local inference)
3. Build basic chat timeline
4. Deploy to macOS, Windows, Linux
5. Get feedback from 5-10 users

**Decision Point:** If Tauri feels good, proceed to Phase 1. If not, stick with Swift.

---

### Phase 1: Chat MVP (Tauri) (4-6 weeks)
**Goal:** Ship a basic cross-platform chat app.

**Scope:**
- ✅ Chat with Codex, Claude Code, Ollama
- ✅ Session history (Tinyvex/SQLite)
- ✅ Settings (API keys, model selection)
- ❌ Orchestration (later)
- ❌ Plugins (later)
- ❌ Marketplace (later)

**Stack:**
- Tauri v2 (desktop: macOS, Windows, Linux)
- React + TypeScript + shadcn/ui
- Rust backend (ACP client, Tinyvex, Ollama)
- OpenRouter for cloud models (fallback)

**Deliverable:** Cross-platform desktop app for chatting with coding agents.

---

### Phase 2: Mobile (iOS First) (4-6 weeks)
**Goal:** Ship iOS app using Tauri mobile.

**Approach:**
- Use Tauri mobile (beta)
- Reuse React UI (with mobile-specific tweaks)
- Reuse Rust backend (100% shared)
- Ship to TestFlight

**Decision Point:** If Tauri mobile is too rough, fall back to Swift for iOS only.

---

### Phase 3: Advanced Features (8-12 weeks)
**Goal:** Add orchestration, plugins, marketplace.

**Scope:**
- Overnight orchestration
- Plugin system (WASM plugins in Rust)
- Marketplace (agent discovery)
- Payments (Stripe)

**Reference:** Follow roadmap from `refactoring-roadmap.md`, but implement in Rust + Tauri.

---

## Implementation Strategy

### Minimal Tauri Chat App (Week 1)

**File Structure:**
```
openagents-tauri/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Tauri app entry
│   │   ├── commands.rs     # Tauri commands (chat, history, settings)
│   │   ├── acp/            # ACP client
│   │   ├── tinyvex/        # SQLite wrapper
│   │   └── ollama.rs       # Ollama client
│   └── Cargo.toml
├── src/                    # React frontend
│   ├── App.tsx
│   ├── components/
│   │   ├── ChatTimeline.tsx
│   │   ├── MessageBubble.tsx
│   │   └── Composer.tsx
│   └── lib/
│       └── tauri.ts        # Tauri invoke wrappers
└── package.json
```

**Tauri Commands:**
```rust
#[tauri::command]
async fn send_message(prompt: String) -> Result<String, String> {
    // Call Ollama or OpenRouter
    ollama::generate(&prompt).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_history() -> Result<Vec<Session>, String> {
    // Query Tinyvex
    tinyvex::query_sessions().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(settings: Settings) -> Result<(), String> {
    // Save to SQLite
    tinyvex::save_settings(settings).await.map_err(|e| e.to_string())
}
```

**React Frontend:**
```tsx
import { invoke } from '@tauri-apps/api/tauri'

async function sendMessage(prompt: string) {
  return await invoke<string>('send_message', { prompt })
}

export function ChatInterface() {
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState('')

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const userMessage = { role: 'user', content: input }
    setMessages([...messages, userMessage])
    setInput('')

    const response = await sendMessage(input)
    const assistantMessage = { role: 'assistant', content: response }
    setMessages([...messages, userMessage, assistantMessage])
  }

  return (
    <div className="flex flex-col h-screen">
      <ScrollArea className="flex-1 p-4">
        {messages.map((msg, i) => (
          <MessageBubble key={i} message={msg} />
        ))}
      </ScrollArea>

      <form onSubmit={handleSubmit} className="p-4 border-t">
        <Input value={input} onChange={(e) => setInput(e.target.value)} />
        <Button type="submit">Send</Button>
      </form>
    </div>
  )
}
```

---

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Tauri mobile too immature | Medium | High | Fall back to Swift for iOS |
| Rust learning curve | Medium | Medium | Start simple, hire Rust expert |
| WebView performance on low-end devices | Low | Medium | Optimize bundle size, lazy load |
| Ollama adoption lower than expected | Low | High | Support OpenRouter as fallback |

### Business Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Windows/Linux market smaller than expected | Medium | Low | Focus on macOS first, expand later |
| React UI feels "un-native" | Medium | Medium | Use native system dialogs, follow OS patterns |
| Rebuilding delays feature roadmap | High | High | Ship MVP fast, iterate |

### Strategic Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Fragmenting focus (Swift + Tauri) | High | High | Commit to one or the other |
| Losing Apple-specific advantages (Foundation Models) | Medium | Medium | Build plugin for Foundation Models via Swift bridge |

---

## Recommendation

### For Cross-Platform: Go Tauri (Option A)

**Why:**
1. ✅ **Reach:** 5B desktop/mobile users vs 1.5B Apple users
2. ✅ **Network Effects:** Ollama integration bootstraps compute marketplace
3. ✅ **Contributors:** React + Rust >> Swift
4. ✅ **Speed:** MVP in 4-6 weeks (vs 8-12 weeks for Rust core + native UIs)
5. ✅ **Modern:** Tauri is the future (Electron is dead)

**Trade-offs Accepted:**
- ⚠️ Less "native" feel than SwiftUI (but shadcn/ui is beautiful)
- ⚠️ Rebuilding existing Swift codebase (but it's only ~25k LOC, and much is foundation)
- ⚠️ Foundation Models harder to integrate (but can build Swift plugin later)

**Approach:**
1. **Week 1:** Validate Tauri with minimal chat app
2. **Weeks 2-6:** Build Chat MVP (Tauri + React + Ollama)
3. **Weeks 7-12:** Add iOS via Tauri mobile (or Swift fallback)
4. **Weeks 13+:** Advanced features (orchestration, plugins, marketplace)

---

### For Apple Ecosystem: Stick with Swift

**Why:**
1. ✅ **Investment:** ~25k LOC Swift already built
2. ✅ **Native Feel:** SwiftUI is unmatched
3. ✅ **Foundation Models:** Apple Intelligence integration is seamless
4. ✅ **App Store:** Native apps are easier to approve

**Trade-offs Accepted:**
- ⚠️ iOS/macOS only (excludes 70% of potential users)
- ⚠️ Smaller contributor pool
- ⚠️ Ollama integration requires HTTP client (not native Rust)

**Approach:**
- Continue with roadmap in `refactoring-roadmap.md`
- Add Windows/Linux later via separate Tauri app (Option C)

---

## Next Steps

### Decision Required:
1. **Validate Tauri:** Spend 1 week building minimal chat app
2. **Get Feedback:** Show to 5-10 potential users
3. **Commit:** Choose Tauri (cross-platform) or Swift (Apple-only)

### If Tauri:
1. Archive current Swift codebase (tag v0.3 as "Swift Edition")
2. Start fresh Tauri project
3. Port ACP client to Rust
4. Build React UI with shadcn/ui
5. Integrate Ollama
6. Ship MVP in 4-6 weeks

### If Swift:
1. Continue with current roadmap
2. Add Ollama support via HTTP client
3. Build plugins/marketplace/payments
4. Ship v0.4 in 11-17 weeks

---

## Appendix: Inspirational Examples

### Tauri Apps in Production
- **Supabase Studio** - Database GUI (Tauri + React)
- **Clash Verge** - Clash GUI client (Tauri + React)
- **GitButler** - Git client (Tauri + Svelte)
- **AppFlowy** - Notion alternative (Tauri + Flutter)

### Rust + React Success Stories
- **Zed** - Code editor (Rust + GPUI, but React for settings)
- **Warp** - Terminal (Rust + React)
- **Turbo** - Vercel's build system (Rust)

---

## Final Thought

The choice is between:
- **Swift:** Best user experience, limited reach
- **Tauri:** Wider reach, good-enough user experience

For a **10x better agent IDE** targeting **network effects** and **open source community**, **Tauri is the right bet**.

The current Swift codebase is excellent, but it's v0.3 — early enough to pivot without massive sunk cost.

**Recommendation: Go Tauri. Ship chat MVP in 4-6 weeks. Win the cross-platform market.**

---

**Document Version:** 1.0
**Last Updated:** November 11, 2025
**Decision Deadline:** After 1-week Tauri validation (Week of November 18, 2025)
