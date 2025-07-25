# Pane Organization System

The OpenAgents application includes an intelligent pane organization system that automatically arranges all visible panes on screen in an optimal layout. This feature is accessible via hotkey `Cmd/Ctrl+2` or the grid icon in the hotbar.

## Overview

The pane organization system provides a one-click solution to neatly arrange all currently open panes (Settings, History, and Chat panes) into logical, space-efficient layouts that maximize screen real estate and improve workflow efficiency.

## Access Methods

### Hotbar Button
- **Location**: Hotbar slot 2
- **Icon**: LayoutGrid (rectangular grid icon)
- **Title**: "Organize Panes"

### Keyboard Shortcut
- **Windows/Linux**: `Ctrl+2`
- **macOS**: `Cmd+2`

## Layout Algorithm

The organization system uses a smart algorithm that adapts based on the number and types of visible panes:

### Settings Pane Priority
If a Settings pane is open, it is always positioned as a narrow sidebar on the left side of the screen with:
- **Width**: 320px (SETTINGS_PANEL_WIDTH)
- **Height**: Full available height
- **Position**: Left edge with standard margin

### Layout Strategies by Pane Count

#### 1 Pane (excluding Settings)
- **Layout**: Single pane uses full remaining width
- **Dimensions**: Maximum available space after Settings sidebar

#### 2 Panes (excluding Settings)  
- **Layout**: Side-by-side arrangement
- **Dimensions**: Each pane gets ~50% of remaining width minus gap
- **Use Case**: Ideal for comparing two chat sessions or History + one chat

#### 3 Panes (excluding Settings)
- **Layout**: Asymmetric split
- **Left Pane**: 40% of remaining width, full height
- **Right Panes**: Two panes stacked vertically, sharing 60% width
- **Use Case**: Primary chat session on left, secondary tasks on right

#### 4+ Panes (excluding Settings)
- **Layout**: Grid arrangement
- **Algorithm**: Calculates optimal grid dimensions using `Math.ceil(Math.sqrt(paneCount))`
- **Distribution**: Panes arranged in rows and columns with equal spacing
- **Use Case**: Multiple chat sessions or complex workflows

## Technical Implementation

### Pane Store Integration
The organization logic is implemented in the Zustand pane store as the `organizePanes()` function:

```typescript
organizePanes: () => {
  // Smart layout calculation based on screen dimensions
  // and visible pane types and counts
}
```

### Layout Calculations

#### Screen Space Management
- **Available Height**: `window.innerHeight - margins - hotbar height`
- **Available Width**: `window.innerWidth - margins`
- **Margins**: 20px standard margin (PANE_MARGIN)
- **Hotbar Space**: 60px reserved at bottom

#### Responsive Behavior
- Layouts adapt to current screen size
- Panes maintain minimum usable dimensions
- Grid layouts optimize for screen aspect ratio

### Pane Type Handling

#### Settings Pane
- Always positioned as left sidebar when present
- Fixed width of 320px
- Full height allocation
- Takes priority in layout calculation

#### History Pane (Metadata)
- Treated as standard content pane
- No special positioning rules
- Included in main layout algorithm

#### Chat Panes
- Primary content panes
- Optimized for readability with ~40% width target
- Arranged in grid patterns for multiple instances

## User Experience

### Predictable Layouts
- Consistent positioning rules across all scenarios
- Settings always on left when present
- Logical progression from side-by-side to grid layouts

### Workflow Optimization
- Quick access via prominent hotkey (Cmd/Ctrl+2)
- One-click organization without manual positioning
- Maintains pane focus and content state

### Visual Feedback
- Immediate layout changes upon activation
- Smooth transitions preserve user context
- Clear visual hierarchy with appropriate sizing

## Usage Scenarios

### Development Workflow
1. **Settings + 2 Chats**: Settings sidebar, two chat sessions side-by-side
2. **History + Multiple Chats**: History pane with grid of active sessions
3. **Full Workspace**: Settings + History + 4 chat sessions in organized grid

### Content Creation
- **Research**: Settings + History + multiple topic-specific chats
- **Comparison**: Side-by-side chat sessions for comparing approaches
- **Project Management**: Organized grid view of all active workstreams

## Integration with Existing Systems

### Pane Management
- Compatible with existing drag/resize functionality
- Preserves pane state and content
- Works with focus management system

### Persistence
- Organized layouts are automatically saved
- Positions persist across application restarts
- Integrates with existing pane position storage

### Keyboard Navigation
- Maintains existing keyboard shortcuts for individual panes
- Complements rather than replaces manual pane management
- Provides quick reset option for cluttered workspaces

## Performance Considerations

### Efficient Calculations
- O(n) complexity for layout algorithm
- Minimal DOM manipulation
- Batched pane position updates

### Memory Usage
- No additional pane state storage required
- Reuses existing pane management infrastructure
- Lightweight function with no persistent overhead

## Future Enhancements

### Potential Improvements
- **Custom Layout Presets**: Save and recall specific arrangements
- **Context-Aware Organization**: Different layouts for different project types
- **Animation Transitions**: Smooth animated transitions between layouts
- **Layout History**: Undo/redo organization actions

### Extensibility
The system is designed to accommodate additional pane types and layout strategies as the application evolves.