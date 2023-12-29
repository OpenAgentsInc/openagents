import { createContext, useContext } from 'react'

import type { FullTheme } from './styles'
import type { PanelSettingsType, InputContextProps } from './types'

export const InputContext = createContext({})

export function useInputContext<T = {}>() {
  return useContext(InputContext) as InputContextProps & T
}

type ThemeContextProps = { theme: FullTheme; className: string }

export const ThemeContext = createContext<ThemeContextProps | null>(null)

export const PanelSettingsContext = createContext<PanelSettingsType | null>(null)

export function usePanelSettingsContext() {
  return useContext(PanelSettingsContext)!
}
