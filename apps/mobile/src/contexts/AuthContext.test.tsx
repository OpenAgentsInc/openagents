import React from 'react';
import { render, fireEvent, waitFor } from '@testing-library/react-native';
import { Alert, Text, TouchableOpacity } from 'react-native';
import * as SecureStore from 'expo-secure-store';
import * as AuthSession from 'expo-auth-session';
import { AuthProvider, useAuth } from '../../contexts/AuthContext';

// Test component to access the auth context
const TestComponent = () => {
  const { user, isAuthenticated, isLoading, login, logout, token } = useAuth();
  
  return (
    <>
      <Text testID="authenticated">{isAuthenticated.toString()}</Text>
      <Text testID="loading">{isLoading.toString()}</Text>
      <Text testID="token">{token || 'null'}</Text>
      <Text testID="user">{user ? user.githubUsername : 'null'}</Text>
      <TouchableOpacity testID="login-button" onPress={login}>
        <Text>Login</Text>
      </TouchableOpacity>
      <TouchableOpacity testID="logout-button" onPress={logout}>
        <Text>Logout</Text>
      </TouchableOpacity>
    </>
  );
};

// Mock fetch for token exchange
const mockFetch = global.fetch as jest.MockedFunction<typeof fetch>;

describe('AuthContext', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockFetch.mockClear();
  });

  it('should provide initial unauthenticated state', () => {
    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    expect(getByTestId('authenticated').children[0]).toBe('false');
    expect(getByTestId('token').children[0]).toBe('null');
    expect(getByTestId('user').children[0]).toBe('null');
  });

  it('should restore authentication from secure storage', async () => {
    const mockUser = {
      id: 'user-123',
      email: 'test@example.com',
      githubId: 'github-123',
      githubUsername: 'testuser',
    };

    (SecureStore.getItemAsync as jest.Mock)
      .mockResolvedValueOnce('mock-token')
      .mockResolvedValueOnce(JSON.stringify(mockUser));

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    expect(getByTestId('authenticated').children[0]).toBe('true');
    expect(getByTestId('token').children[0]).toBe('mock-token');
    expect(getByTestId('user').children[0]).toBe('testuser');
  });

  it('should handle successful login flow', async () => {
    const mockUser = {
      id: 'user-123',
      email: 'test@example.com',
      githubId: 'github-123',
      githubUsername: 'testuser',
    };

    // Mock successful token exchange
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        access_token: 'new-token',
        user: mockUser,
      }),
    } as Response);

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    fireEvent.press(getByTestId('login-button'));

    await waitFor(() => {
      expect(getByTestId('authenticated').children[0]).toBe('true');
    });

    expect(getByTestId('token').children[0]).toBe('new-token');
    expect(getByTestId('user').children[0]).toBe('testuser');
    expect(SecureStore.setItemAsync).toHaveBeenCalledWith('openauth_token', 'new-token');
    expect(SecureStore.setItemAsync).toHaveBeenCalledWith('openauth_user', JSON.stringify(mockUser));
  });

  it('should handle failed token exchange', async () => {
    const alertSpy = jest.spyOn(Alert, 'alert');

    // Mock failed token exchange
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 400,
      text: async () => 'Bad Request',
    } as Response);

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    fireEvent.press(getByTestId('login-button'));

    await waitFor(() => {
      expect(alertSpy).toHaveBeenCalledWith('Login Error', 'An error occurred during login. Please try again.');
    });

    expect(getByTestId('authenticated').children[0]).toBe('false');
  });

  it('should handle cancelled OAuth flow', async () => {
    // Mock cancelled OAuth
    const mockRequest = new (AuthSession.AuthRequest as jest.Mock)();
    mockRequest.promptAsync = jest.fn().mockResolvedValue({
      type: 'cancel',
    });

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    fireEvent.press(getByTestId('login-button'));

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    expect(getByTestId('authenticated').children[0]).toBe('false');
  });

  it('should handle logout', async () => {
    const mockUser = {
      id: 'user-123',
      email: 'test@example.com',
      githubId: 'github-123',
      githubUsername: 'testuser',
    };

    // Setup initial authenticated state
    (SecureStore.getItemAsync as jest.Mock)
      .mockResolvedValueOnce('mock-token')
      .mockResolvedValueOnce(JSON.stringify(mockUser));

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('authenticated').children[0]).toBe('true');
    });

    fireEvent.press(getByTestId('logout-button'));

    await waitFor(() => {
      expect(getByTestId('authenticated').children[0]).toBe('false');
    });

    expect(getByTestId('token').children[0]).toBe('null');
    expect(getByTestId('user').children[0]).toBe('null');
    expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith('openauth_token');
    expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith('openauth_user');
  });

  it('should clear invalid stored auth data on error', async () => {
    (SecureStore.getItemAsync as jest.Mock)
      .mockRejectedValueOnce(new Error('SecureStore error'));

    const { getByTestId } = render(
      <AuthProvider>
        <TestComponent />
      </AuthProvider>
    );

    await waitFor(() => {
      expect(getByTestId('loading').children[0]).toBe('false');
    });

    expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith('openauth_token');
    expect(SecureStore.deleteItemAsync).toHaveBeenCalledWith('openauth_user');
    expect(getByTestId('authenticated').children[0]).toBe('false');
  });

  it('should throw error when useAuth is used outside AuthProvider', () => {
    const spy = jest.spyOn(console, 'error').mockImplementation();
    
    expect(() => {
      render(<TestComponent />);
    }).toThrow('useAuth must be used within an AuthProvider');

    spy.mockRestore();
  });
});