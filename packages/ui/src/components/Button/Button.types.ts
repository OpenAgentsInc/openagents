import { TouchableOpacityProps } from 'react-native';

export interface ButtonProps extends TouchableOpacityProps {
  /** Button label text */
  label: string;
  /** Button variant */
  variant?: 'primary' | 'secondary' | 'tertiary';
  /** Size of the button */
  size?: 'small' | 'medium' | 'large';
  /** Is the button in a loading state? */
  loading?: boolean;
  /** Is the button disabled? */
  disabled?: boolean;
  /** Left icon name */
  leftIcon?: string;
  /** Right icon name */
  rightIcon?: string;
  /** Icon color */
  iconColor?: string;
  /** Function to render the icon */
  renderIcon?: (iconName: string) => React.ReactNode;
}
