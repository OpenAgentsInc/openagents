import { StyleSheet } from 'react-native';

export const COLORS = {
  background: '#000000',
  border: '#FFFFFF',
  text: '#FFFFFF',
  success: '#4CAF50',
  error: '#F44336',
  warning: '#FF9800',
};

export const styles = StyleSheet.create({
  container: {
    position: 'absolute',
    left: 0,
    right: 0,
    top: 0,
    zIndex: 999,
    alignItems: 'center',
    paddingTop: 40,
    paddingHorizontal: 16,
  },
  toast: {
    width: '100%',
    backgroundColor: COLORS.background,
    borderRadius: 0,
    padding: 16,
    marginBottom: 8,
    flexDirection: 'row',
    alignItems: 'center',
    borderWidth: 1,
    borderColor: COLORS.border,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.25,
    shadowRadius: 3.84,
    elevation: 5,
  },
  contentContainer: {
    flex: 1,
    marginLeft: 12,
  },
  title: {
    color: COLORS.text,
    fontWeight: '600',
    fontSize: 14,
    fontFamily: 'Berkeley Mono',
    marginBottom: 4,
  },
  message: {
    color: COLORS.text,
    fontSize: 14,
    fontFamily: 'Berkeley Mono',
  },
  closeButton: {
    marginLeft: 16,
  },
  closeIcon: {
    color: COLORS.text,
  },
  actionContainer: {
    marginLeft: 8,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
  },
});

export const getVariantStyles = (variant: 'default' | 'success' | 'error' | 'warning') => {
  switch (variant) {
    case 'success':
      return { borderColor: COLORS.success };
    case 'error':
      return { borderColor: COLORS.error };
    case 'warning':
      return { borderColor: COLORS.warning };
    default:
      return { borderColor: COLORS.border };
  }
};
