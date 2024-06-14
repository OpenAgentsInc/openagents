import {defineConfig} from 'vite';
import laravel, {refreshPaths} from 'laravel-vite-plugin';
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
            laravel({
                input: [
                    'resources/css/app.css',
                    'resources/js/app.js',
                    'resources/js/inertia.js',
                ],
                refresh: [...refreshPaths, 'app/Livewire/**'],
            }),
        ],
        resolve: {
            alias,
        },
    }
});
