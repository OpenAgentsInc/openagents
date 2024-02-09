<!DOCTYPE html>
<html class="dark"
    lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>OpenAgents</title>
    @include('partials.vite')
    @include('analytics')
</head>

<body class="bg-black text-white font-mono antialiased">
    <div class="min-h-screen flex flex-col sm:justify-center items-center pt-6 sm:pt-0">
        <!--
    <div>
            <a href="/">
                <x-application-logo class="w-20 h-20 fill-white" />
            </a>
        </div>
-->

        <div class="w-full sm:max-w-md mt-6 px-6 py-4 shadow-md overflow-hidden sm:rounded-lg">
            {{ $slot }}
        </div>
    </div>
</body>

</html>
