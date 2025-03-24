# @openagents/ui

A shared UI component library for OpenAgents applications. These components work across Tauri/React (desktop/web) and React Native (mobile) applications.

## Installation

```bash
# Using npm
npm install @openagents/ui

# Using yarn
yarn add @openagents/ui
```

## Configuration

### For React/Tauri apps

Add react-native-web as a dependency:

```bash
yarn add react-native-web
```

Configure Vite to alias 'react-native' to 'react-native-web':

```typescript
// vite.config.ts
export default defineConfig({
  // ...other config
  resolve: {
    alias: {
      'react-native': 'react-native-web',
    },
    extensions: ['.web.tsx', '.web.ts', '.web.jsx', '.web.js', '.tsx', '.ts', '.jsx', '.js'],
  },
});
```

### For React Native apps

No special configuration is needed.

## Usage

```tsx
import { Button } from '@openagents/ui';

function MyComponent() {
  return (
    <Button 
      label="Press Me"
      variant="primary"
      onPress={() => console.log('Button pressed')}
    />
  );
}
```

## Available Components

### Button

A cross-platform button component with multiple variants, sizes, and states.

```tsx
<Button 
  label="Primary Button" 
  variant="primary"  // 'primary' | 'secondary' | 'tertiary'
  size="medium"      // 'small' | 'medium' | 'large'
  loading={false}    // Show loading indicator
  disabled={false}   // Disable the button
  onPress={() => {}} // Press handler
/>
```

## Development

To work on this package:

```bash
# Install dependencies
yarn install

# Build the package
yarn build

# Watch for changes and rebuild
yarn dev
```