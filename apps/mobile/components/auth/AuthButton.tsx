import React, { useState } from 'react';
import { TouchableOpacity, StyleSheet, Platform, View } from 'react-native';
import { Text } from '../index';
import { useConfectAuth } from '../../contexts/SimpleConfectAuthContext';

export const AuthButton: React.FC = () => {
  const { user, isAuthenticated, isLoading, login, logout } = useConfectAuth();
  const [localLoading, setLocalLoading] = useState(false);

  const handleLogin = async () => {
    setLocalLoading(true);
    try {
      await login();
    } finally {
      setLocalLoading(false);
    }
  };

  const handleLogout = async () => {
    setLocalLoading(true);
    try {
      await logout();
    } finally {
      setLocalLoading(false);
    }
  };

  const showLoading = isLoading || localLoading;

  const renderButtonWithShadow = (onPress: (() => void) | undefined, disabled: boolean, text: string, buttonStyle: any) => (
    <View style={styles.buttonContainer}>
      <View style={styles.buttonShadow} />
      <TouchableOpacity 
        style={[styles.button, buttonStyle]} 
        onPress={onPress}
        disabled={disabled}
      >
        <Text style={styles.buttonText}>{text}</Text>
      </TouchableOpacity>
    </View>
  );

  if (showLoading) {
    return renderButtonWithShadow(undefined, true, "Log in with GitHub", styles.buttonLoading);
  }

  if (isAuthenticated && user) {
    return renderButtonWithShadow(handleLogout, false, `Logout (${user.githubUsername})`, styles.buttonLogin);
  }

  return renderButtonWithShadow(handleLogin, false, "Log in with GitHub", styles.buttonLogin);
};

const styles = StyleSheet.create({
  buttonContainer: {
    position: 'relative',
  },
  buttonShadow: {
    position: 'absolute',
    top: 5,
    left: 5,
    right: -5,
    bottom: -5,
    borderWidth: 1,
    borderColor: '#ffffff',
    borderRadius: 0,
  },
  button: {
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderRadius: 0,
    alignItems: 'center',
    justifyContent: 'center',
    position: 'relative',
    zIndex: 1,
  },
  buttonLogin: {
    backgroundColor: '#000000',
    borderWidth: 1,
    borderColor: '#ffffff',
  },
  buttonLoading: {
    backgroundColor: '#000000',
    borderWidth: 1,
    borderColor: '#ffffff',
    opacity: 0.6,
  },
  buttonText: {
    color: '#ffffff',
    fontSize: 14,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    } as const),
  },
});