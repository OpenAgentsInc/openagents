import forms from "@tailwindcss/forms"
import typography from "@tailwindcss/typography"

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    "./resources/views/**/*.blade.php",
  ],
  theme: {
    colors: {
      black: '#000',
      white: '#fff',
      transparent: 'transparent',
      bitcoin: '#FF9900',
      // Primary
      // 'teal': {
      //   '50': '#F0FCF9',
      //   '100': '#C6F7E9',
      //   '200': '#8EEDD1',
      //   '300': '#5FE3C0',
      //   '400': '#2DCCA7',
      //   '500': '#17B897',
      //   '600': '#079A82',
      //   '700': '#048271',
      //   '800': '#016457',
      //   '900': '#004440',
      // },
      // Neutrals
      'grey': {
        '50': '#F7F7F7',
        '100': '#E1E1E1',
        '200': '#CFCFCF',
        '300': '#B1B1B1',
        '400': '#9E9E9E',
        '500': '#7E7E7E',
        '600': '#626262',
        '700': '#515151',
        '800': '#3B3B3B',
        '900': '#222222',
      },
      // Supporting
      // 'yellow': {
      //   '50': '#FFFBEA',
      //   '100': '#FFF3C4',
      //   '200': '#FCE588',
      //   '300': '#FADB5F',
      //   '400': '#F7C948',
      //   '500': '#F0B429',
      //   '600': '#DE911D',
      //   '700': '#CB6E17',
      //   '800': '#B44D12',
      //   '900': '#8D2B0B',
      // },
      // 'red': {
      //   '50': '#FFE3E3',
      //   '100': '#FFBDBD',
      //   '200': '#FF9B9B',
      //   '300': '#F86A6A',
      //   '400': '#EF4E4E',
      //   '500': '#E12D39',
      //   '600': '#CF1124',
      //   '700': '#AB091E',
      //   '800': '#8A041A',
      //   '900': '#610316',
      // },
      // shad
      border: "hsl(var(--border))",
      input: "hsl(var(--input))",
      ring: "hsl(var(--ring))",
      background: "hsl(var(--background))",
      foreground: "hsl(var(--foreground))",
      primary: {
        DEFAULT: "hsl(var(--primary))",
        foreground: "hsl(var(--primary-foreground))",
      },
      secondary: {
        DEFAULT: "hsl(var(--secondary))",
        foreground: "hsl(var(--secondary-foreground))",
      },
      destructive: {
        DEFAULT: "hsl(var(--destructive))",
        foreground: "hsl(var(--destructive-foreground))",
      },
      muted: {
        DEFAULT: "hsl(var(--muted))",
        foreground: "hsl(var(--muted-foreground))",
      },
      accent: {
        DEFAULT: "hsl(var(--accent))",
        foreground: "hsl(var(--accent-foreground))",
      },
      popover: {
        DEFAULT: "hsl(var(--popover))",
        foreground: "hsl(var(--popover-foreground))",
      },
      card: {
        DEFAULT: "hsl(var(--card))",
        foreground: "hsl(var(--card-foreground))",
      },
    }
  },
  plugins: [forms, typography],
}
