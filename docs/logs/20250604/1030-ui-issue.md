# UI Package Extraction Analysis - December 4, 2025, 10:30 AM

## Objective

Extract UI components from the Commander repository into a new @openagentsinc/ui package in the openagents monorepo.

### Required Components
- Pane system
- Hotbar
- Tailwind and ShadUI configuration
- Related tests

### Excluded Components
- Hand tracking (to be extracted separately later)

### Design Considerations
- **Short term**: Support Electron and web applications (CSS-based)
- **Long term**: Support mobile applications (no CSS, reimagined components)
- Must plan for future mobile extensibility within the same package

## Analysis Log

### 1. Commander Repository Structure

#### Overall Architecture
- **Framework**: Electron + React + TypeScript
- **Styling**: Tailwind CSS v4 with custom configuration
- **Component Library**: ShadCN UI (customized)
- **State Management**: Zustand with persistence
- **Build Tool**: Vite with Tailwind plugin

#### Key UI Components Found

**Pane System:**
- `/src/panes/PaneManager.tsx` - Main orchestrator
- `/src/panes/Pane.tsx` - Individual pane component
- `/src/stores/pane.ts` - State management
- Features: Draggable, resizable, z-index management, keyboard shortcuts

**Hotbar:**
- `/src/components/hud/Hotbar.tsx` - Main component
- `/src/components/hud/HotbarItem.tsx` - Individual items
- 9 slots with keyboard shortcuts (Cmd/Ctrl + 1-9)
- Fixed position at bottom center

**UI Components:**
- `/src/components/ui/` - ShadCN components
- Custom zero border radius design
- Dark mode optimized

### 2. Detailed Configuration Analysis

#### Dependencies to Extract

**Core Dependencies:**
- React 19.1.0
- TypeScript 5.8.3
- Vite 6.3.3

**Tailwind CSS v4 Stack:**
- tailwindcss: ^4.1.7 (latest v4!)
- @tailwindcss/vite: ^4.1.4
- tailwind-merge: ^3.2.0
- tailwindcss-animate: ^1.0.7

**ShadCN/Radix UI Components:**
- Full suite of @radix-ui components (dialog, dropdown, tabs, etc.)
- class-variance-authority: ^0.7.1
- clsx: ^2.1.1
- lucide-react: ^0.510.0

#### Configuration Details

**ShadCN Setup (components.json):**
- Style: default
- CSS Variables: enabled
- Base color: slate
- Icon library: lucide
- Aliases configured for @/components, @/utils, @/ui

**Tailwind CSS v4 Configuration:**
- Uses new @import syntax (no traditional config file)
- OKLCH color space for all colors
- Zero border radius design system
- Berkeley Mono font
- Dark mode with .dark class
- Custom animations for accordions

### 3. Mobile Extensibility Analysis

#### Browser-Specific Dependencies

**APIs and Features:**
- Window dimensions (window.innerWidth/Height)
- Mouse/pointer events
- CSS absolute positioning and z-index
- Cursor styles for resize handles
- Webkit-specific scrollbar styling
- Backdrop blur effects

**Key Libraries:**
- @use-gesture/react - Handles drag/resize (supports touch)
- Radix UI components (web-only)
- localStorage for persistence

#### Mobile Adaptation Requirements

**Gesture Differences:**
- Replace mouse drag with touch gestures
- Pinch to resize instead of corner handles
- Long press to initiate drag
- Swipe gestures for navigation

**Layout Changes:**
- Full-screen panes instead of floating windows
- Tab-based or stack navigation
- Bottom sheets for menus
- Responsive sizing constraints

**Platform Abstractions Needed:**
1. Event handling layer (mouse vs touch)
2. Layout engine (floating vs stacked)
3. Storage abstraction (localStorage vs native)
4. Menu system (dropdown vs native sheets)
5. Animation/performance profiles

### 4. Proposed Package Architecture

#### Directory Structure
```
packages/ui/
├── src/
│   ├── core/           # Shared logic
│   │   ├── state/      # State management
│   │   ├── types/      # Type definitions
│   │   └── utils/      # Shared utilities
│   ├── web/            # Web-specific components
│   │   ├── components/ # React components with CSS
│   │   ├── styles/     # Tailwind/CSS files
│   │   └── hooks/      # Web-specific hooks
│   ├── mobile/         # Mobile-specific (future)
│   │   ├── components/ # React Native components
│   │   └── hooks/      # Mobile-specific hooks
│   └── index.ts        # Main exports
├── package.json
└── tsconfig.json
```

#### Export Strategy
```typescript
// Web exports
export * from './web/components'
export * from './core/state'
export * from './core/types'

// Future mobile exports
// export * from './mobile/components'
```

### 5. Implementation Plan

#### Phase 1: Initial Setup
1. Create @openagentsinc/ui package structure
2. Set up Tailwind CSS v4 configuration
3. Configure TypeScript and build process
4. Add core dependencies

#### Phase 2: Core Extraction
1. Extract pane types and state management
2. Create platform abstraction layer
3. Extract shared utilities
4. Set up export structure

#### Phase 3: Web Components
1. Extract and adapt Pane components
2. Extract Hotbar components
3. Migrate ShadCN UI components
4. Set up styles and themes

#### Phase 4: Testing
1. Set up test infrastructure
2. Adapt existing pane tests
3. Create hotbar tests (missing in original)
4. Add integration tests

#### Phase 5: Documentation
1. API documentation
2. Usage examples
3. Migration guide from Commander
4. Mobile roadmap

### 6. Test Analysis

#### Existing Tests to Extract
- **Pane Store Tests**: paneActions.test.ts, openAgentChatPane.test.ts
- **UI Elements Tests**: uiElementsStore.test.ts (positioning, pinning)
- **Component Tests**: Various pane component tests (need adaptation)
- **Test Utilities**: Effect test utils, MSW setup

#### Missing Tests
- No hotbar component tests exist
- Limited UI component coverage
- No gesture/drag tests

### 7. Key Decisions and Recommendations

1. **Use Tailwind CSS v4**: Maintain consistency with Commander's modern setup
2. **Platform Abstraction First**: Design core interfaces before implementation
3. **Progressive Enhancement**: Start with web, plan for mobile
4. **Test Coverage**: Add missing hotbar tests immediately
5. **Documentation**: Create comprehensive examples for both platforms
6. **Version Strategy**: Start at 0.1.0 for initial extraction

### 8. Potential Challenges

1. **Tailwind v4 Documentation**: Limited resources as it's very new
2. **Effect Integration**: May need special consideration for state management
3. **Mobile Abstractions**: Complex gesture handling differences
4. **Breaking Changes**: Commander will need updates to use new package
5. **CI/CD Setup**: Need to integrate with existing monorepo workflow

## Issue Created

GitHub Issue #903 has been created with a comprehensive plan for extracting the UI components from Commander into the @openagentsinc/ui package.

**Issue URL**: https://github.com/OpenAgentsInc/openagents/issues/903

The issue includes:
- Detailed technical specifications
- Mobile extensibility architecture
- Implementation phases
- Success criteria
- Platform abstraction strategy
- Comprehensive documentation of all findings from this analysis
