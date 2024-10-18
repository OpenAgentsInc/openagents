<!DOCTYPE html>
<html lang="en" class="dark">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" href="{{ asset('favicon.ico') }}" type="image/x-icon">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <link rel="stylesheet" href="{{ asset('css/jbm.css') }}">
    <link rel="stylesheet" href="{{ asset('css/variables.css') }}">
    <script src="{{ asset('js/tailwind.min.js') }}"></script>
    <script src="{{ asset('js/tailwind-config.js') }}"></script>
    <script src="https://unpkg.com/htmx.org@2.0.3"></script>
    <script src="https://unpkg.com/htmx-ext-sse@2.2.2/sse.js"></script>
    @stack('scripts')
</head>

<body class="bg-background text-foreground font-mono h-screen overflow-hidden antialiased">
    <div class="flex h-full">
        <x-sidebar />
        <main class="flex-1 overflow-auto">
            {{ $slot }}
        </main>
    </div>
    <script src="{{ asset('js/stream.js') }}"></script>
    <script src="{{ asset('js/sidebar.js') }}"></script>
</body>

</html>