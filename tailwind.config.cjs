const { fontFamily } = require("tailwindcss/defaultTheme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.html",
    "./templates/*.html"
  ],
  theme: {
    extend: {
      fontFamily: {
        mono: ["Berkeley Mono", ...fontFamily.mono],
        sans: ["Inter var", ...fontFamily.sans],
      },
      borderWidth: {
        '1': '1px',
      },
      boxShadow: {
        'nav': '4px 4px 0 0 rgba(255, 255, 255, 0.75)',
      },
    },
  },
  safelist: [
    'border',
    'border-white',
    'shadow-nav'
  ],
};
