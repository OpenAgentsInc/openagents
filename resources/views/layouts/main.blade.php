<!DOCTYPE html>
<html lang="en" class="dark">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>@yield('title', 'OpenAgents')</title>
    @include('partials.vite')
    @include('analytics')
</head>

<body class="flex flex-col min-h-screen bg-black text-white font-mono antialiased">
    <x-header />

    <div class="flex flex-grow items-center justify-center">
        <div class="w-full max-w-5xl mx-auto p-6">
            @yield('content')
        </div>
    </div>

    <x-footer />

    @stack('scripts')
</body>

</html>
