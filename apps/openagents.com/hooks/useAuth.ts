'use client'

import { useConvexAuth } from 'convex/react'
import { useAuthActions } from '@convex-dev/auth/react'
import { useRouter } from 'next/navigation'

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
  const { isAuthenticated, isLoading } = useConvexAuth()
  const { signIn: convexSignIn, signOut: convexSignOut } = useAuthActions()
  const router = useRouter()

  // Create a mock user object for now since Convex auth doesn't provide GitHub-style user data
  // In a real implementation, you'd fetch this from a users table
  const user: User | null = isAuthenticated ? {
    id: 'convex-user',
    login: 'user',
    name: 'OpenAgents User',
    avatar_url: 'https://github.com/identicons/user.png',
    email: 'user@openagents.com'
  } : null

  const signIn = async () => {
    // Redirect to the sign-in page
    // The actual Convex auth is handled on that page
    router.push('/signin')
  }

  const signOut = () => {
    convexSignOut()
  }

  return {
    isAuthenticated,
    user,
    isLoading,
    signIn,
    signOut
  }
}