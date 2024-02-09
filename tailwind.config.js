/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    "./resources/views/**/*.blade.php",
  ],
  theme: {
    fontFamily: {
      mono: ['"JetBrains Mono"', 'monospace'],
    },
    colors: {
      black: '#000',
      offblack: '#2C2C2D',
      darkgray: '#3D3D40',
      gray: '#8B8585',
      lightgray: '#A7A7A7',
      white: '#fff',
      transparent: 'transparent',
      bitcoin: '#FF9900',
    }
  },
}
