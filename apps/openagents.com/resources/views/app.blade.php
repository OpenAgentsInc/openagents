<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}" class="dark">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">

    {{-- Dark mode only: inline style for HTML background (matches app.css .dark) --}}
    <style>
        html {
            background-color: oklch(0.145 0 0);
        }
    </style>

    <title inertia>{{ config('app.name', 'Laravel') }}</title>

    <link rel="icon" href="/favicon.ico" sizes="any">

    <script>
        window.__OA_POSTHOG__ = {
            key: @json((string) config('posthog.api_key', '')),
            host: @json((string) config('posthog.host', 'https://us.i.posthog.com')),
            disabled: @json((bool) config('posthog.disabled', false)),
        };
    </script>

    <link rel="preconnect" href="https://fonts.bunny.net">
    <link href="https://fonts.bunny.net/css?family=inter:400,500,600,700" rel="stylesheet" />

    @viteReactRefresh
    @vite(['resources/js/app.tsx'])
    @inertiaHead
</head>

<body class="font-sans antialiased">
    @inertia
</body>

</html>
