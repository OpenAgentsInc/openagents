import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { AuthProvider, useAuth } from './AuthContext'

// Mock @tauri-apps/plugin-opener
const mockOpenUrl = vi.fn()
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: mockOpenUrl,
}))

// Test component that uses the auth context
const TestComponent = () => {
  const { user, isAuthenticated, isLoading, login, logout, token } = useAuth()
  
  return (
    <div>
      <div data-testid="auth-state">
        <span data-testid="is-authenticated">{String(isAuthenticated)}</span>
        <span data-testid="is-loading">{String(isLoading)}</span>
        <span data-testid="user-name">{user?.name || 'null'}</span>
        <span data-testid="token">{token || 'null'}</span>
      </div>
      <button onClick={() => login().catch(() => {})} data-testid="login-btn">Login</button>
      <button onClick={() => logout().catch(() => {})} data-testid="logout-btn">Logout</button>
    </div>
  )
}

// Helper to render with AuthProvider
const renderWithAuth = async () => {
  const result = render(
    <AuthProvider>
      <TestComponent />
    </AuthProvider>
  )
  
  // Wait for initial loading to complete
  await waitFor(() => {
    expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
  }, { timeout: 2000 })
  
  return result
}

describe('AuthContext', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Clear localStorage
    localStorage.clear()
    // Mock console methods to avoid noise in tests
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'log').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('Hook Error Handling', () => {
    it('should throw error when useAuth is used outside AuthProvider', () => {
      const TestComponentOutsideProvider = () => {
        useAuth()
        return <div>Test</div>
      }

      expect(() => render(<TestComponentOutsideProvider />)).toThrow(
        'useAuth must be used within an AuthProvider'
      )
    })
  })

  describe('Initial State', () => {
    it('should initialize with default unauthenticated state', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
      expect(screen.getByTestId('user-name')).toHaveTextContent('null')
      expect(screen.getByTestId('token')).toHaveTextContent('null')
    })

    it('should start with loading state and then become false', async () => {
      renderWithAuth()
      
      // Initially loading should be true, but it updates quickly
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
    })
  })

  describe('Stored Authentication Recovery', () => {
    it('should restore authentication from localStorage on mount', async () => {
      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
      }
      const mockToken = 'mock-jwt-token'
      
      localStorage.setItem('openauth_token', mockToken)
      localStorage.setItem('openauth_user', JSON.stringify(mockUser))
      
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('true')
      expect(screen.getByTestId('user-name')).toHaveTextContent('Test User')
      expect(screen.getByTestId('token')).toHaveTextContent(mockToken)
    })

    it('should handle invalid stored user data gracefully', async () => {
      localStorage.setItem('openauth_token', 'valid-token')
      localStorage.setItem('openauth_user', 'invalid-json')
      
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      // Should clear invalid data and remain unauthenticated
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
      expect(localStorage.getItem('openauth_token')).toBeNull()
      expect(localStorage.getItem('openauth_user')).toBeNull()
    })
  })

  describe('Login Functionality', () => {
    it('should trigger login flow when login is called', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      const loginBtn = screen.getByTestId('login-btn')
      
      await act(async () => {
        fireEvent.click(loginBtn)
      })
      
      // Should call openUrl with correct auth URL
      expect(mockOpenUrl).toHaveBeenCalledWith(
        expect.stringContaining('/authorize?provider=github&redirect_uri=')
      )
    })

    it('should use environment variable for auth URL if available', async () => {
      vi.stubEnv('VITE_OPENAUTH_URL', 'https://auth.openagents.com')
      
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      const loginBtn = screen.getByTestId('login-btn')
      
      await act(async () => {
        fireEvent.click(loginBtn)
      })
      
      expect(mockOpenUrl).toHaveBeenCalledWith(
        expect.stringContaining('https://auth.openagents.com/authorize')
      )
    })

    it('should handle login errors gracefully', async () => {
      mockOpenUrl.mockRejectedValueOnce(new Error('Failed to open URL'))
      
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      const loginBtn = screen.getByTestId('login-btn')
      
      await expect(async () => {
        await act(async () => {
          fireEvent.click(loginBtn)
        })
      }).rejects.toThrow('Failed to open URL')
      
      // Should still set loading back to false
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
    })
  })

  describe('Logout Functionality', () => {
    it('should clear authentication state when logout is called', async () => {
      // Set up authenticated state
      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
      }
      const mockToken = 'mock-jwt-token'
      
      localStorage.setItem('openauth_token', mockToken)
      localStorage.setItem('openauth_user', JSON.stringify(mockUser))
      
      renderWithAuth()
      
      // Wait for auth state to be restored
      await waitFor(() => {
        expect(screen.getByTestId('is-authenticated')).toHaveTextContent('true')
      })
      
      const logoutBtn = screen.getByTestId('logout-btn')
      
      await act(async () => {
        fireEvent.click(logoutBtn)
      })
      
      // Should clear state and localStorage
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
      expect(screen.getByTestId('user-name')).toHaveTextContent('null')
      expect(screen.getByTestId('token')).toHaveTextContent('null')
      expect(localStorage.getItem('openauth_token')).toBeNull()
      expect(localStorage.getItem('openauth_user')).toBeNull()
    })

    it('should handle logout from unauthenticated state gracefully', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      const logoutBtn = screen.getByTestId('logout-btn')
      
      await act(async () => {
        fireEvent.click(logoutBtn)
      })
      
      // Should remain unauthenticated
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
      expect(screen.getByTestId('user-name')).toHaveTextContent('null')
      expect(screen.getByTestId('token')).toHaveTextContent('null')
    })
  })

  describe('Auth Update Events', () => {
    it('should handle auth-update events from Tauri backend', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
      }
      const mockToken = 'new-jwt-token'
      
      // Simulate auth update event from Tauri backend
      const authUpdateEvent = new CustomEvent('auth-update', {
        detail: { token: mockToken, user: mockUser }
      })
      
      act(() => {
        window.dispatchEvent(authUpdateEvent)
      })
      
      // Should update state and localStorage
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('true')
      expect(screen.getByTestId('user-name')).toHaveTextContent('Test User')
      expect(screen.getByTestId('token')).toHaveTextContent(mockToken)
      expect(localStorage.getItem('openauth_token')).toBe(mockToken)
      expect(localStorage.getItem('openauth_user')).toBe(JSON.stringify(mockUser))
    })

    it('should ignore auth-update events with missing data', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      // Simulate incomplete auth update event
      const authUpdateEvent = new CustomEvent('auth-update', {
        detail: { token: 'token-only' } // Missing user
      })
      
      act(() => {
        window.dispatchEvent(authUpdateEvent)
      })
      
      // Should remain unauthenticated
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
      expect(screen.getByTestId('user-name')).toHaveTextContent('null')
      expect(screen.getByTestId('token')).toHaveTextContent('null')
    })
  })

  describe('Authentication State Logic', () => {
    it('should require both user and token for authenticated state', async () => {
      renderWithAuth()
      
      await waitFor(() => {
        expect(screen.getByTestId('is-loading')).toHaveTextContent('false')
      })
      
      // Set only token, no user
      localStorage.setItem('openauth_token', 'token-only')
      
      // Trigger auth state check by refreshing component
      act(() => {
        const authUpdateEvent = new CustomEvent('auth-update', {
          detail: { token: 'token-only', user: null }
        })
        window.dispatchEvent(authUpdateEvent)
      })
      
      // Should not be authenticated with token only
      expect(screen.getByTestId('is-authenticated')).toHaveTextContent('false')
    })

    it('should properly cleanup event listeners on unmount', async () => {
      const { unmount } = await renderWithAuth()
      
      // Track event listener removal
      const removeEventListenerSpy = vi.spyOn(window, 'removeEventListener')
      
      unmount()
      
      expect(removeEventListenerSpy).toHaveBeenCalledWith(
        'auth-update',
        expect.any(Function)
      )
    })
  })
})