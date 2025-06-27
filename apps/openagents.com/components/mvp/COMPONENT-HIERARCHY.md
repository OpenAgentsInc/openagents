# Component Hierarchy & Relationships

## Visual Component Tree

```
┌─────────────────────────────────────────────────────────────┐
│                     OpenAgents MVP UI                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  TEMPLATES (Full Experiences)                                │
│  ├── BitcoinPunsDemo ────────────────┐                      │
│  │   └── Uses: ProjectWorkspace      │                      │
│  │                                   │                      │
│  ├── DeploymentSuccess               │                      │
│  │   └── Uses: StatusBadge           │                      │
│  │             DeploymentUrl         │                      │
│  │                                   │                      │
│  ├── FirstDeploymentCelebration 🆕   │                      │
│  │   ├── Uses: DeploymentUrl         │                      │
│  │   ├── Uses: CopyButton            │                      │
│  │   └── Uses: Confetti animations   │                      │
│  │                                   │                      │
│  └── DesktopRequired                 │                      │
│      └── Uses: FrameCorners (Arwes)  │                      │
│                                      │                      │
│  ORGANISMS (Major Sections)          │                      │
│  ├── ProjectWorkspace ◄──────────────┘                      │
│  │   ├── Uses: ChatInterface                                │
│  │   ├── Uses: GenerationProgress                           │
│  │   └── Uses: DeploymentProgress                           │
│  │                                                          │
│  ├── AutoPlayingDemoLoop 🆕                                 │
│  │   ├── Uses: StreamingMessage                             │
│  │   ├── Uses: DeploymentProgress                           │
│  │   ├── Uses: DeploymentSuccess                            │
│  │   └── Uses: CodeBlock                                    │
│  │                                                          │
│  ├── OnboardingPathSelector 🆕                              │
│  │   └── Uses: FrameBox (Arwes)                             │
│  │                                                          │
│  ├── ChatInterface                                          │
│  │   ├── Uses: ChatMessage (multiple)                       │
│  │   ├── Uses: StreamingMessage                             │
│  │   └── Uses: StatusBadge                                  │
│  │                                                          │
│  ├── DeploymentProgress                                     │
│  │   └── Uses: DeploymentStage (multiple)                   │
│  │                                                          │
│  ├── DeploymentTracker (Real-time WebSocket)                │
│  │   ├── Uses: StatusBadge                                  │
│  │   ├── Uses: LoadingSpinner                               │
│  │   └── Uses: Toast notifications                          │
│  │                                                          │
│  └── GenerationProgress                                     │
│      └── Uses: GenerationStep (multiple)                    │
│                                                             │
│  MOLECULES (Composite Components)                           │
│  ├── RecentBuildsStream 🆕                                  │
│  │   └── Uses: FrameworkIcon                                │
│  │                                                          │
│  ├── GuidedPromptInput 🆕                                   │
│  │   └── Uses: ContextHint                                  │
│  │                                                          │
│  ├── OnboardingErrorRecovery 🆕                             │
│  │   └── Uses: FrameBox (Arwes)                             │
│  │                                                          │
│  ├── ChatMessage                                            │
│  │   ├── Uses: StatusBadge                                  │
│  │   ├── Uses: ModelBadge                                   │
│  │   └── Uses: CopyButton                                   │
│  │                                                          │
│  ├── StreamingMessage                                       │
│  │   ├── Uses: StatusBadge                                  │
│  │   ├── Uses: StreamingCursor                              │
│  │   └── Uses: ModelBadge                                   │
│  │                                                          │
│  ├── CodeBlock                                              │
│  │   └── Uses: CopyButton                                   │
│  │                                                          │
│  ├── DeploymentStage                                        │
│  │   └── Uses: StatusBadge                                  │
│  │                                                          │
│  ├── GenerationStep                                         │
│  │   ├── Uses: StatusBadge                                  │
│  │   └── Uses: LoadingSpinner                               │
│  │                                                          │
│  ├── ToolInvocation                                         │
│  │   └── Uses: CodeBlock principles                         │
│  │                                                          │
│  ├── ChatInputWithStatus                                    │
│  │   ├── Uses: StatusBadge                                  │
│  │   └── Uses: StreamingCursor                              │
│  │                                                          │
│  └── ProjectHeader                                          │
│      ├── Uses: StatusBadge                                  │
│      └── Uses: ModelBadge                                   │
│                                                             │
│  ATOMS (Basic Building Blocks)                              │
│  ├── HeroCallToAction 🆕 (Homepage CTA)                     │
│  ├── LiveUsageStats 🆕 (Platform metrics)                   │
│  ├── StatusBadge (Used by 8+ components)                    │
│  ├── LoadingSpinner (Standalone)                            │
│  ├── StreamingCursor (Used by chat components)              │
│  ├── CopyButton (Used by code/message components)           │
│  ├── DeploymentUrl (Used in success screens)                │
│  └── ModelBadge (Used by chat components)                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Component Relationships

### Data Flow Patterns

```
User Input ──► ChatInputWithStatus ──► ChatInterface ──► ChatMessage/StreamingMessage
                                            │
                                            ▼
                                    GenerationProgress ──► GenerationStep
                                            │
                                            ▼
                                    DeploymentProgress ──► DeploymentStage
                                            │
                                            ▼
                                    DeploymentSuccess ──► DeploymentUrl
