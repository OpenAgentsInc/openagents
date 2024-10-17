<!DOCTYPE html>
<html lang="en" class="dark">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <link rel="stylesheet" href="{{ asset('css/jbm.css') }}">
    <link rel="stylesheet" href="{{ asset('css/variables.css') }}">
    <script src="{{ asset('js/tailwind.min.js') }}"></script>
    <script src="{{ asset('js/tailwind-config.js') }}"></script>
</head>

<body class="bg-background text-foreground font-mono h-screen overflow-hidden antialiased">
    <div class="fixed top-4 left-4 z-50">
        <x-logo />
    </div>
    <main class="h-full overflow-auto">
        {{ $slot }}
    </main>
</body>

</html>
