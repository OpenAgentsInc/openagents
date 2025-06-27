# MVP Components Library

A comprehensive collection of Arwes-styled React components for the OpenAgents MVP, organized using Atomic Design principles.

## Table of Contents

### 🔵 Atoms (8 Basic Building Blocks)

1. **[StatusBadge](./atoms/StatusBadge.stories.tsx)**
   - Status indicator with 6 states (idle, generating, deploying, deployed, error, paused) featuring animated icons and optional pulse effects

2. **[LoadingSpinner](./atoms/LoadingSpinner.stories.tsx)**
   - Customizable animated loading indicators with multiple variants (ring, dots, pulse, bars) and size options

3. **[StreamingCursor](./atoms/StreamingCursor.stories.tsx)**
   - Animated blinking cursor for AI text generation, mimicking terminal-style typing with customizable blink speed and colors

4. **[CopyButton](./atoms/CopyButton.stories.tsx)**
   - Copy-to-clipboard button with visual feedback, supporting icon-only, text-only, or combined variants with success/error states

5. **[DeploymentUrl](./atoms/DeploymentUrl.stories.tsx)**
   - Clickable deployment URL component with status indicators, copy functionality, and automatic truncation for long URLs

6. **[ModelBadge](./atoms/ModelBadge.stories.tsx)**
   - AI model indicator badges with provider-specific theming and icons for Cloudflare, OpenRouter, OpenAI, and Anthropic

7. **[HeroCallToAction](./atoms/HeroCallToAction.stories.tsx)**
   - High-impact call-to-action button with countdown timer, benefits display, and conversion-optimized styling

8. **[LiveUsageStats](./atoms/LiveUsageStats.stories.tsx)**
   - Real-time platform usage statistics with animated counters, user activity feed, and social proof indicators

### 🔷 Molecules (11 Compound Components)

9. **[ChatMessage](./molecules/ChatMessage.stories.tsx)**
   - Complete chat message display with user/assistant variants, timestamps, model info, and action buttons

10. **[StreamingMessage](./molecules/StreamingMessage.stories.tsx)**
    - Real-time streaming message display with typing animation, partial content rendering, and animated cursor

11. **[CodeBlock](./molecules/CodeBlock.stories.tsx)**
    - Syntax-highlighted code display with line numbers, copy functionality, line highlighting, and language-specific icons

12. **[DeploymentStage](./molecules/DeploymentStage.stories.tsx)**
    - Individual deployment stage indicator showing progress through different deployment phases with animated transitions

13. **[GenerationStep](./molecules/GenerationStep.stories.tsx)**
    - AI code generation step tracker displaying current operation, file being generated, and completion status

14. **[ToolInvocation](./molecules/ToolInvocation.stories.tsx)**
    - Tool usage display for AI function calls, showing tool name, parameters, and execution results with syntax highlighting

15. **[ChatInputWithStatus](./molecules/ChatInputWithStatus.stories.tsx)**
    - Enhanced chat input with integrated status bar, character count, voice input, attachments, and real-time status updates

16. **[ProjectHeader](./molecules/ProjectHeader.stories.tsx)**
    - Project header with title, status badge, deployment controls, and quick actions for managing the current project

17. **[GuidedPromptInput](./molecules/GuidedPromptInput.stories.tsx)**
    - Enhanced chat input with context hints, suggestions, and guided prompting for improved user experience

18. **[OnboardingErrorRecovery](./molecules/OnboardingErrorRecovery.stories.tsx)**
    - Error recovery interface for onboarding flow with clear messaging, retry options, and alternative paths

19. **[RecentBuildsStream](./molecules/RecentBuildsStream.stories.tsx)**
    - Live feed of recent platform activity showing deployments, frameworks, and social proof for engagement

### 🟦 Organisms (6 Complex UI Sections)

20. **[ChatInterface](./organisms/ChatInterface.stories.tsx)**
    - Complete chat UI with message history, auto-scrolling, model selection, and integrated input controls

21. **[DeploymentProgress](./organisms/DeploymentProgress.stories.tsx)**
    - Multi-stage deployment visualization showing initialization, building, optimizing, and deployment with progress tracking

22. **[GenerationProgress](./organisms/GenerationProgress.stories.tsx)**
    - AI code generation progress tracker with multiple steps, file listings, and animated state transitions

23. **[ProjectWorkspace](./organisms/ProjectWorkspace.stories.tsx)**
    - Three-panel layout system with chat, generation, and deployment panels, supporting multiple layout configurations

24. **[AutoPlayingDemoLoop](./organisms/AutoPlayingDemoLoop.stories.tsx)**
    - Auto-playing demonstration carousel showcasing platform capabilities for homepage and marketing use

25. **[OnboardingPathSelector](./organisms/OnboardingPathSelector.stories.tsx)**
    - Post-authentication choice architecture for template selection vs. chat-to-build flow

### 📄 Templates (4 Full Page Layouts)

26. **[BitcoinPunsDemo](./templates/BitcoinPunsDemo.stories.tsx)**
    - Complete interactive demo showcasing the full OpenAgents flow from chat to deployment with auto-progression

