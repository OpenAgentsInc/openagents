# MVP Components Library

A comprehensive collection of Arwes-styled React components for the OpenAgents MVP, organized using Atomic Design principles.

## Table of Contents

### 🔵 Atoms (Basic Building Blocks)

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

### 🔷 Molecules (Compound Components)

7. **[ChatMessage](./molecules/ChatMessage.stories.tsx)**
   - Complete chat message display with user/assistant variants, timestamps, model info, and action buttons

8. **[StreamingMessage](./molecules/StreamingMessage.stories.tsx)**
   - Real-time streaming message display with typing animation, partial content rendering, and animated cursor

9. **[CodeBlock](./molecules/CodeBlock.stories.tsx)**
   - Syntax-highlighted code display with line numbers, copy functionality, line highlighting, and language-specific icons

10. **[DeploymentStage](./molecules/DeploymentStage.stories.tsx)**
    - Individual deployment stage indicator showing progress through different deployment phases with animated transitions

11. **[GenerationStep](./molecules/GenerationStep.stories.tsx)**
    - AI code generation step tracker displaying current operation, file being generated, and completion status

12. **[ToolInvocation](./molecules/ToolInvocation.stories.tsx)**
    - Tool usage display for AI function calls, showing tool name, parameters, and execution results with syntax highlighting

13. **[ChatInputWithStatus](./molecules/ChatInputWithStatus.stories.tsx)**
    - Enhanced chat input with integrated status bar, character count, voice input, attachments, and real-time status updates

14. **[ProjectHeader](./molecules/ProjectHeader.stories.tsx)**
    - Project header with title, status badge, deployment controls, and quick actions for managing the current project

### 🟦 Organisms (Complex UI Sections)

15. **[ChatInterface](./organisms/ChatInterface.stories.tsx)**
    - Complete chat UI with message history, auto-scrolling, model selection, and integrated input controls

16. **[DeploymentProgress](./organisms/DeploymentProgress.stories.tsx)**
    - Multi-stage deployment visualization showing initialization, building, optimizing, and deployment with progress tracking

17. **[GenerationProgress](./organisms/GenerationProgress.stories.tsx)**
    - AI code generation progress tracker with multiple steps, file listings, and animated state transitions

18. **[ProjectWorkspace](./organisms/ProjectWorkspace.stories.tsx)**
    - Three-panel layout system with chat, generation, and deployment panels, supporting multiple layout configurations

### 📄 Templates (Full Page Layouts)

19. **[BitcoinPunsDemo](./templates/BitcoinPunsDemo.stories.tsx)**
    - Complete interactive demo showcasing the full OpenAgents flow from chat to deployment with auto-progression

20. **[DeploymentSuccess](./templates/DeploymentSuccess.stories.tsx)**
    - Celebration screen shown after successful deployment with confetti, stats, and action buttons

21. **[DesktopRequired](./templates/DesktopRequired.stories.tsx)**
    - Desktop requirement alert screen with amber warning styling, enforcing minimum 1024px width for optimal experience

## Component Features

### Common Props & Patterns

- **Animation**: Most components support `animated` prop for entrance/exit animations using Arwes AnimatorGeneralProvider
- **Theming**: Consistent use of Arwes color palette (cyan, yellow, green, red, purple) with transparency variants
- **Typography**: Arwes Text component with Berkeley Mono font for code and Titillium Web for UI text
- **Interactions**: Hover states, active states, and disabled states following Arwes design patterns
- **Error Handling**: Graceful fallbacks and error states with proper TypeScript typing

### Storybook Organization

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
import { ChatInterface } from '@/components/mvp/organisms/ChatInterface.stories'
import { StatusBadge } from '@/components/mvp/atoms/StatusBadge.stories'

function MyApp() {
  return (
    <div className="min-h-screen bg-black">
      <StatusBadge status="generating" />
      <ChatInterface 
        messages={messages}
        onSendMessage={handleSend}
        autoScroll={false}
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

## Future Enhancements

- [ ] Add keyboard shortcuts for common actions
- [ ] Implement drag-and-drop for file uploads
- [ ] Add voice input integration
- [ ] Create mobile-responsive variants
- [ ] Add internationalization support
- [ ] Implement custom theme builder
- [ ] Add component composition examples
- [ ] Create E2E test coverage