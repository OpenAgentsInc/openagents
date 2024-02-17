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

<body>
    <div class="flex flex-col h-full">
        <livewire:navbar />
        <main class="flex-1 overflow-hidden mt-12">
            {{ $slot }}
        </main>
    </div>
</body>

</html>
