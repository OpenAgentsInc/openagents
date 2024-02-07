<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>@yield('title', 'OpenAgents')</title>
    </script>
    @include('partials.vite')
    @include('analytics')
</head>

<body class="flex flex-col min-h-screen bg-white dark:bg-black text-black dark:text-white font-mono">
    <x-theme-switcher />
    <div class="flex flex-grow items-center justify-center">
        <div class="w-full max-w-5xl mx-auto p-6">
            @yield('content')
        </div>
    </div>
    <footer class="px-6 py-4 fixed bottom-0 w-full">
        Footer content here
    </footer>
</body>

</html>
