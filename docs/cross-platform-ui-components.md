# Cross-Platform UI Components in OpenAgents

This document explains the implementation of shared UI components that work across the Tauri-based Coder application and React Native-based Onyx application.

## Overview

The OpenAgents project has multiple applications targeting different platforms:
- **Coder**: A desktop/web application built with React and Tauri
- **Onyx**: A mobile application built with React Native

To maintain consistent UI and reduce code duplication, we've implemented a shared UI component library (`@openagents/ui`) that works seamlessly across both platforms.

## Implementation Approach

We've chosen a unified implementation approach using **React Native Web**, which allows React Native components to be rendered in web environments. This approach has several advantages:

1. **Single codebase** for UI components
2. **Consistent behavior** across platforms
3. **Reduced maintenance** compared to platform-specific implementations
4. **Well-established pattern** used by many large companies

## Architecture

The shared UI package is structured as follows:

```
packages/ui/
├── src/
│   ├── components/
│   │   ├── Button/
│   │   │   ├── Button.tsx             # Main implementation using React Native components
│   │   │   ├── Button.styles.ts       # Shared styles
│   │   │   ├── Button.types.ts        # TypeScript interfaces
│   │   │   └── index.ts               # Re-export
│   │   └── ... (other components)
│   └── index.ts                       # Main export
├── package.json
├── tsconfig.json
└── README.md
```

### Key Technical Aspects

1. **React Native Components as the Base**
   - All components are built using React Native primitives (`View`, `Text`, `TouchableOpacity`, etc.)
   - These components are automatically adapted for web by react-native-web

2. **Platform-Specific Optimizations**
   - For more complex components, platform-specific optimizations can be added using `.native.tsx` and `.web.tsx` extensions
   - The appropriate file is automatically selected during the build process

3. **Build Configuration**
   - The package is built with both ESM and CommonJS formats for maximum compatibility
   - For Coder (web), Vite is configured to alias 'react-native' to 'react-native-web'

4. **Styling System**
   - Styles are created using React Native's `StyleSheet` API
   - This ensures consistent styling across platforms while leveraging platform-specific optimizations

## Component Example: Button

The `Button` component demonstrates the cross-platform approach:

```tsx
// Button.tsx
import React from 'react';
import { Text, TouchableOpacity, ActivityIndicator, View } from 'react-native';
import { ButtonProps } from './Button.types';
import { getButtonStyles, getButtonHeight, getTextStyle, getTextSize, styles, COLORS } from './Button.styles';

export const Button = ({
  label,
  variant = 'primary',
  size = 'medium',
  loading = false,
  disabled = false,
  style,
  onPress,
  ...rest
}: ButtonProps) => {
  const buttonStyles = getButtonStyles(variant, disabled);
  const height = getButtonHeight(size);
  const textStyles = getTextStyle(variant, disabled);
  const fontSize = getTextSize(size);

  return (
    <TouchableOpacity
      style={[
        buttonStyles,
        { height },
        style,
      ]}
      disabled={disabled || loading}
      onPress={onPress}
      {...rest}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        {loading && (
          <ActivityIndicator 
            size="small" 
            color={variant === 'tertiary' ? COLORS.black : COLORS.white} 
            style={styles.activityIndicator} 
          />
        )}
        <Text style={[textStyles, { fontSize }]}>{label}</Text>
      </View>
    </TouchableOpacity>
  );
};
```

This component renders as expected in both environments:
- In Onyx (React Native), it uses native TouchableOpacity and Text components
- In Coder (Web/Tauri), react-native-web converts these components to appropriate HTML elements with correct styling and behavior

## Integration with Applications

### In Coder (Tauri/React)

1. **Dependencies**
   ```json
   {
     "dependencies": {
       "@openagents/ui": "*",
       "react-native-web": "^0.19.10"
     }
   }
   ```

2. **Vite Configuration**
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

3. **Usage**
   ```tsx
   import { Button } from "@openagents/ui";

   function MyComponent() {
     return (
       <Button 
         label="Primary Button" 
         variant="primary" 
         onPress={() => console.log('Button pressed')} 
       />
     );
   }
   ```

### In Onyx (React Native)

1. **Dependencies**
   ```json
   {
     "dependencies": {
       "@openagents/ui": "*"
     }
   }
   ```

2. **Usage**
   ```tsx
   import { Button as SharedButton } from "@openagents/ui";

   function MyComponent() {
     return (
       <SharedButton 
         label="Primary Button" 
         variant="primary" 
         onPress={() => console.log('Button pressed')} 
       />
     );
   }
   ```

## Benefits of This Approach

1. **Consistency**: Users experience the same UI components regardless of platform
2. **Developer Efficiency**: Developers only need to maintain one component library
3. **Testing Efficiency**: Testing can be centralized on the shared components
4. **Feature Development**: New UI features can be added once and work everywhere

## Limitations and Considerations

1. **Performance**: While react-native-web is optimized, there may be performance differences compared to native web components
2. **Platform-Specific Features**: Some advanced features might require platform-specific implementations
3. **Bundle Size**: Adding react-native-web increases the bundle size for web applications

## Future Enhancements

The current implementation focuses on basic UI components. Future enhancements could include:

1. **Theme System**: Adding a shared theme system that works across platforms
2. **Animation Library**: Implementing animations that work consistently
3. **Form Components**: Creating form controls with consistent validation
4. **Storybook Integration**: Adding Storybook to showcase and test components
5. **Component Testing**: Implementing thorough testing for all shared components

## Conclusion

The shared UI component library leverages React Native Web to create a unified UI experience across all OpenAgents applications. This approach significantly reduces code duplication while ensuring a consistent user experience regardless of platform.

As the application suite grows, this shared foundation will continue to provide value by enabling rapid development of new features with consistent UI across all platforms.