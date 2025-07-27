import React from 'react';
import { render, fireEvent } from '@testing-library/react-native';
import { AuthButton } from '../../../components/auth/AuthButton';
import { useAuth } from '../../../contexts/AuthContext';

// Mock the AuthContext
jest.mock('../../../contexts/AuthContext', () => ({
  useAuth: jest.fn(),
}));

const mockUseAuth = useAuth as jest.MockedFunction<typeof useAuth>;

describe('AuthButton', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('should show loading state', () => {
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: true,
      login: jest.fn(),
      logout: jest.fn(),
      token: null,
    });

    const { getByText } = render(<AuthButton />);
    
    expect(getByText('Loading...')).toBeTruthy();
  });

  it('should show login button when unauthenticated', () => {
    const mockLogin = jest.fn();
    
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: false,
      login: mockLogin,
      logout: jest.fn(),
      token: null,
    });

    const { getByText } = render(<AuthButton />);
    
    const loginButton = getByText('Login with GitHub');
    expect(loginButton).toBeTruthy();
    
    fireEvent.press(loginButton);
    expect(mockLogin).toHaveBeenCalledTimes(1);
  });

  it('should show logout button when authenticated', () => {
    const mockLogout = jest.fn();
    const mockUser = {
      id: 'user-123',
      email: 'test@example.com',
      githubId: 'github-123',
      githubUsername: 'testuser',
    };
    
    mockUseAuth.mockReturnValue({
      user: mockUser,
      isAuthenticated: true,
      isLoading: false,
      login: jest.fn(),
      logout: mockLogout,
      token: 'mock-token',
    });

    const { getByText } = render(<AuthButton />);
    
    const logoutButton = getByText('Logout (testuser)');
    expect(logoutButton).toBeTruthy();
    
    fireEvent.press(logoutButton);
    expect(mockLogout).toHaveBeenCalledTimes(1);
  });

  it('should disable button when loading', () => {
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: true,
      login: jest.fn(),
      logout: jest.fn(),
      token: null,
    });

    const { getByText } = render(<AuthButton />);
    
    const loadingButton = getByText('Loading...');
    expect(loadingButton.parent?.props.disabled).toBe(true);
  });

  it('should show correct styles for different states', () => {
    // Test login button styles
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: false,
      login: jest.fn(),
      logout: jest.fn(),
      token: null,
    });

    const { getByText, rerender } = render(<AuthButton />);
    
    const loginButton = getByText('Login with GitHub');
    expect(loginButton.parent?.props.style).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ backgroundColor: '#22c55e' })
      ])
    );

    // Test logout button styles
    const mockUser = {
      id: 'user-123',
      email: 'test@example.com',
      githubId: 'github-123',
      githubUsername: 'testuser',
    };
    
    mockUseAuth.mockReturnValue({
      user: mockUser,
      isAuthenticated: true,
      isLoading: false,
      login: jest.fn(),
      logout: jest.fn(),
      token: 'mock-token',
    });

    rerender(<AuthButton />);
    
    const logoutButton = getByText('Logout (testuser)');
    expect(logoutButton.parent?.props.style).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ backgroundColor: '#ef4444' })
      ])
    );

    // Test loading button styles
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: true,
      login: jest.fn(),
      logout: jest.fn(),
      token: null,
    });

    rerender(<AuthButton />);
    
    const loadingButton = getByText('Loading...');
    expect(loadingButton.parent?.props.style).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ backgroundColor: '#6b7280' })
      ])
    );
  });
});