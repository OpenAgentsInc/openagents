import { ViewProps } from 'react-native';

// Use a custom interface that doesn't extend ViewProps directly to avoid React 19 ReactNode type conflict
export interface CardProps {
  /** Content to be rendered inside the card */
  children: React.ReactNode;
  /** Optional padding for the card content */
  padding?: 'small' | 'medium' | 'large';
  /** Optional border width */
  borderWidth?: number;
  /** Additional style properties */
  style?: ViewProps['style'];
  /** All other view props */
  [key: string]: any;
}
