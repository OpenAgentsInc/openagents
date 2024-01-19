import forms from "@tailwindcss/forms"

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    // "./resources/views/**/*.blade.php",
    "./resources/views/splash.blade.php",
    "./resources/views/components/theme-switcher.blade.php",
  ],
  plugins: [
    forms,
  ],
}
