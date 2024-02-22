<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">

    <title>{{ $title ?? 'OpenAgents' }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('analytics')
</head>

<body class="antialiased">
    {{ $slot }}
</body>

</html>
