// Type assertions for Radix UI components to fix TypeScript strict JSX checking

declare module '@radix-ui/react-scroll-area' {
  import * as React from 'react';
  
  export const Root: React.ComponentType<any>;
  export const Viewport: React.ComponentType<any>;
  export const Scrollbar: React.ComponentType<any>;
  export const Thumb: React.ComponentType<any>;
  export const Corner: React.ComponentType<any>;
}

declare module '@radix-ui/react-separator' {
  import * as React from 'react';
  
  export const Root: React.ComponentType<any>;
}

declare module '@radix-ui/react-dialog' {
  import * as React from 'react';
  
  export const Root: React.ComponentType<any>;
  export const Trigger: React.ComponentType<any>;
  export const Close: React.ComponentType<any>;
  export const Overlay: React.ComponentType<any>;
  export const Content: React.ComponentType<any>;
  export const Title: React.ComponentType<any>;
  export const Description: React.ComponentType<any>;
  export const Portal: React.ComponentType<any>;
}

declare module '@radix-ui/react-tooltip' {
  import * as React from 'react';
  
  export const Provider: React.ComponentType<any>;
  export const Root: React.ComponentType<any>;
  export const Trigger: React.ComponentType<any>;
  export const Content: React.ComponentType<any>;
  export const Arrow: React.ComponentType<any>;
  export const Portal: React.ComponentType<any>;
}

declare module '@radix-ui/react-slot' {
  import * as React from 'react';
  
  export const Slot: React.ComponentType<any>;
}