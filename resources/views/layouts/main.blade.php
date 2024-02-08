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
    <div class="flex flex-grow items-center justify-center">
        <div class="w-full max-w-5xl mx-auto p-6">
            @yield('content')
        </div>
    </div>
    <footer class="text-sm px-6 sm:px-8 py-4 fixed bottom-0 w-full">
        <div class="mx-auto flex justify-end space-x-4">
            <a href="https://twitter.com/OpenAgentsInc" target="_blank"
                class="text-gray hover:text-black dark:hover:text-white mx-2">Twitter</a>
            <a href="https://github.com/OpenAgentsInc/openagents" target="_blank"
                class="text-gray hover:text-black dark:hover:text-white mx-2">GitHub</a>
        </div>
    </footer>
</body>

</html>
