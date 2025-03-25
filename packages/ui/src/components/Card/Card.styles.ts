import { StyleSheet } from 'react-native';

export const COLORS = {
  white: '#FFFFFF',
  transparent: 'transparent',
};

export const getCardPadding = (padding: 'small' | 'medium' | 'large') => {
  switch (padding) {
    case 'small':
      return 8;
    case 'medium':
      return 16;
    case 'large':
      return 24;
    default:
      return 16;
  }
};

export const styles = StyleSheet.create({
  card: {
    backgroundColor: COLORS.transparent,
    borderColor: COLORS.white,
    borderWidth: 1,
    borderRadius: 0,
  },
});
