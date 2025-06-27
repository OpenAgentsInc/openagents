'use client'

import { useState, useEffect } from 'react'

export interface User {
  id: string
  login: string
  name: string
  avatar_url: string
  email?: string
}

export interface AuthState {
  isAuthenticated: boolean
  user: User | null
  isLoading: boolean
}

export function useAuth(): AuthState & {
  signIn: () => Promise<void>
  signOut: () => void
} {
  const [authState, setAuthState] = useState<AuthState>({
    isAuthenticated: false,
    user: null,
    isLoading: true
  })

  useEffect(() => {
    // Check for existing auth on mount
    checkAuthStatus()
  }, [])

  const checkAuthStatus = async () => {
    try {
      // For now, check localStorage for demo purposes
      // Later this will check actual OAuth tokens
      const user = localStorage.getItem('openagents_user')
      const isAuthenticated = !!user

      setAuthState({
        isAuthenticated,
        user: user ? JSON.parse(user) : null,
        isLoading: false
      })
    } catch (error) {
      console.error('Auth check failed:', error)
      setAuthState({
        isAuthenticated: false,
        user: null,
        isLoading: false
      })
    }
  }

  const signIn = async () => {
    try {
      // For demo purposes, simulate OAuth flow
      // Later this will redirect to GitHub OAuth
      console.log('Initiating GitHub OAuth flow...')
      
      // Simulate OAuth success with mock user
      const mockUser: User = {
        id: 'demo-user-123',
        login: 'demo-user',
        name: 'Demo User',
        avatar_url: 'https://github.com/identicons/demo-user.png'
      }

      // Store auth state
      localStorage.setItem('openagents_user', JSON.stringify(mockUser))
      
      setAuthState({
        isAuthenticated: true,
        user: mockUser,
        isLoading: false
      })

      console.log('Authentication successful!')
    } catch (error) {
      console.error('Authentication failed:', error)
    }
  }

  const signOut = () => {
    localStorage.removeItem('openagents_user')
    setAuthState({
      isAuthenticated: false,
      user: null,
      isLoading: false
    })
    console.log('Signed out successfully')
  }

  return {
    ...authState,
    signIn,
    signOut
  }
}