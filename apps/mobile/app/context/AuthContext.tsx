import { createContext, FC, PropsWithChildren, useCallback, useContext, useMemo } from "react"
import { useMMKVString } from "react-native-mmkv"

export type AuthUser = {
  id: string
  email: string | null
  firstName: string | null
  lastName: string | null
}

export type AuthContextType = {
  isAuthenticated: boolean
  authToken?: string
  authEmail?: string
  authUserId?: string
  authUser?: AuthUser | null
  setAuthToken: (token?: string) => void
  setAuthEmail: (email: string) => void
  /** Set session after WorkOS magic or SSO login (token optional for magic). */
  setSession: (opts: {
    userId: string
    email?: string
    token?: string
    user?: AuthUser | null
  }) => void
  logout: () => void
  validationError: string
}

export const AuthContext = createContext<AuthContextType | null>(null)

const AUTH_USER_KEY = "AuthProvider.authUser"

export interface AuthProviderProps {}

export const AuthProvider: FC<PropsWithChildren<AuthProviderProps>> = ({ children }) => {
  console.log("[AuthProvider] mount")
  const [authToken, setAuthToken] = useMMKVString("AuthProvider.authToken")
  const [authEmail, setAuthEmail] = useMMKVString("AuthProvider.authEmail")
  const [authUserId, setAuthUserId] = useMMKVString("AuthProvider.authUserId")
  const [authUserJson, setAuthUserJson] = useMMKVString(AUTH_USER_KEY)

  const authUser: AuthUser | null = useMemo(() => {
    if (!authUserJson) return null
    try {
      return JSON.parse(authUserJson) as AuthUser
    } catch {
      return null
    }
  }, [authUserJson])

  const setSession = useCallback(
    (opts: { userId: string; email?: string; token?: string; user?: AuthUser | null }) => {
      setAuthUserId(opts.userId)
      if (opts.email !== undefined) setAuthEmail(opts.email)
      if (opts.token !== undefined) setAuthToken(opts.token)
      setAuthUserJson(opts.user ? JSON.stringify(opts.user) : undefined)
    },
    [setAuthEmail, setAuthToken, setAuthUserId, setAuthUserJson],
  )

  const logout = useCallback(() => {
    setAuthToken(undefined)
    setAuthEmail("")
    setAuthUserId(undefined)
    setAuthUserJson(undefined)
  }, [setAuthEmail, setAuthToken, setAuthUserId, setAuthUserJson])

  const validationError = useMemo(() => {
    if (!authEmail || authEmail.length === 0) return "can't be blank"
    if (authEmail.length < 6) return "must be at least 6 characters"
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(authEmail)) return "must be a valid email address"
    return ""
  }, [authEmail])

  const value = {
    isAuthenticated: !!(authToken || authUserId),
    authToken,
    authEmail,
    authUserId: authUserId ?? undefined,
    authUser: authUser ?? undefined,
    setAuthToken,
    setAuthEmail,
    setSession,
    logout,
    validationError,
  }

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export const useAuth = () => {
  const context = useContext(AuthContext)
  if (!context) throw new Error("useAuth must be used within an AuthProvider")
  return context
}
