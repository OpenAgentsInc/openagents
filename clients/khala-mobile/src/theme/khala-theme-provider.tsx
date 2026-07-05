import { createContext, useContext, type ReactNode } from "react"

import { khalaMobileTheme, type KhalaMobileTheme } from "./tokens"

const KhalaThemeContext = createContext<KhalaMobileTheme>(khalaMobileTheme)

export const KhalaThemeProvider = ({ children }: { children: ReactNode }) => (
  <KhalaThemeContext.Provider value={khalaMobileTheme}>
    {children}
  </KhalaThemeContext.Provider>
)

export const useKhalaTheme = () => useContext(KhalaThemeContext)
