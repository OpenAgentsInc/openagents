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
<<<<<<< HEAD
    <script src="https://unpkg.com/htmx.org@1.9.2"></script>
    <script src="https://unpkg.com/htmx.org/dist/ext/sse.js"></script>
=======
    <script src="https://unpkg.com/htmx.org@2.0.3"></script>
    <script src="https://unpkg.com/htmx-ext-sse@2.2.2/sse.js"></script>
>>>>>>> 7caa157b (htmx sse scripts)
</head>

<body class="bg-background text-foreground font-mono h-screen overflow-hidden antialiased">
    <!--
    <div class="fixed top-4 left-4 z-50">
        <x-logo />
    </div>
    -->
    <main class="h-full overflow-auto">
        {{ $slot }}
    </main>
</body>

</html>