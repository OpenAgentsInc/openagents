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
            <header class="fixed top-0 left-0 right-0 z-50 flex items-center h-16 px-4 bg-background/80 backdrop-blur-sm">
                <div class="ml-[270px] transition-all duration-300 ease-in-out" id="headerBrand">
                    <x-app-brand />
                </div>
            </header>
            <main class="flex-1 overflow-auto pt-16">
                {{ $slot }}
            </main>
        </div>
    </div>
    @auth
    <script src="{{ asset('js/stream.js') }}"></script>
    @endauth
    @stack('scripts')
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            const sidebar = document.getElementById('sidebar');
            const headerBrand = document.getElementById('headerBrand');

            function updateHeaderPosition() {
                const sidebarWidth = sidebar.offsetWidth;
                headerBrand.style.marginLeft = `${sidebarWidth}px`;
            }

            // Initial position
            updateHeaderPosition();

            // Update on sidebar toggle
            document.addEventListener('sidebar-toggled', function(e) {
                setTimeout(updateHeaderPosition, 300); // Wait for sidebar transition to complete
            });
        });
    </script>
</body>

</html>