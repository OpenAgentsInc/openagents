<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <script defer src="https://unpkg.com/alpinejs@3/dist/cdn.min.js"></script>
    <script src="https://unpkg.com/htmx.org@2.0.0-beta4/dist/htmx.min.js"></script>
    <script src="https://unpkg.com/htmx-ext-sse@2.0.0/sse.js"></script>
    @include('partials.vite')
</head>

<body>
Sup
</body>

</html>