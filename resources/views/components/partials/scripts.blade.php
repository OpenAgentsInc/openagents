<script>
    (function() {
        var sidebarCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';
        document.documentElement.classList.add('sidebar-init');
        document.documentElement.style.setProperty('--sidebar-width', sidebarCollapsed ? '70px' : '270px');
        document.documentElement.style.setProperty('--sidebar-content-opacity', sidebarCollapsed ? '0' : '1');
        document.documentElement.style.setProperty('--sidebar-content-visibility', sidebarCollapsed ? 'hidden' : 'visible');
    })();
</script>
<script src="{{ asset('js/tailwind.min.js') }}"></script>
<script src="{{ asset('js/tailwind-config.js') }}"></script>
<script src="{{ asset('js/htmx.min.js') }}"></script>
<script src="{{ asset('js/htmx-sse.js') }}"></script>
<script src="{{ asset('js/alpine.min.js') }}" defer></script>
<script>
    // htmx.logAll();
</script>
@auth
<script src="{{ asset('js/sidebar.js') }}"></script>
@endauth
@production
<script src="https://cdn.usefathom.com/script.js" data-site="COZQPXXY" defer></script>
@endproduction
