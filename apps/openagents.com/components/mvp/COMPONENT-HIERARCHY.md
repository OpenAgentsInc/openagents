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
│  └── DesktopRequired                 │                      │
│      └── Uses: FrameCorners (Arwes)  │                      │
│                                      │                      │
│  ORGANISMS (Major Sections)          │                      │
│  ├── ProjectWorkspace ◄──────────────┘                      │
│  │   ├── Uses: ChatInterface                                │
│  │   ├── Uses: GenerationProgress                           │
│  │   └── Uses: DeploymentProgress                           │
│  │                                                          │
│  ├── ChatInterface                                          │
│  │   ├── Uses: ChatMessage (multiple)                       │
│  │   ├── Uses: StreamingMessage                             │
│  │   └── Uses: StatusBadge                                  │
│  │                                                          │
│  ├── DeploymentProgress                                     │
│  │   └── Uses: DeploymentStage (multiple)                   │
│  │                                                          │
│  └── GenerationProgress                                     │
│      └── Uses: GenerationStep (multiple)                    │
│                                                             │
│  MOLECULES (Composite Components)                           │
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

### Future Component Opportunities

1. **Missing Atoms**:
   - ProgressBar (linear progress)
   - IconButton (standardized icon actions)
   - Tooltip (hover information)

2. **Missing Molecules**:
   - ErrorBoundary (error handling)
   - FileUpload (drag-drop interface)
   - ModelSelector (dropdown/modal)

3. **Missing Organisms**:
   - SettingsPanel (configuration)
   - HistoryBrowser (past sessions)
   - MetricsDisplay (usage stats)