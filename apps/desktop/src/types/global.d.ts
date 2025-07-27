// Global type declarations for React compatibility

import * as React from 'react';

declare module 'react' {
  // Override ReactNode to be compatible with React 18 (remove bigint)
  type ReactNode = 
    | string
    | number
    | boolean
    | React.ReactElement<any, any>
    | React.ReactFragment
    | React.ReactPortal
    | Iterable<ReactNode>
    | null
    | undefined;
}