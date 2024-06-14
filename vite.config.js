import {defineConfig} from 'vite';
import laravel, {refreshPaths} from 'laravel-vite-plugin';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig(({command}) => {
    let alias = {}

    // dev environment
    if (command === 'serve') {
        Object.assign(alias, {
            '/fonts': path.resolve(__dirname, 'public/fonts'),
        });
    }

    return {
        plugins: [
            react(),
            laravel({
                input: [
                    'resources/css/app.css',
                    'resources/js/app.js',
                    'resources/js/inertia.jsx',
                ],
                refresh: [...refreshPaths, 'app/Livewire/**'],
            }),
        ],
        resolve: {
            alias,
        },
    }
});
