import { createContext } from 'react'

import type { FullTheme } from './styles'

type ThemeContextProps = { theme: FullTheme; className: string }

export const ThemeContext = createContext<ThemeContextProps | null>(null)
