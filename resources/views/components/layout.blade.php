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
            <header class="flex justify-between items-center bg-black border-b border-border md:border-none p-0 sm:p-3 z-[9999]">
                <div class="flex flex-row items-center gap-x-4 px-3 py-1">
                    <x-app-brand />
                    <x-upgrade-button />
                </div>
                <div class="">
                    <!-- Add any additional header content here -->
                </div>
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
