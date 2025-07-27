import React from 'react';
import { TouchableOpacity, StyleSheet, Platform } from 'react-native';
import { Text } from '../index';
import { useAuth } from '../../contexts/AuthContext';

export const AuthButton: React.FC = () => {
  const { user, isAuthenticated, isLoading, login, logout } = useAuth();

  if (isLoading) {
    return (
      <TouchableOpacity style={[styles.button, styles.buttonLoading]} disabled>
        <Text style={styles.buttonText}>Loading...</Text>
      </TouchableOpacity>
    );
  }

  if (isAuthenticated && user) {
    return (
      <TouchableOpacity style={[styles.button, styles.buttonLogout]} onPress={logout}>
        <Text style={styles.buttonText}>
          Logout ({user.githubUsername})
        </Text>
      </TouchableOpacity>
    );
  }

  return (
    <TouchableOpacity style={[styles.button, styles.buttonLogin]} onPress={login}>
      <Text style={styles.buttonText}>Login with GitHub</Text>
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
    backgroundColor: '#22c55e',
  },
  buttonLogout: {
    backgroundColor: '#ef4444',
  },
  buttonLoading: {
    backgroundColor: '#6b7280',
  },
  buttonText: {
    color: '#ffffff',
    fontSize: 14,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
});