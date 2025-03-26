// Type definitions for our custom Ionicons implementation
declare module '@expo/vector-icons' {
  import * as React from 'react';

  export interface IoniconsProps {
    name: any; // Allow any string as name
    size?: number;
    color?: string;
    style?: React.CSSProperties;
  }

  export const Ionicons: React.FC<IoniconsProps>;
}