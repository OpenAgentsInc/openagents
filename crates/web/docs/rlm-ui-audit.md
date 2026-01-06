# RLM Execution Visualizer - UI Audit

Current state documentation for redesign consideration.

## Screenshots Analyzed

- Screenshot 1: "STREAMING" state (during execution)
- Screenshot 2: "COMPLETE" state (after execution)

---

## Current Layout Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ HEADER                                                                       │
│ ┌─────────────────────────────────────────────────────────────┬────────────┐│
│ │ RLM EXECUTION VISUALIZER                                    │ [STATUS]   ││
│ │ DSPy-Powered Document Analysis: Route -> Extract -> Reduce  │ STREAMING/ ││
│ │                                                             │ COMPLETE   ││
│ └─────────────────────────────────────────────────────────────┴────────────┘│
├─────────────────────────────────────────────────────────────────────────────┤
│ INPUT SECTION                                                                │
│ ┌───────────────────────────────────────────────────────────────┬──────────┐│
│ │ Query: [What are the main components of the Repo-RLM...]     │ [RUN/    ││
│ │                                                               │  STOP]   ││
│ ├───────────────────────────────────────────────────────────────┴──────────┤│
│ │ Document:                                                                 ││
│ │ ┌───────────────────────────────────────────────────────────────────────┐││
│ │ │ ## 0) What we're building                                             │││
│ │ │ ### External contract (drop-in)                                       │││
│ │ │ ```text                                                               │││
│ │ │ answer = repo_rlm(prompt: String) -> String                           │││
│ │ │ ```                                                                   │││
│ │ │ ## # Internal reality                                                 │││
│ │ │ * prompt stays small (task/question)                                  │││
│ │ │ * repo is mounted into a sandbox                                      │││
│ │ │ * the runtime exposes a **tool API** (list/read/grep/symbols/diff)    │││
│ │ └───────────────────────────────────────────────────────────────────────┘││
│ └───────────────────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────────────────┤
│ TIMELINE BAR                                                                 │
│ ┌───────────────────────────────────────────────────────────────────────────┐│
│ │     Route          Chunk          Extract          Reduce                 ││
│ │ ════●══════════════●══════════════●════════════════●═════════════════════ ││
│ │     ■              ■ ■            ■ ■ ■ ■ ■ ■ ■ ■  ■          Chunks: 8/8 ││
│ └───────────────────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────────────────┤
│ MAIN CONTENT AREA                                                            │
│ ┌─────────────────────────────┬─────────────────────────────────────────────┐│
│ │ LEFT PANEL                  │ RIGHT PANEL                                 ││
│ │ Phase: Reduce/Complete      │ DETAIL VIEW                                 ││
│ │                             │                                             ││
│ │ EXECUTION PHASES            │ Section: Module Signatures                  ││
│ │ = Routing                   │ RouterSignature: query + hints -> ...       ││
│ │ = Chunking                  │ ExtractorSignature: query + span + text ... ││
│ │ = Extraction (CoT)          │ ReducerSignature: query + facts -> ...      ││
│ │ > Reduce + Verify           │ VerifierSignature: query + answer -> ...    ││
│ │   Combining and validating  │                                             ││
│ │                             │ Findings:                                   ││
│ │                             │ Four typed module signatures: Router...     ││
│ │                             │                                             ││
│ │                             ├─────────────────────────────────────────────┤│
│ │                             │ FINAL ANSWER                                ││
│ │                             │ ┌─────────────────────────────────────────┐ ││
│ │                             │ │ The Repo-RLM runtime spec consists of  │ ││
│ │                             │ │ 6 main components:                     │ ││
│ │                             │ │                                        │ ││
│ │                             │ │ 1. **External Contract**: String-in/   │ ││
│ │                             │ │    string-out API for LLM compat...    │ ││
│ │                             │ │ 2. **Repository Environment**: Repo    │ ││
│ │                             │ │    handle with commit pinning...       │ ││
│ │                             │ │ 3. **Tool API**: Minimal orthogonal    │ ││
│ │                             │ │    primitives - file discovery...      │ ││
│ │                             │ │ 4. **Program Graph**: DSPy-style       │ ││
│ │                             │ │    modular pipeline - Router...        │ ││
│ │                             │ │ 5. **Budget System**: Enforces         │ ││
│ │                             │ │    max_wall_ms, max_tool_calls...      │ ││
│ │                             │ │ 6. **Tracing & Provenance**: JSONL     │ ││
│ │                             │ │    event logging (ToolCall, LimCall... │ ││
│ │                             │ │                                        │ ││
│ │                             │ │ The design achieves out-of-core repo   │ ││
│ │                             │ │ access while maintaining safety...     │ ││
│ │                             │ └─────────────────────────────────────────┘ ││
│ └─────────────────────────────┴─────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Element-by-Element Breakdown

### 1. Header Section

| Element | Content | Style |
|---------|---------|-------|
| Title | "RLM EXECUTION VISUALIZER" | Green text, large, all caps |
| Subtitle | "DSPy-Powered Document Analysis: Route -> Extract -> Reduce -> Verify" | Gray text, smaller |
| Status Badge | "STREAMING" or "COMPLETE" | Cyan/green background, top-right corner |

### 2. Input Section

| Element | Content | Style |
|---------|---------|-------|
| Query Label | "Query:" | Gray text, left-aligned |
| Query Input | Text field with demo query | Dark background, monospace |
| Document Label | "Document:" | Gray text |
| Document Preview | Multi-line markdown preview | Dark box, ~6 lines visible |
| Action Button | "RUN" / "STOP" | Cyan border button, top-right |

### 3. Timeline Bar

| Element | Description | Issues |
|---------|-------------|--------|
| Phase Labels | "Route", "Chunk", "Extract", "Reduce" | Small text above bar, hard to read |
| Progress Bar | Horizontal green/orange bar | Shows progress left-to-right |
| Phase Dots | Small squares under each phase | Meaning unclear (gray/green) |
| Chunk Counter | "Chunks: 8/8" | Right-aligned, shows progress |

**Visual Issues with Timeline:**
- The text "Route", "Chunk", etc. overlaps with document content above
- Phase dots are too small to distinguish
- Progress bar color meaning not obvious
- "Chunks: 8/8" only relevant during Extract phase

### 4. Left Panel - Execution Phases

| Element | Content | Style |
|---------|---------|-------|
| Phase Label | "Phase: Reduce" or "Phase: Complete" | Overlaps with other text |
| Section Title | "EXECUTION PHASES" | Green text, caps |
| Phase List | 4 phases with status markers | Colored text (green/orange) |

**Phase markers:**
- `=` = completed (green)
- `>` = active (orange)
- ` ` = pending (gray)

**Phase descriptions shown:**
- "Routing"
- "Chunking"
- "Extraction (CoT)"
- "Reduce + Verify" with sub-text "Combining and validating"

### 5. Right Panel - Detail View

| Section | Content |
|---------|---------|
| Header | "DETAIL VIEW" (green) |
| Section Name | "Section: Module Signatures" |
| Section Content | Long text describing signatures (truncated, runs off screen) |
| Findings Label | "Findings:" (orange/yellow) |
| Findings Text | Extracted insights (also truncated) |

### 6. Final Answer Section

| Element | Content | Style |
|---------|---------|-------|
| Header | "FINAL ANSWER" | Green text |
| Answer Box | Bordered box with numbered list | Cyan border, dark background |
| Content | 6-item list + summary paragraph | Markdown-formatted text |

---

## Identified UX Problems

### Critical Issues

1. **Text Overlap/Collision**
   - "Phase: Reduce## # Internal reality" - phase label crashes into document text
   - Timeline labels overlap with document preview
   - Multiple z-index issues

2. **Truncation Without Scroll**
   - Section content in Detail View runs off right edge
   - Findings text similarly truncated
   - No horizontal scroll or text wrap

3. **Information Hierarchy Unclear**
   - Hard to understand what to look at first
   - Status badge competes with title
   - Multiple "sections" with similar styling

4. **Timeline Bar Confusion**
   - What do the small squares mean?
   - Why are some green vs gray?
   - Relationship between bar progress and dots unclear

5. **Dense Layout**
   - Too much information visible at once
   - No clear visual separation between sections
   - Monospace font everywhere makes scanning difficult

### Medium Issues

6. **No Loading States for Individual Chunks**
   - Can't tell which chunk is being processed
   - Chunk grid in left panel is mentioned in docs but not clearly visible

7. **Query/Document Fields Non-Interactive**
   - Fields look editable but are display-only in demo mode
   - No visual distinction between input vs display

8. **Color Coding Inconsistent**
   - Green used for: titles, completed phases, status badge
   - Cyan used for: borders, buttons, some text
   - Orange used for: active phase, findings label
   - No legend or explanation

9. **No Visual Feedback on Progress**
   - During streaming, hard to tell what's happening
   - Timeline moves but feels disconnected from content

---

## Alternative Design Directions

### Option A: Focus Mode (Single-Pane Progressive Reveal)

```
┌────────────────────────────────────────────┐
│ Query: What are the main components...     │
├────────────────────────────────────────────┤
│                                            │
│   ┌──────────────────────────────────┐     │
│   │     ROUTING                      │     │
│   │     Finding relevant sections... │     │
│   │     ████████░░░░░░░░░░░░░░░░░░░░ │     │
│   └──────────────────────────────────┘     │
│                                            │
│   Selected: 8 sections                     │
│   ─────────────────────────────────────    │
│   □ External Contract                      │
│   □ Repository Environment                 │
│   □ Tool API                               │
│   ...                                      │
│                                            │
└────────────────────────────────────────────┘
```

**Pros:** Clear focus, one thing at a time, easier to follow
**Cons:** Less "at a glance" overview, longer to see full picture

### Option B: Vertical Timeline (Scroll-Based)

```
┌────────────────────────────────────────────┐
│ ROUTE ────────────────────────── ✓ 0.4s   │
│   Selected 8 candidate sections            │
├────────────────────────────────────────────┤
│ CHUNK ────────────────────────── ✓ 0.2s   │
│   Split into 8 semantic chunks             │
├────────────────────────────────────────────┤
│ EXTRACT ─────────────────────── ✓ 8.2s   │
│ ┌─────────────────────────────────────┐    │
│ │ 1. External Contract         ✓ 0.8s │    │
│ │    "String-in/string-out API..."    │    │
│ ├─────────────────────────────────────┤    │
│ │ 2. Repository Environment    ✓ 1.0s │    │
│ │    "Repo handle with commit..."     │    │
│ └─────────────────────────────────────┘    │
├────────────────────────────────────────────┤
│ REDUCE ──────────────────────── ✓ 2.3s   │
│   Combined 8 extractions into answer       │
├────────────────────────────────────────────┤
│ FINAL ANSWER                               │
│ ┌─────────────────────────────────────┐    │
│ │ The Repo-RLM runtime spec...        │    │
│ └─────────────────────────────────────┘    │
└────────────────────────────────────────────┘
```

**Pros:** Natural reading order, clear timing, expandable sections
**Cons:** Requires scrolling, less "dashboard" feel

### Option C: Card-Based Chunks (Grid Focus)

```
┌────────────────────────────────────────────────────────┐
│ Query: [________________________] [RUN]    ● STREAMING │
├────────────────────────────────────────────────────────┤
│ ROUTING → CHUNKING → EXTRACTING (3/8) → REDUCE        │
│ ═══════════════════●══════════════════════════════════│
├────────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐   │
│ │ External │ │ Repo Env │ │ Tool API │ │ Program  │   │
│ │ Contract │ │          │ │ ████░░░░ │ │ Graph    │   │
│ │    ✓     │ │    ✓     │ │ working  │ │   ○      │   │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘   │
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐   │
│ │ Budget   │ │ Tracing  │ │ Module   │ │ Summary  │   │
│ │ System   │ │ & Prov   │ │ Sigs     │ │          │   │
│ │    ○     │ │    ○     │ │    ○     │ │    ○     │   │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘   │
├────────────────────────────────────────────────────────┤
│ SELECTED: Tool API                                     │
│ ┌────────────────────────────────────────────────────┐ │
│ │ Content: Minimal orthogonal primitives - file...   │ │
│ │ Findings: Three main tool categories identified... │ │
│ └────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────┘
```

**Pros:** Visual chunk progress, click to expand, clear status per chunk
**Cons:** Needs more vertical space, card layout may feel cluttered

### Option D: Terminal/Log Style (Developer-Focused)

```
┌────────────────────────────────────────────────────────┐
│ $ rlm analyze --doc "git.md"                           │
│   --query "What are the main components?"              │
│                                                        │
│ [00:00.0] ROUTE    selecting sections...               │
│ [00:00.4] ROUTE    found 8 candidates                  │
│ [00:00.8] CHUNK    splitting document                  │
│ [00:01.0] CHUNK    8 chunks created                    │
│ [00:01.2] EXTRACT  chunk[0] "External Contract"        │
│ [00:02.0] EXTRACT  chunk[0] done (relevance: 0.95)     │
│           → "String-in/string-out API for LLM..."      │
│ [00:02.2] EXTRACT  chunk[1] "Repository Environment"   │
│ [00:03.2] EXTRACT  chunk[1] done (relevance: 0.92)     │
│           → "Repo handle with commit pinning..."       │
│ ...                                                    │
│ [00:11.2] REDUCE   combining 8 extractions             │
│ [00:13.5] REDUCE   synthesis complete                  │
│ [00:14.0] DONE     confidence: 0.91                    │
│                                                        │
│ ═══════════════════════════════════════════════════════│
│ ANSWER:                                                │
│ The Repo-RLM runtime spec consists of 6 main...        │
└────────────────────────────────────────────────────────┘
```

**Pros:** Familiar to developers, clear timeline, easy to follow
**Cons:** Less visual appeal, not for non-technical audiences

### Option E: Simplified Dashboard (Minimal)

```
┌─────────────────────────────────────────────┐
│ RLM VISUALIZER              [● STREAMING]   │
├─────────────────────────────────────────────┤
│                                             │
│   "What are the main components of the      │
│    Repo-RLM runtime spec?"                  │
│                                             │
│   ○ Route  ● Extract (3/8)  ○ Reduce        │
│                                             │
│   ┌───────────────────────────────────┐     │
│   │ Processing: Tool API              │     │
│   │ ████████████░░░░░░░░░░░░░░░░░░░░  │     │
│   │                                   │     │
│   │ "Three main tool categories..."   │     │
│   └───────────────────────────────────┘     │
│                                             │
└─────────────────────────────────────────────┘
```

**Pros:** Clean, focused, mobile-friendly
**Cons:** Hides complexity, less impressive demo

---

## Recommendations

### Quick Fixes (Current Design)
1. Fix text overlap issues (z-index, padding)
2. Add text wrapping to Detail View
3. Make timeline dots larger with tooltips
4. Add visual separator between input and output sections

### Medium Effort
5. Implement Option C (Card-Based) for chunk visualization
6. Add click-to-expand on chunks
7. Show timing information per phase
8. Add a "minimize input" toggle after starting

### Redesign Consideration
- Option B (Vertical Timeline) is most natural for understanding flow
- Option D (Terminal Style) fits the developer audience best
- Option E (Minimal) for embedding/mobile

---

## Technical Notes

- Current implementation: `crates/web/client/src/views/rlm.rs`
- Trace playback: `crates/web/client/assets/rlm-demo-trace.json`
- State management: `crates/web/client/src/state.rs` (RlmVizState)
- All rendering is GPU-based via wgpui (not HTML/CSS)
