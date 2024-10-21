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
    @include('partials.mobile-message')
    <div class="hidden sm:flex h-full">
        @auth <x-sidebar.sidebar /> @endauth
        <main class="flex-1 overflow-auto">
            {{ $slot }}
        </main>
    </div>
    @auth
    <script src="{{ asset('js/stream.js') }}"></script>
    @endauth
    @stack('scripts')
</body>

</html>
