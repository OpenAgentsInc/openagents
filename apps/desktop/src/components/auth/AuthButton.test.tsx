import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AuthButton } from './AuthButton'
import { useAuth } from '@/contexts/AuthContext'

// Mock the AuthContext
vi.mock('@/contexts/AuthContext', () => ({
  useAuth: vi.fn(),
}))

// Mock the UI Button component
vi.mock('@/components/ui/button', () => ({
  Button: ({ children, onClick, disabled, size, variant, ...props }: any) => (
    <button 
      onClick={onClick} 
      disabled={disabled} 
      data-size={size}
      data-variant={variant}
      {...props}
    >
      {children}
    </button>
  ),
}))

const mockUseAuth = vi.mocked(useAuth)

describe('AuthButton', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('Loading State', () => {
    it('should display loading button when isLoading is true', () => {
      mockUseAuth.mockReturnValue({
        user: null,
        isAuthenticated: false,
        isLoading: true,
        login: vi.fn(),
        logout: vi.fn(),
        token: null,
      })

      render(<AuthButton />)
      
      const button = screen.getByRole('button', { name: /loading/i })
      expect(button).toBeInTheDocument()
      expect(button).toBeDisabled()
      expect(button).toHaveAttribute('data-size', 'sm')
      expect(button).toHaveAttribute('data-variant', 'outline')
    })
  })

  describe('Unauthenticated State', () => {
    it('should display login button when not authenticated', () => {
      const mockLogin = vi.fn()
      mockUseAuth.mockReturnValue({
        user: null,
        isAuthenticated: false,
        isLoading: false,
        login: mockLogin,
        logout: vi.fn(),
        token: null,
      })

      render(<AuthButton />)
      
      const loginButton = screen.getByRole('button', { name: /login with github/i })
      expect(loginButton).toBeInTheDocument()
      expect(loginButton).not.toBeDisabled()
      expect(loginButton).toHaveAttribute('data-size', 'sm')
    })

    it('should call login function when login button is clicked', async () => {
      const mockLogin = vi.fn()
      mockUseAuth.mockReturnValue({
        user: null,
        isAuthenticated: false,
        isLoading: false,
        login: mockLogin,
        logout: vi.fn(),
        token: null,
      })

      render(<AuthButton />)
      
      const loginButton = screen.getByRole('button', { name: /login with github/i })
      fireEvent.click(loginButton)
      
      expect(mockLogin).toHaveBeenCalledTimes(1)
    })
  })

  describe('Authenticated State', () => {
    const mockUser = {
      id: 'user123',
      email: 'test@example.com',
      name: 'Test User',
      avatar: 'https://github.com/avatar.png',
      githubId: 'github|12345',
      githubUsername: 'testuser',
    }

    it('should display user info and logout button when authenticated', () => {
      const mockLogout = vi.fn()
      mockUseAuth.mockReturnValue({
        user: mockUser,
        isAuthenticated: true,
        isLoading: false,
        login: vi.fn(),
        logout: mockLogout,
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      // Check user info display
      expect(screen.getByText('Test User')).toBeInTheDocument()
      expect(screen.getByRole('img')).toHaveAttribute('src', mockUser.avatar)
      expect(screen.getByRole('img')).toHaveAttribute('alt', 'Test User avatar')
      
      // Check logout button
      const logoutButton = screen.getByRole('button', { name: /logout/i })
      expect(logoutButton).toBeInTheDocument()
      expect(logoutButton).toHaveAttribute('data-size', 'sm')
      expect(logoutButton).toHaveAttribute('data-variant', 'outline')
    })

    it('should display github username when name is not available', () => {
      const userWithoutName = { ...mockUser, name: undefined }
      mockUseAuth.mockReturnValue({
        user: userWithoutName,
        isAuthenticated: true,
        isLoading: false,
        login: vi.fn(),
        logout: vi.fn(),
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      expect(screen.getByText('testuser')).toBeInTheDocument()
    })

    it('should handle missing avatar gracefully', () => {
      const userWithoutAvatar = { ...mockUser, avatar: undefined }
      mockUseAuth.mockReturnValue({
        user: userWithoutAvatar,
        isAuthenticated: true,
        isLoading: false,
        login: vi.fn(),
        logout: vi.fn(),
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      expect(screen.queryByRole('img')).not.toBeInTheDocument()
      expect(screen.getByText('Test User')).toBeInTheDocument()
    })

    it('should call logout function when logout button is clicked', () => {
      const mockLogout = vi.fn()
      mockUseAuth.mockReturnValue({
        user: mockUser,
        isAuthenticated: true,
        isLoading: false,
        login: vi.fn(),
        logout: mockLogout,
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      const logoutButton = screen.getByRole('button', { name: /logout/i })
      fireEvent.click(logoutButton)
      
      expect(mockLogout).toHaveBeenCalledTimes(1)
    })

    it('should hide avatar on error', () => {
      mockUseAuth.mockReturnValue({
        user: mockUser,
        isAuthenticated: true,
        isLoading: false,
        login: vi.fn(),
        logout: vi.fn(),
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      const avatar = screen.getByRole('img')
      
      // Simulate image error
      fireEvent.error(avatar)
      
      expect(avatar.style.display).toBe('none')
    })
  })

  describe('Authentication Edge Cases', () => {
    it('should not display authenticated state when user is null but isAuthenticated is true', () => {
      const mockLogin = vi.fn()
      mockUseAuth.mockReturnValue({
        user: null,
        isAuthenticated: true, // This shouldn't happen but let's test it
        isLoading: false,
        login: mockLogin,
        logout: vi.fn(),
        token: 'mock-token',
      })

      render(<AuthButton />)
      
      // Should fall back to login button
      const loginButton = screen.getByRole('button', { name: /login with github/i })
      expect(loginButton).toBeInTheDocument()
    })

    it('should handle auth context hook error gracefully', () => {
      // Mock AuthContext to throw error
      mockUseAuth.mockImplementation(() => {
        throw new Error('useAuth must be used within an AuthProvider')
      })

      // This should throw during render, so we need to catch it
      expect(() => render(<AuthButton />)).toThrow('useAuth must be used within an AuthProvider')
    })
  })
})