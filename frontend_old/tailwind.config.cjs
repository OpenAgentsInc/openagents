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
      typography: {
        DEFAULT: {
          css: {
            maxWidth: "85ch",
            fontSize: "0.75rem",
            lineHeight: "1.5",
            p: {
              fontSize: "0.9rem",
              lineHeight: "1.6",
            },
            li: {
              fontSize: "0.9rem",
              lineHeight: "1.5",
            },
            h1: {
              fontSize: "1rem",
              lineHeight: "1.5",
            },
            h2: {
              fontSize: "1rem",
              lineHeight: "1.5",
            },
            h3: {
              fontSize: "0.75rem",
              lineHeight: "1.5",
            },
            h4: {
              fontSize: "0.75rem",
              lineHeight: "1.5",
            },
          },
        },
      },
    },
  },
  safelist: [
    // Navigation classes
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
    "md:py-3",

    // Text wrapping and overflow classes
    "break-all",
    "break-words",
    "whitespace-pre-wrap",
    "overflow-x-auto",
    "max-w-full",

    // Background and spacing classes
    "bg-black/30",
    "p-2",
    "rounded",
    "space-y-1",
    "space-y-4",

    // Font classes
    "font-mono",
    "text-xs",
    "text-sm",

    // Colors
    "text-gray-300",
    "text-gray-400",
    "text-yellow-400",
    "text-green-400",
    "text-red-400",
    "text-red-300",

    // Background colors
    "bg-gray-800",
    "bg-red-900/20",
    "bg-black/50",

    // Border colors
    "border-red-500/20",
    "border-white/10",

    // Margins and padding
    "mb-2",
    "mt-2",
    "mt-4",
    "p-4",

    // Display and visibility
    "hidden",

    // Transitions
    "transition-all",
    "duration-300",
    "ease-in-out",
  ],
  plugins: [require("@tailwindcss/forms"), require("@tailwindcss/typography")],
};
