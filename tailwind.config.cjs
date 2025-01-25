/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.{html,js}",
    "./static/**/*.{html,js}",
  ],
  theme: {
    extend: {
      typography: {
        DEFAULT: {
          css: {
            fontSize: '0.75rem', // text-xs size
            p: {
              fontSize: '0.75rem',
            },
            li: {
              fontSize: '0.75rem',
            },
            h1: {
              fontSize: '1.25rem', // Proportionally reduced
            },
            h2: {
              fontSize: '1.125rem',
            },
            h3: {
              fontSize: '1rem',
            },
            h4: {
              fontSize: '0.875rem',
            },
          },
        },
      },
    },
  },
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/typography'),
  ],
}