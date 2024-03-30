<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
    @include('partials.ogtags')
</head>

<body class="h-full bg-black" x-cloak x-data="{ sidebarOpen: false, showSidebar: true, collapsed: false }">

<div class="h-full">
    <div class="flex flex-col z-[20]">
        <livewire:navbar/>

        <main class="flex-1">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                {{$slot}}
            </div>
        </main>
    </div>
</div>

@include('partials.modals')
@yield('modal')

</body>

</html>

