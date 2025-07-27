import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { AuthProvider } from '@/contexts/AuthContext'
import { AuthButton } from '@/components/auth/AuthButton'

// Mock Convex
vi.mock('convex/react', () => ({
  useMutation: vi.fn(() => vi.fn()),
  useQuery: vi.fn(() => undefined),
}))

// Mock @tauri-apps/plugin-opener
const mockOpenUrl = vi.fn()
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: mockOpenUrl,
}))

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('Authentication Integration Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  describe('Full Authentication Flow', () => {
    it('should complete end-to-end authentication flow', async () => {
      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      // Initial state - should show login button
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      // Click login button
      const loginButton = screen.getByRole('button', { name: /login with github/i })
      fireEvent.click(loginButton)

      // Should attempt to open auth URL
      expect(mockOpenUrl).toHaveBeenCalledWith(
        expect.stringContaining('/authorize?provider=github&redirect_uri=openagents://auth/callback')
      )

      // Simulate successful authentication callback
      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
        avatar: 'https://github.com/avatar.png',
      }
      const mockToken = 'eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.test_payload.test_signature'

      // Simulate auth update event from Tauri backend
      const authUpdateEvent = new CustomEvent('auth-update', {
        detail: { token: mockToken, user: mockUser }
      })
      
      window.dispatchEvent(authUpdateEvent)

      // Should now show authenticated state
      await waitFor(() => {
        expect(screen.getByText('Test User')).toBeInTheDocument()
        expect(screen.getByRole('button', { name: /logout/i })).toBeInTheDocument()
      })

      // Verify user avatar is displayed
      const avatar = screen.getByRole('img')
      expect(avatar).toHaveAttribute('src', mockUser.avatar)
      expect(avatar).toHaveAttribute('alt', 'Test User avatar')

      // Test logout functionality
      const logoutButton = screen.getByRole('button', { name: /logout/i })
      fireEvent.click(logoutButton)

      // Should return to unauthenticated state
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
        expect(screen.queryByText('Test User')).not.toBeInTheDocument()
      })

      // Verify localStorage is cleared
      expect(localStorage.getItem('openauth_token')).toBeNull()
      expect(localStorage.getItem('openauth_user')).toBeNull()
    })

    it('should persist authentication across app restarts', async () => {
      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
        avatar: 'https://github.com/avatar.png',
      }
      const mockToken = 'persistent-jwt-token'

      // Pre-populate localStorage as if user was previously authenticated
      localStorage.setItem('openauth_token', mockToken)
      localStorage.setItem('openauth_user', JSON.stringify(mockUser))

      // Render component - should restore auth state
      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      // Should immediately show authenticated state
      await waitFor(() => {
        expect(screen.getByText('Test User')).toBeInTheDocument()
        expect(screen.getByRole('button', { name: /logout/i })).toBeInTheDocument()
      })

      // Verify avatar is displayed
      expect(screen.getByRole('img')).toHaveAttribute('src', mockUser.avatar)
    })

    it('should handle authentication errors gracefully', async () => {
      mockOpenUrl.mockRejectedValueOnce(new Error('Network error'))

      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      const loginButton = screen.getByRole('button', { name: /login with github/i })

      // Click should trigger error
      await expect(async () => {
        fireEvent.click(loginButton)
        await new Promise(resolve => setTimeout(resolve, 100)) // Wait for async operation
      }).rejects.toThrow('Network error')

      // Should still show login button after error
      expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
    })

    it('should handle corrupted localStorage data', async () => {
      // Simulate corrupted data
      localStorage.setItem('openauth_token', 'valid-token')
      localStorage.setItem('openauth_user', 'invalid-json-data')

      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      // Should gracefully handle corruption and show login button
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      // Should have cleared corrupted data
      expect(localStorage.getItem('openauth_token')).toBeNull()
      expect(localStorage.getItem('openauth_user')).toBeNull()
    })
  })

  describe('Authentication State Consistency', () => {
    it('should maintain consistent state during rapid login/logout cycles', async () => {
      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      const mockUser = {
        id: 'user123',
        email: 'test@example.com',
        name: 'Test User',
        githubId: 'github|12345',
        githubUsername: 'testuser',
      }
      const mockToken = 'rapid-cycle-token'

      // Rapid login
      const authUpdateEvent = new CustomEvent('auth-update', {
        detail: { token: mockToken, user: mockUser }
      })
      window.dispatchEvent(authUpdateEvent)

      await waitFor(() => {
        expect(screen.getByText('Test User')).toBeInTheDocument()
      })

      // Rapid logout
      const logoutButton = screen.getByRole('button', { name: /logout/i })
      fireEvent.click(logoutButton)

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      // Rapid login again
      window.dispatchEvent(authUpdateEvent)

      await waitFor(() => {
        expect(screen.getByText('Test User')).toBeInTheDocument()
      })

      // State should be consistent
      expect(screen.queryByRole('button', { name: /login with github/i })).not.toBeInTheDocument()
    })

    it('should handle multiple auth update events correctly', async () => {
      render(
        <AuthProvider>
          <AuthButton />
        </AuthProvider>
      )

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /login with github/i })).toBeInTheDocument()
      })

      const firstUser = {
        id: 'user1',
        email: 'user1@example.com',
        name: 'First User',
        githubId: 'github|1',
        githubUsername: 'user1',
      }

      const secondUser = {
        id: 'user2',
        email: 'user2@example.com',
        name: 'Second User',
        githubId: 'github|2',
        githubUsername: 'user2',
      }

      // First auth update
      window.dispatchEvent(new CustomEvent('auth-update', {
        detail: { token: 'token1', user: firstUser }
      }))

      await waitFor(() => {
        expect(screen.getByText('First User')).toBeInTheDocument()
      })

      // Second auth update (user switching)
      window.dispatchEvent(new CustomEvent('auth-update', {
        detail: { token: 'token2', user: secondUser }
      }))

      await waitFor(() => {
        expect(screen.getByText('Second User')).toBeInTheDocument()
        expect(screen.queryByText('First User')).not.toBeInTheDocument()
      })

      // Verify localStorage has latest user
      expect(localStorage.getItem('openauth_token')).toBe('token2')
      expect(JSON.parse(localStorage.getItem('openauth_user') || '{}')).toEqual(secondUser)
    })
  })
})