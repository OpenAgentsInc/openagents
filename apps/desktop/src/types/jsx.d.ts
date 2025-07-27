// Global type declarations to fix JSX component recognition issues
// with TypeScript 5.6.2 + React 18.3.1 + third-party libraries

declare module 'lucide-react' {
  import { LucideProps } from 'lucide-react';
  import { ForwardRefExoticComponent, RefAttributes } from 'react';
  
  export const Plus: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const LayoutGrid: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const History: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const BarChart: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Settings: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Hand: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const FileText: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const GitBranch: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const RefreshCw: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Calendar: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Folder: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const MessageSquare: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const XIcon: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const PanelLeftIcon: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Loader2: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const TrendingUp: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Clock: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const X: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
  export const Eye: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>;
}

// Fix for Radix UI components
declare module '@radix-ui/react-dialog' {
  export * from '@radix-ui/react-dialog/dist/index';
}

declare module '@radix-ui/react-scroll-area' {
  export * from '@radix-ui/react-scroll-area/dist/index';
}

declare module '@radix-ui/react-separator' {
  export * from '@radix-ui/react-separator/dist/index';
}

declare module '@radix-ui/react-tooltip' {
  export * from '@radix-ui/react-tooltip/dist/index';
}

declare module '@radix-ui/react-slot' {
  export * from '@radix-ui/react-slot/dist/index';
}

// Global JSX fix for component props
declare global {
  namespace JSX {
    interface IntrinsicAttributes {
      [key: string]: any;
    }
  }
}