```

### Most Used Components (Utility Ranking)

1. **StatusBadge** - 8+ direct uses
   - Foundation for all state visualization
   - Used in messages, stages, steps, headers, and inputs

2. **CopyButton** - 4+ direct uses
   - Essential for code sharing functionality
   - Integrated into messages, code blocks, and URLs

3. **ModelBadge** - 3+ direct uses
   - Identifies AI provider context
   - Critical for multi-model support

4. **StreamingCursor** - 2+ direct uses
   - Key visual indicator for AI activity
   - Creates "alive" feeling in the UI

### Component Categories by Function

#### State Indicators
- StatusBadge (primary)
- LoadingSpinner (activity)
- StreamingCursor (typing)
- DeploymentStage (progress)
- GenerationStep (progress)

#### Content Display
- ChatMessage (communication)
- StreamingMessage (real-time)
- CodeBlock (technical)
- ToolInvocation (operations)
- DeploymentUrl (results)

#### User Input
- ChatInputWithStatus (primary input)
- CopyButton (action)

#### Layout & Structure
- ProjectWorkspace (main layout)
- ChatInterface (chat section)
- ProjectHeader (navigation)
- DeploymentProgress (process view)
- GenerationProgress (process view)

#### Full Experiences
- BitcoinPunsDemo (demonstration)
- DeploymentSuccess (completion)
- DesktopRequired (gate)

### Integration Points

1. **ProjectWorkspace** acts as the main container, orchestrating:
   - Chat functionality (ChatInterface)
   - Code generation visualization (GenerationProgress)
   - Deployment tracking (DeploymentProgress)

2. **ChatInterface** serves as the communication hub:
   - Displays message history (ChatMessage)
   - Shows real-time responses (StreamingMessage)
   - Accepts user input (internal textarea)

3. **Status Management** flows through:
   - StatusBadge (visual indicator)
   - Status props (data passing)
   - State machines (in parent components)

### Component Reusability Score

- **High Reusability** (used everywhere): StatusBadge, CopyButton
- **Medium Reusability** (domain-specific): ModelBadge, StreamingCursor, LoadingSpinner
- **Low Reusability** (purpose-built): BitcoinPunsDemo, DesktopRequired, DeploymentSuccess

### New Onboarding Components (Phase 4)

1. **Onboarding Atoms Added**:
   - **HeroCallToAction** - High-impact CTA with countdown timer and benefits
   - **LiveUsageStats** - Real-time platform metrics with animated counters

2. **Onboarding Molecules Added**:
   - **RecentBuildsStream** - Live feed of platform activity for social proof
   - **GuidedPromptInput** - Enhanced chat input with suggestions and hints
   - **OnboardingErrorRecovery** - Graceful error handling during onboarding

3. **Onboarding Organisms Added**:
   - **AutoPlayingDemoLoop** - Homepage demo carousel showing platform capabilities
   - **OnboardingPathSelector** - Post-auth choice architecture (template vs chat)

4. **Enhanced Templates Added**:
   - **FirstDeploymentCelebration** - Maximizes psychological impact of first success

### Remaining Component Opportunities

1. **Missing Atoms**:
   - ProgressBar (linear progress)
   - IconButton (standardized icon actions)
   - Tooltip (hover information)
   - AnimatedCounter (number animations)

2. **Missing Molecules**:
   - FileUpload (drag-drop interface)
   - ModelSelector (dropdown/modal)
   - TemplateSelectionOnboarding (enhanced template cards)
   - AIStreamingTheater (enhanced streaming visualization)

3. **Missing Organisms**:
   - SettingsPanel (configuration)
   - HistoryBrowser (past sessions)
   - PostSuccessExploration (next steps after deployment)
   - OnboardingProgressTracker (analytics component)

### Phase 3 Enhancements (Implemented)

1. **Real-time WebSocket Integration**:
   - DeploymentTracker with live deployment updates
   - WebSocket connection management with auto-reconnect
   - Mock fallback for development environments

2. **Enhanced Error Handling**:
   - ErrorBoundary components with specialized fallbacks
   - Toast notification system with Arwes styling
   - Retry mechanisms with exponential backoff

3. **Performance Optimization**:
   - Lazy loading with React.Suspense
   - Web Vitals monitoring
   - Memory usage tracking
   - Performance recommendations