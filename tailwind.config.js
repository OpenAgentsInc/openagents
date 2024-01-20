import forms from "@tailwindcss/forms"

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    "./resources/views/**/*.blade.php",
  ],
  theme: {
    extend: {
      colors: {
        haiti: '#120B29',
        purple: '#1C133A',
        portgore: '#2D2252',
        bluebell: '#9D98CB',
        bluebellfaded: 'rgba(157, 152, 203, 0.6)',
        minsk: '#46367C',
        moonraker: '#EEECFB',
        radicalRed: '#FC3A57',
        pinkflamingo: '#F459F4',
        electricviolet: '#AE30FF',
        electricindigo: '#5B20F2',
      }
    },
  },
  plugins: [forms],
}
