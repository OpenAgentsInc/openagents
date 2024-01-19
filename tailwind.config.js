import forms from "@tailwindcss/forms"

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: [
    // "./resources/views/**/*.blade.php",
    "./resources/views/**/*.blade.php",
  ],
  plugins: [
    forms,
  ],
}
