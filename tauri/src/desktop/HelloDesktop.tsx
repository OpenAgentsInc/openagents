import React from 'react'

// Minimal desktop entrypoint view using our global theme
// Theme CSS (fonts, colors) is imported by main.tsx via App.css

export default function HelloDesktop() {
  return (
    <div
      style={{
        height: '100vh',
        width: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: 'var(--foreground)',
        background: 'var(--background)',
        fontSize: 22,
      }}
    >
      Hello world
    </div>
  )
}

