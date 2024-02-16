<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
    <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <title>{{ $title ?? 'Page Title' }}</title>
        @stack('scripts')
        @include('partials.vite')
        @include('analytics')
    </head>
    <body class="bg-black text-white h-screen font-mono">
        <div class="flex flex-col h-full">
            @include('layouts.navigation')
            <main class="flex-1 overflow-hidden">
                {{ $slot }}
            </main>
        </div>
    </body>
</html>
