<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>{{ config('app.name', 'OpenAgents') }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('analytics')
</head>

<body class="bg-black text-white font-mono h-screen">
    <div class="flex flex-col h-full">
        @include('layouts.navigation')
        <main class="flex-1 overflow-hidden">
            <div class="flex flex-row w-screen h-full">
                <div class="w-1/5 border-r border-offblack shadow-xl nice-scrollbar overflow-y-auto">
                    what
                </div>
                <div class="w-4/5 flex flex-col px-2">
                    <div id="chatbox-container" class="grow nice-scrollbar weird-height">
                        hi
                    </div>
                </div>
            </div>
        </main>
    </div>
</body>

</html>
