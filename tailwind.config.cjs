const { fontFamily } = require("tailwindcss/defaultTheme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./templates/*.html"],
  theme: {
    extend: {
      fontFamily: {
        mono: ["JetBrains Mono", ...fontFamily.mono],
        sans: ["Inter var", ...fontFamily.sans],
      },
    },
  },
};
