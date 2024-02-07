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

<body class="bg-white dark:bg-black text-black dark:bg-black">
    <div class="fixed right-[20px] top-[20px]">
        <x-theme-switcher />
    </div>
    <div class="max-w-5xl mx-auto my-10 p-6">
        @yield('content')
    </div>
</body>

</html>
