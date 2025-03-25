import { ViewProps } from 'react-native';

export interface CardProps extends ViewProps {
  /** Content to be rendered inside the card */
  children: React.ReactNode;
  /** Optional padding for the card content */
  padding?: 'small' | 'medium' | 'large';
  /** Optional border width */
  borderWidth?: number;
}
