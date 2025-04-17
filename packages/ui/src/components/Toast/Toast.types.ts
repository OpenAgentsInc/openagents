import { ViewStyle } from 'react-native';

export interface ToastProps {
  /** Toast message */
  message: string;
  /** Optional title for the toast */
  title?: string;
  /** Toast variant */
  variant?: 'default' | 'success' | 'error' | 'warning';
  /** Duration in milliseconds before auto-close (0 for no auto-close) */
  duration?: number;
  /** Optional icon to display */
  icon?: React.ReactNode;
  /** Optional action component to display */
  action?: React.ReactNode;
  /** Callback when toast is closed */
  onClose?: () => void;
  /** Callback when toast is pressed */
  onPress?: () => void;
  /** Custom styles */
  style?: ViewStyle;
  visible: boolean;
  type?: 'success' | 'error' | 'info';
}

export interface ToastProviderProps {
  /** Children components */
  children: React.ReactNode;
}

export interface ToastOptions {
  /** Toast message */
  message: string;
  /** Toast type */
  type?: 'success' | 'error' | 'info';
  /** Duration in milliseconds before auto-close */
  duration?: number;
  /** Optional ID for the toast */
  id?: string;
}

export interface ToastContextType {
  /** Show a toast with the given options */
  show: (options: ToastOptions) => string;
  /** Update an existing toast by id */
  update: (id: string, options: ToastOptions) => void;
  /** Close a toast by id */
  close: (id: string) => void;
  /** Close all toasts */
  closeAll: () => void;
  /** Show a toast with the given options */
  showToast: (options: ToastOptions) => void;
  /** Hide a toast by id */
  hideToast: (id: string) => void;
}
