# Theme Switcher Documentation

## Overview

The OpenAgents theme system provides a flexible, CSS-based theming mechanism that allows users to switch between multiple color schemes. The system is built on CSS custom properties (CSS variables) and uses the `:root:has()` selector for dynamic theme application.

## Architecture

### File Structure

```
apps/openagents.com/
├── public/
│   └── webtui/
│       ├── theme-zinc.css      # Default dark theme
│       ├── theme-ayu.css       # Ayu dark theme
│       ├── theme-catppuccin.css# Catppuccin dark theme
│       ├── theme-flexoki.css   # Flexoki dark theme
│       ├── theme-gruvbox.css   # Gruvbox dark theme
│       ├── theme-monokai.css   # Monokai dark theme
│       ├── theme-nord.css      # Nord dark theme
│       ├── theme-onedark.css   # One Dark theme
│       ├── theme-tokyonight.css# Tokyo Night dark theme
│       └── theme-tron.css      # Tron neon theme
├── src/
│   ├── components/
│   │   ├── shared-header.ts    # Header with integrated theme switcher
│   │   └── theme-switcher.ts   # Standalone theme switcher component
│   └── styles.ts               # Main styles with theme imports
```

### How Themes Work

1. **CSS Custom Properties**: Each theme defines a set of CSS variables that control colors throughout the application
2. **Dynamic Class Application**: When a user selects a theme, a class like `theme-nord` is added to the `<body>` element
3. **CSS Selector Magic**: The `:root:has(.theme-nord)` selector applies the theme's color variables when the body has that class
4. **Persistence**: Theme preference is saved to localStorage and restored on page load

## Theme File Format

Each theme file follows this structure:

```css
/* Theme Name (Style) */
:root:has(.theme-name) {
  /* Backgrounds - from darkest to lightest */
  --background0: #000000; /* Main background */
  --background1: #111111; /* Slightly lighter */
  --background2: #222222; /* Hover/selection */
  --background3: #333333; /* Lightest background */
  
  /* Foregrounds - from dim to bright */
  --foreground0: #666666; /* Dim text */
  --foreground1: #999999; /* Normal text */
  --foreground2: #cccccc; /* Bright text */
  
  /* Accent colors */
  --accent: #0066cc;   /* Primary accent */
  --success: #00cc66;  /* Success/positive */
  --warning: #cccc00;  /* Warning/caution */
  --danger: #cc0000;   /* Error/danger */
  
  /* Overlays - for modals, shadows, etc */
  --overlay0: rgba(0, 0, 0, 0.7);
  --overlay1: rgba(0, 0, 0, 0.5);
  --overlay2: rgba(0, 0, 0, 0.3);
  --overlay3: rgba(0, 0, 0, 0.1);
}
```

## Theme Switcher Component

### Shared Header Implementation

The primary theme switcher is integrated into the shared header (`shared-header.ts`):

```typescript
<select id="theme-select" class="theme-select" onchange="switchTheme(this.value)">
  <option value="zinc">Zinc</option>
  <option value="ayu">Ayu</option>
  <option value="catppuccin">Catppuccin</option>
  <!-- ... more themes ... -->
</select>
```

### JavaScript Implementation

```javascript
function switchTheme(theme) {
  // Remove all existing theme classes
  document.body.classList.remove(
    'theme-zinc', 'theme-ayu', 'theme-catppuccin', 
    'theme-flexoki', 'theme-gruvbox', 'theme-monokai', 
    'theme-nord', 'theme-onedark', 'theme-tokyonight', 
    'theme-tron'
  );
  
  // Add the selected theme class
  document.body.classList.add('theme-' + theme);
  
  // Save preference to localStorage
  localStorage.setItem('openagents-theme', theme);
}
```

### Initialization

On page load, the saved theme is restored:

```javascript
(function() {
  const savedTheme = localStorage.getItem('openagents-theme') || 'zinc';
  const themeSelect = document.getElementById('theme-select');
  
  if (themeSelect) {
    themeSelect.value = savedTheme;
    switchTheme(savedTheme);
  }
})();
```

## CSS Variable Reference

### Background Colors
- `--background0`: Darkest background (main app background)
- `--background1`: Slightly lighter (cards, panels)
- `--background2`: Hover states, selections
- `--background3`: Lightest background (borders, dividers)

### Foreground Colors
- `--foreground0`: Dim text (secondary info, placeholders)
- `--foreground1`: Normal text (primary content)
- `--foreground2`: Bright text (headings, emphasis)

### Semantic Colors
- `--accent`: Primary brand/accent color
- `--success`: Success states, positive actions
- `--warning`: Warning states, caution messages
- `--danger`: Error states, destructive actions

### Overlay Colors
- `--overlay0` to `--overlay3`: Semi-transparent overlays for modals, dropdowns

## Adding New Themes

To add a new theme:

1. **Create Theme File**: Create a new CSS file in `apps/openagents.com/public/webtui/theme-{name}.css`

2. **Define Variables**: Follow the theme file format above, ensuring all required variables are defined

3. **Import Theme**: Add the import to `apps/openagents.com/src/styles.ts`:
   ```typescript
   @import '/webtui/theme-{name}.css';
   ```

4. **Update Theme Switchers**: Add the theme option to both:
   - `apps/openagents.com/src/components/shared-header.ts`
   - `apps/openagents.com/src/components/theme-switcher.ts`

5. **Update Class Removal**: Add the theme class to the remove list in the `switchTheme` function

## Technical Details

### Browser Support
- Requires support for CSS custom properties (all modern browsers)
- Uses `:has()` selector (Chrome 105+, Firefox 121+, Safari 15.4+)

### Performance
- Theme switching is instant (no page reload required)
- CSS variables cascade naturally through the DOM
- No JavaScript framework dependencies

### Default Theme
- Zinc dark theme is applied by default via CSS in `styles.ts`
- Provides fallback values if no theme class is present

### WebTUI Integration
- Themes work seamlessly with WebTUI components
- All WebTUI elements automatically use the CSS variables
- Custom components should use the same variables for consistency

## Best Practices

1. **Color Contrast**: Ensure sufficient contrast between foreground and background colors
2. **Consistency**: Use the semantic color variables appropriately (e.g., `--danger` for errors)
3. **Testing**: Test themes with both light and dark content
4. **Accessibility**: Consider users with color vision deficiencies

## Available Themes

- **Zinc**: Monochromatic dark theme with subtle grays
- **Ayu**: Elegant dark theme with blue accents
- **Catppuccin**: Pastel-inspired dark theme (Mocha variant)
- **Flexoki**: High-contrast dark theme with earthy tones
- **Gruvbox**: Retro warm color scheme
- **Monokai**: Classic code editor theme with vibrant colors
- **Nord**: Arctic-inspired palette with cool blues and grays
- **One Dark**: Atom editor's popular dark theme
- **Tokyo Night**: Modern dark theme with purple accents
- **Tron**: Neon-inspired theme with bright cyan highlights