// Type definitions for our custom Ionicons implementation
declare module '@expo/vector-icons' {
  import * as React from 'react';

  // Common props interface
  interface IconBaseProps {
    size?: number;
    color?: string;
    style?: React.CSSProperties;
  }

  // Ionicons props interface
  export interface IoniconsProps extends IconBaseProps {
    name: string;
  }

  // MaterialCommunityIcons props interface
  export interface MaterialCommunityIconsProps extends IconBaseProps {
    name: string;
  }

  // Component type definitions as React FCs
  export const Ionicons: React.FC<IoniconsProps>;
  export const MaterialCommunityIcons: React.FC<MaterialCommunityIconsProps>;
}