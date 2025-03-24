import { StyleSheet } from 'react-native';

export const COLORS = {
  primary: '#007AFF',
  secondary: '#5AC8FA',
  tertiary: '#EFEFF4',
  white: '#FFFFFF',
  black: '#000000',
  disabled: '#CDCDCD',
};

export const getButtonStyles = (variant: 'primary' | 'secondary' | 'tertiary', disabled: boolean) => {
  const baseStyle = {
    borderRadius: 8,
    paddingHorizontal: 16,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
  };

  if (disabled) {
    return {
      ...baseStyle,
      backgroundColor: COLORS.disabled,
    };
  }

  switch (variant) {
    case 'primary':
      return {
        ...baseStyle,
        backgroundColor: COLORS.primary,
      };
    case 'secondary':
      return {
        ...baseStyle,
        backgroundColor: COLORS.secondary,
      };
    case 'tertiary':
      return {
        ...baseStyle,
        backgroundColor: COLORS.tertiary,
      };
    default:
      return baseStyle;
  }
};

export const getButtonHeight = (size: 'small' | 'medium' | 'large') => {
  switch (size) {
    case 'small':
      return 32;
    case 'medium':
      return 44;
    case 'large':
      return 56;
    default:
      return 44;
  }
};

export const getTextStyle = (variant: 'primary' | 'secondary' | 'tertiary', disabled: boolean) => {
  const baseStyle = {
    fontWeight: '600' as const,
    textAlign: 'center' as const,
  };

  if (disabled) {
    return {
      ...baseStyle,
      color: COLORS.white,
    };
  }

  switch (variant) {
    case 'primary':
    case 'secondary':
      return {
        ...baseStyle,
        color: COLORS.white,
      };
    case 'tertiary':
      return {
        ...baseStyle,
        color: COLORS.black,
      };
    default:
      return baseStyle;
  }
};

export const getTextSize = (size: 'small' | 'medium' | 'large') => {
  switch (size) {
    case 'small':
      return 14;
    case 'medium':
      return 16;
    case 'large':
      return 18;
    default:
      return 16;
  }
};

export const styles = StyleSheet.create({
  activityIndicator: {
    marginRight: 8,
  },
});
