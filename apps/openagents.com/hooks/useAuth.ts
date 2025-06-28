'use client'

import { useConvexAuth } from 'convex/react'
import { useAuthActions } from '@convex-dev/auth/react'
import { useRouter } from 'next/navigation'
import { useQuery } from 'convex/react'
import { api } from '@/convex/_generated/api'

export interface User {
  id: string
  login: string
  name: string
  avatar_url: string
  email?: string | null
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
  const { isAuthenticated, isLoading: authLoading } = useConvexAuth()
  const { signIn: convexSignIn, signOut: convexSignOut } = useAuthActions()
  const router = useRouter()
  
  // Fetch real user data from Convex
  const userData = useQuery(
    api.users.getCurrentUser,
    isAuthenticated ? {} : "skip"
  )
  
  const user = userData || null
  const isLoading = authLoading || (isAuthenticated && userData === undefined)

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