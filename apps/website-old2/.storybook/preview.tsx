import type { Preview } from '@storybook/react-vite'
import React, { useLayoutEffect, useState, useCallback, useEffect } from 'react'
import { createPortal } from 'react-dom'
import '../src/styles.css'

type Theme = 'light' | 'dark'
const DEFAULT_THEME: Theme = 'dark'
const STORAGE_KEY = 'sb-openagents-theme'

function getStoredTheme(): Theme {
  if (typeof window === 'undefined') return DEFAULT_THEME
  const stored = window.localStorage.getItem(STORAGE_KEY)
  return stored === 'light' || stored === 'dark' ? stored : DEFAULT_THEME
}

function applyPreviewTheme(theme: Theme) {
  const html = document.documentElement
  html.setAttribute('data-theme', theme)
  html.classList.toggle('dark', theme === 'dark')
  html.style.colorScheme = theme
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(STORAGE_KEY, theme)
  }
}

function ThemeWrapper({
  theme,
  children,
}: {
  theme: Theme
  children: React.ReactNode
}) {
  useLayoutEffect(() => {
    applyPreviewTheme(theme)
  }, [theme])
  return <>{children}</>
}

const toggleButtonStyle: React.CSSProperties = {
  position: 'fixed',
  top: 8,
  right: 8,
  zIndex: 9999,
  width: 36,
  height: 36,
  borderRadius: 8,
  border: '1px solid oklch(0.7 0 0)',
  background: 'oklch(0.98 0 0)',
  cursor: 'pointer',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  boxShadow: '0 1px 3px oklch(0 0 0 / 0.12)',
}
const toggleButtonStyleDark: React.CSSProperties = {
  ...toggleButtonStyle,
  border: '1px solid oklch(0.35 0 0)',
  background: 'oklch(0.2 0 0)',
  boxShadow: '0 1px 3px oklch(0 0 0 / 0.4)',
}

function ThemeToggleButton({
  theme,
  onToggle,
}: {
  theme: Theme
  onToggle: () => void
}) {
  const isDark = theme === 'dark'
  return (
    <button
      type="button"
      title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      onClick={onToggle}
      style={isDark ? toggleButtonStyleDark : toggleButtonStyle}
    >
      {isDark ? (
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="18"
          height="18"
          fill="currentColor"
          viewBox="0 0 24 24"
          aria-hidden
        >
          <path
            fillRule="evenodd"
            d="M12 1a1 1 0 0 1 1 1v1a1 1 0 1 1-2 0V2a1 1 0 0 1 1-1ZM1 12a1 1 0 0 1 1-1h1a1 1 0 1 1 0 2H2a1 1 0 0 1-1-1Zm19 0a1 1 0 0 1 1-1h1a1 1 0 1 1 0 2h-1a1 1 0 0 1-1-1Zm-8 8a1 1 0 0 1 1 1v1a1 1 0 1 1-2 0v-1a1 1 0 0 1 1-1Zm0-12a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm-6 4a6 6 0 1 1 12 0 6 6 0 0 1-12 0Z"
            clipRule="evenodd"
          />
        </svg>
      ) : (
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="18"
          height="18"
          fill="currentColor"
          viewBox="0 0 24 24"
          aria-hidden
        >
          <path
            fillRule="evenodd"
            d="M12.784 2.47a1 1 0 0 1 .047.975A8 8 0 0 0 20 15h.057a1 1 0 0 1 .902 1.445A10 10 0 0 1 12 22C6.477 22 2 17.523 2 12c0-5.499 4.438-9.961 9.928-10a1 1 0 0 1 .856.47Z"
            clipRule="evenodd"
          />
        </svg>
      )}
    </button>
  )
}

const WithTheme: Preview['decorators'][0] = (Story, context) => {
  const globalsTheme = context.globals?.theme as Theme | undefined
  const [theme, setTheme] = useState<Theme>(() =>
    globalsTheme && (globalsTheme === 'light' || globalsTheme === 'dark')
      ? globalsTheme
      : getStoredTheme()
  )

  useEffect(() => {
    if (globalsTheme && (globalsTheme === 'light' || globalsTheme === 'dark') && globalsTheme !== theme) {
      setTheme(globalsTheme)
    }
  }, [globalsTheme])

  const onToggle = useCallback(() => {
    setTheme((prev) => {
      const next = prev === 'light' ? 'dark' : 'light'
      applyPreviewTheme(next)
      return next
    })
  }, [])

  return (
    <ThemeWrapper theme={theme}>
      {typeof document !== 'undefined' &&
        createPortal(
          <ThemeToggleButton theme={theme} onToggle={onToggle} />,
          document.body
        )}
      <Story />
    </ThemeWrapper>
  )
}

const preview: Preview = {
  globalTypes: {
    theme: {
      name: 'Theme',
      description: 'Global theme for components',
      defaultValue: DEFAULT_THEME,
      toolbar: {
        icon: 'circlehollow',
        items: [
          { value: 'light', title: 'Light' },
          { value: 'dark', title: 'Dark' },
        ],
        dynamicTitle: true,
      },
    },
  },
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
  decorators: [WithTheme],
  initialGlobals: {
    theme: DEFAULT_THEME,
  },
}

export default preview
