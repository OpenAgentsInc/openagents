const { fontFamily } = require("tailwindcss/defaultTheme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./templates/*.html"],
  theme: {
    extend: {
      fontFamily: {
        mono: ["Berkeley Mono", ...fontFamily.mono],
        sans: ["Inter var", ...fontFamily.sans],
      },
    },
  },
};
