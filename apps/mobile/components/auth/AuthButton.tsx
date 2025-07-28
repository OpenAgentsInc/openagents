import React, { useState } from 'react';
import { TouchableOpacity, StyleSheet, Platform } from 'react-native';
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

  if (showLoading) {
    return (
      <TouchableOpacity style={[styles.button, styles.buttonLoading]} disabled>
        <Text style={styles.buttonText}>Log in with GitHub</Text>
      </TouchableOpacity>
    );
  }

  if (isAuthenticated && user) {
    return (
      <TouchableOpacity style={[styles.button, styles.buttonLogin]} onPress={handleLogout}>
        <Text style={styles.buttonText}>
          Logout ({user.githubUsername})
        </Text>
      </TouchableOpacity>
    );
  }

  return (
    <TouchableOpacity style={[styles.button, styles.buttonLogin]} onPress={handleLogin}>
      <Text style={styles.buttonText}>Log in with GitHub</Text>
    </TouchableOpacity>
  );
};

const styles = StyleSheet.create({
  button: {
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderRadius: 8,
    alignItems: 'center',
    justifyContent: 'center',
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