27. **[DeploymentSuccess](./templates/DeploymentSuccess.stories.tsx)**
    - Celebration screen shown after successful deployment with confetti, stats, and action buttons

28. **[DesktopRequired](./templates/DesktopRequired.stories.tsx)**
    - Desktop requirement alert screen with amber warning styling, enforcing minimum 1024px width for optimal experience

29. **[FirstDeploymentCelebration](./templates/FirstDeploymentCelebration.stories.tsx)**
    - Enhanced celebration experience for first-time deployments with maximum psychological impact and next steps

## Component Features

### Common Props & Patterns

- **Animation**: Most components support `animated` prop for entrance/exit animations using Arwes AnimatorGeneralProvider
- **Theming**: Consistent use of Arwes color palette (cyan, yellow, green, red, purple) with transparency variants
- **Typography**: Arwes Text component with Berkeley Mono font for code and Titillium Web for UI text
- **Interactions**: Hover states, active states, and disabled states following Arwes design patterns
- **Error Handling**: Graceful fallbacks and error states with proper TypeScript typing

### Storybook Organization

As of issue #1115 implementation, MVP components maintain their atomic design structure within the broader Storybook categorization:

```
📁 Storybook
├── 📁 Foundation (Arwes Core, Utilities, Icons)
├── 📁 Components (Basic, Layout, Navigation, Data Display)
├── 📁 Features (Chat & AI, Workspace)
├── 📁 MVP ⭐
│   ├── 📁 Atoms (8 stories)
│   ├── 📁 Molecules (11 stories)
│   ├── 📁 Organisms (6 stories)
│   └── 📁 Templates (4 stories)
└── 📁 Patterns & Examples (Advanced Techniques, Examples)
```

- **Autodocs**: All components have comprehensive prop documentation
- **Playground**: Interactive story for testing all prop combinations
- **Variants**: Multiple stories showcasing different states and configurations
- **Demo Stories**: Real-world usage examples and interactive demonstrations

### Design System Integration

- **Arwes Components**: FrameCorners, Animator, Animated, Text, and other Arwes primitives
- **Tailwind CSS**: Utility classes for layout and spacing with custom theme
- **CSS-in-JS**: Inline styles for dynamic properties and animations
- **Dark Theme**: All components designed for dark backgrounds with proper contrast

## Usage Example

```typescript
import { ChatInterface } from './organisms/ChatInterface'
import { StatusBadge } from './atoms/StatusBadge'
import { ProjectWorkspace } from './organisms/ProjectWorkspace'

function MyApp() {
  const [status, setStatus] = useState('idle')
  const [messages, setMessages] = useState([])

  return (
    <div className="min-h-screen bg-black">
      <StatusBadge status={status} />
      <ProjectWorkspace
        currentProject="My Bitcoin App"
        leftPanel={{
          id: 'chat',
          type: 'chat',
          title: 'Chat',
          content: { messages }
        }}
      />
    </div>
  )
}
```

## Development Guidelines

1. **Component Structure**: Each component exports both the component and its TypeScript interface
2. **Animation**: Use AnimatorGeneralProvider for consistent timing across components
3. **Error Boundaries**: All components include fallback values for undefined props
4. **Accessibility**: Proper ARIA labels, keyboard navigation, and focus management
5. **Performance**: Memoization where appropriate, lazy loading for heavy components
6. **Atomic Design**: Follow the hierarchy - atoms → molecules → organisms → templates

## Implementation Statistics

- **Total Components**: 29 implemented
- **Atoms**: 8 basic building blocks
- **Molecules**: 11 compound components
- **Organisms**: 6 complex UI sections
- **Templates**: 4 full page layouts
- **Storybook Stories**: 55+ individual stories across all components

## Recent Enhancements

### Onboarding & Conversion (Phase 4)
- **HeroCallToAction**: High-conversion homepage CTA
- **LiveUsageStats**: Real-time social proof
- **AutoPlayingDemoLoop**: Homepage demonstration carousel
- **OnboardingPathSelector**: Post-auth choice architecture
- **GuidedPromptInput**: Enhanced user guidance
- **OnboardingErrorRecovery**: Graceful error handling
- **FirstDeploymentCelebration**: Maximum psychological impact

### Core Features (Established)
- **Complete Chat Flow**: From input to streaming responses
- **Deployment Tracking**: Real-time progress with WebSocket integration
- **Code Generation**: Step-by-step AI code creation visualization
- **Project Management**: Three-panel workspace architecture
- **Error Handling**: Comprehensive error states and recovery

## Future Enhancements

- [ ] Add keyboard shortcuts for common actions
- [ ] Implement drag-and-drop for file uploads  
- [ ] Add voice input integration
- [ ] Create mobile-responsive variants
- [ ] Add internationalization support
- [ ] Implement custom theme builder
- [ ] Add component composition examples
- [ ] Create E2E test coverage
- [ ] Performance optimization pass
- [ ] Advanced animation choreography