import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { AuthButton } from './AuthButton'

// Mock the AuthContext with simple return values
vi.mock('@/contexts/AuthContext', () => ({
  useAuth: vi.fn(),
}))

// Mock the UI Button component
vi.mock('@/components/ui/button', () => ({
  Button: ({ children, ...props }: any) => (
    <button {...props}>{children}</button>
  ),
}))

import { useAuth } from '@/contexts/AuthContext'
const mockUseAuth = vi.mocked(useAuth)

describe('AuthButton - Simple Tests', () => {
  it('should render loading state', () => {
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: true,
      login: vi.fn(),
      logout: vi.fn(),
      token: null,
    })

    const { container } = render(<AuthButton />)
    console.log('Loading state DOM:', container.innerHTML)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('should render login button when not authenticated', () => {
    mockUseAuth.mockReturnValue({
      user: null,
      isAuthenticated: false,
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
      token: null,
    })

    render(<AuthButton />)
    expect(screen.getByText('Login with GitHub')).toBeInTheDocument()
  })

  it('should render user info when authenticated', () => {
    const mockUser = {
      id: 'user123',
      email: 'test@example.com',
      name: 'Test User',
      githubId: 'github|12345',
      githubUsername: 'testuser',
    }

    mockUseAuth.mockReturnValue({
      user: mockUser,
      isAuthenticated: true,
      isLoading: false,
      login: vi.fn(),
      logout: vi.fn(),
      token: 'mock-token',
    })

    render(<AuthButton />)
    expect(screen.getByText('Test User')).toBeInTheDocument()
    expect(screen.getByText('Logout')).toBeInTheDocument()
  })
})