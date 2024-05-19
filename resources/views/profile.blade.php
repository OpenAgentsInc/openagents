<!doctype html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <script defer src="https://unpkg.com/alpinejs@3/dist/cdn.min.js"></script>
    @include('partials.vite')
</head>
<body hx-boost="true">
<main class="p-12">
    <div class="flex flex-row gap-x-6">
        <img src="https://pbs.twimg.com/profile_images/1607882836740120576/3Tg1mTYJ.jpg"
             alt="{{ auth()->user()->name }}"
             class="rounded-xl w-[120px] h-[120px]"/>
        <div>
            <h1>{{ auth()->user()->name }}</h1>
            <h2>{{ $username }}</h2>
        </div>
    </div>
    <div class="m-3">
        <a href="https://x.com/{{ auth()->user()->username }}" target="_blank">
            <x-icon.x class="h-6 w-6"/>
        </a>
    </div>
</main>
</body>
</html>