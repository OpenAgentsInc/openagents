<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">

    <title>{{ config('app.name', 'OpenAgents') }}</title>

    @include('partials.vite')
    @include('analytics')
</head>

<body class="bg-black text-white font-mono antialiased">
    <div class="min-h-screen">
        @include('layouts.navigation')

        @if(isset($header))
            <header class="bg-black text-white">
                <div class="max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8 flex justify-center">
                    {{ $header }}
                </div>
            </header>
        @endif

        <main>
            {{ $slot }}
        </main>
    </div>
</body>

</html>
