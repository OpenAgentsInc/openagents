const { fontFamily } = require("tailwindcss/defaultTheme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./templates/**/*.html", "./templates/*.html"],
  theme: {
    extend: {
      fontFamily: {
        mono: ["Berkeley Mono", ...fontFamily.mono],
        sans: ["Inter var", ...fontFamily.sans],
      },
      borderWidth: {
        1: "1px",
      },
      boxShadow: {
        nav: "2px 2px 0 0 rgba(255, 255, 255, 0.5)",
        "nav-hover": "3px 3px 0 0 rgba(255, 255, 255, 0.5)",
        "nav-active": "1px 1px 0 0 rgba(255, 255, 255, 0.5)",
      },
      transitionTimingFunction: {
        nav: "cubic-bezier(0.4, 0, 0.2, 1)",
      },
      transitionDuration: {
        nav: "150ms",
      },
    },
  },
  safelist: [
    "border",
    "border-white", 
    "shadow-nav",
    "animate-nav-in",
    "animate-nav-out",
    "my-2",
    "py-2",
    "py-3",
    "md:py-1",
    "md:py-2",
    "md:py-3"
  ],
};
