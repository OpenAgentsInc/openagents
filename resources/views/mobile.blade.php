<!DOCTYPE html>
<html lang="en" class="dark">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" href="{{ asset('favicon.ico') }}" type="image/x-icon">
    <title>{{ $title ?? 'OpenAgents Mobile' }}</title>
    <link rel="stylesheet" href="{{ asset('css/jbm.css') }}">
    <link rel="stylesheet" href="{{ asset('css/variables.css') }}">
    @include('partials.scripts')
</head>

<body class="bg-background text-foreground font-mono min-h-screen antialiased">
    <div class="p-4">
        <main>
            {{ $slot }}
        </main>
    </div>
    <script src="{{ asset('js/stream.js') }}"></script>
    @stack('scripts')
</body>

</html>
