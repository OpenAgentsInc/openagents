export interface TokenStore {
  get(): Promise<string | null>
  set(token: string): Promise<void>
  clear(): Promise<void>
}

export function createInMemoryTokenStore(): TokenStore {
  let currentToken: string | null = null

  return {
    async get() {
      return currentToken
    },
    async set(token: string) {
      currentToken = token
    },
    async clear() {
      currentToken = null
    },
  }
}
