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
            boxShadow: {
              'white-glow': '20px 20px 20px rgba(255, 255, 255, 0.8)',
            },
            fontFamily: {
                mono: ['"JetBrains Mono"', ...defaultTheme.fontFamily.mono],
                berkeley: ['"Berkeley Mono"', ...defaultTheme.fontFamily.mono]
            },
            colors: {
                text: '#D7D8E5',
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
                        img: {
                            borderWidth: 0,
                        },
                        // Customizing paragraph spacing
                        p: {
                            // marginTop: '2.2em', // Increase top margin
                            // marginBottom: '2.2em', // Increase bottom margin
                            // color: 'white'
                        },
                        // You can also customize other elements here as needed
                    },
                },
            }),
        },
    },

    plugins: [forms, typography],
};
