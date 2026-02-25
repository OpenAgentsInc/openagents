<?php

namespace Laravel\Roster\Enums;

enum Packages: string
{
    // Compound
    case INERTIA = 'inertia';
    case WAYFINDER = 'wayfinder';

    // BACKEND
    case BOOST = 'boost';
    case BREEZE = 'breeze';
    case CASHIER = 'cashier';
    case DUSK = 'dusk';
    case ENVOY = 'envoy';
    case FILAMENT = 'filament';
    case FOLIO = 'folio';
    case FORTIFY = 'fortify';
    case FLUXUI_FREE = 'flux_free';
    case FLUXUI_PRO = 'flux_pro';
    case HORIZON = 'horizon';
    case INERTIA_LARAVEL = 'inertia-laravel';
    case LARASTAN = 'larastan';
    case LARAVEL = 'laravel';
    case LIVEWIRE = 'livewire';
    case MCP = 'mcp';
    case NIGHTWATCH = 'nightwatch';
    case NOVA = 'nova';
    case OCTANE = 'octane';
    case PAIL = 'pail';
    case PASSPORT = 'passport';
    case PENNANT = 'pennant';
    case PEST = 'pest';
    case PHPUNIT = 'phpunit';
    case PINT = 'pint';
    case PROMPTS = 'prompts';
    case PULSE = 'pulse';
    case RECTOR = 'rector';
    case REVERB = 'reverb';
    case SAIL = 'sail';
    case SANCTUM = 'sanctum';
    case SCOUT = 'scout';
    case SOCIALITE = 'socialite';
    case STATAMIC = 'statamic';
    case TELESCOPE = 'telescope';
    case VOLT = 'volt';
    case WAYFINDER_LARAVEL = 'wayfinder_laravel';
    case ZIGGY = 'ziggy';

    // NPM
    case ALPINEJS = 'alpinejs';
    case ECHO = 'laravel-echo';
    case ESLINT = 'eslint';
    case INERTIA_REACT = 'inertia-react';
    case INERTIA_SVELTE = 'inertia-svelte';
    case INERTIA_VUE = 'inertia-vue';
    case PRETTIER = 'prettier';
    case REACT = 'react';
    case TAILWINDCSS = 'tailwindcss';
    case VUE = 'vue';
    case WAYFINDER_VITE = 'wayfinder_vite';
}
