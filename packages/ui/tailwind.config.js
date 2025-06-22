/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./src/**/*.{html,js,ts,jsx,tsx,css}",
    "./examples/**/*.html",
    "../../apps/*/src/**/*.{html,js,ts,jsx,tsx}",
    "./node_modules/basecoat-css/dist/**/*.css"
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // OpenAgents custom colors
        'oa-black': '#000000',
        'oa-offblack': '#0a0a0a',
        'oa-darkgray': '#333333',
        'oa-gray': '#666666',
        'oa-lightgray': '#999999',
        'oa-white': '#ffffff',
        'oa-text': '#e5e5e5',
        
        // Map to Tailwind's color system for Basecoat compatibility
        background: '#000000',
        foreground: '#e5e5e5',
        card: {
          DEFAULT: '#0a0a0a',
          foreground: '#e5e5e5',
        },
        popover: {
          DEFAULT: '#0a0a0a',
          foreground: '#e5e5e5',
        },
        primary: {
          DEFAULT: '#3b82f6', // blue-500
          foreground: '#ffffff',
        },
        secondary: {
          DEFAULT: '#666666',
          foreground: '#e5e5e5',
        },
        muted: {
          DEFAULT: '#333333',
          foreground: '#999999',
        },
        accent: {
          DEFAULT: '#333333',
          foreground: '#e5e5e5',
        },
        destructive: {
          DEFAULT: '#ef4444', // red-500
          foreground: '#ffffff',
        },
        border: '#333333',
        input: '#333333',
        ring: '#3b82f6',
      },
      borderRadius: {
        lg: '0.5rem',
        md: '0.375rem',
        sm: '0.25rem',
      },
      ringColor: {
        'destructive/20': 'rgb(239 68 68 / 0.2)',
      },
    },
  },
  plugins: [],
}