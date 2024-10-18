<!DOCTYPE html>
<html lang="en" class="dark">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" href="{{ asset('favicon.ico') }}" type="image/x-icon">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <link rel="stylesheet" href="{{ asset('css/globals.css') }}">
    <link rel="stylesheet" href="{{ asset('css/jbm.css') }}">
    <link rel="stylesheet" href="{{ asset('css/variables.css') }}">
    @include('partials.scripts')
</head>

<body class="bg-background text-foreground font-mono h-screen overflow-hidden antialiased">
    <div class="flex h-full">
        @auth <x-sidebar /> @endauth
        <div class="flex-1 flex flex-col">
            <header class="flex items-center h-16 px-4 border-b border-border">
                <x-app-brand />
            </header>
            <main class="flex-1 overflow-auto">
                {{ $slot }}
            </main>
        </div>
    </div>
    @auth
    <script src="{{ asset('js/stream.js') }}"></script>
    @endauth
    @stack('scripts')
</body>

</html>