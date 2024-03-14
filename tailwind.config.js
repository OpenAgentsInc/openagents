import defaultTheme from 'tailwindcss/defaultTheme';
import forms from '@tailwindcss/forms';
import typography from '@tailwindcss/typography';

/** @type {import('tailwindcss').Config} */
export default {
    darkMode: 'class',
    content: [
        './vendor/laravel/framework/src/Illuminate/Pagination/resources/views/*.blade.php',
        './vendor/laravel/jetstream/**/*.blade.php',
        './storage/framework/views/*.php',
        './resources/views/**/*.blade.php',
    ],

    theme: {
        extend: {
            fontFamily: {
                mono: ['"JetBrains Mono"', ...defaultTheme.fontFamily.mono],
            },
            colors: {
                offblack: '#1e1e1e',
                darkgray: '#3D3D40',
                gray: '#8B8585',
                lightgray: '#A7A7A7',
                white: '#fff',
                bitcoin: '#FF9900',
                green: '#00CC00',
                red: '#FF3B00'
            },
            typography: (theme) => ({
                DEFAULT: {
                    css: {
                        // Customizing paragraph spacing
                        p: {
                            marginTop: '2.2em', // Increase top margin
                            marginBottom: '2.2em', // Increase bottom margin
                        },
                        // You can also customize other elements here as needed
                    },
                },
            }),
        },
    },

    plugins: [forms, typography],
};
