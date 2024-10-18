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
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            const sidebar = document.getElementById('sidebar');
            const sidebarToggle = document.getElementById('sidebarToggle');
            const sidebarContent = document.getElementById('sidebarContent');

            // Get the initial state from localStorage
            let sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';

            function updateSidebarState(immediate = false) {
                if (sidebarCollapsed) {
                    sidebar.style.width = '70px';
                    sidebarContent.style.opacity = '0';
                    if (immediate) {
                        sidebarContent.style.visibility = 'hidden';
                    } else {
                        setTimeout(() => {
                            sidebarContent.style.visibility = 'hidden';
                        }, 200); // Match this with the transition duration
                    }
                } else {
                    sidebar.style.width = '270px';
                    sidebarContent.style.visibility = 'visible';
                    setTimeout(() => {
                        sidebarContent.style.opacity = '1';
                    }, 50); // Small delay to ensure display change has taken effect
                }
            }

            // Set initial state
            updateSidebarState(true);

            sidebarToggle.addEventListener('click', function() {
                sidebarCollapsed = !sidebarCollapsed;
                localStorage.setItem('sidebarCollapsed', sidebarCollapsed);
                updateSidebarState();
            });
        });
    </script>
</body>

</html>