<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
    <x-social-tags/>
</head>

<body class="h-screen bg-black antialiased">

<div class="relative z-0 flex h-full w-full overflow-hidden min-h-screen">
    <main class="relative h-full w-full flex-1 overflow-auto transition-width">
        {{$slot}}
    </main>
</div>

</body>

@include('partials.twitter')

</html>
