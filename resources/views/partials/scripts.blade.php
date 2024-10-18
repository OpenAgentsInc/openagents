<script src="{{ asset('js/tailwind.min.js') }}"></script>
<script src="{{ asset('js/tailwind-config.js') }}"></script>
<script src="{{ asset('js/htmx.min.js') }}"></script>
<script src="{{ asset('js/htmx-sse.js') }}"></script>
<script src="{{ asset('js/alpine.min.js') }}" defer></script>
@auth
<script src="{{ asset('js/sidebar.js') }}"></script>
@endauth
@production
<script src="https://cdn.usefathom.com/script.js" data-site="COZQPXXY" defer></script>
@endproduction
