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

<body class="h-full bg-black" x-cloak x-data="{ sidebarOpen: true, collapsed: false }">


{{--<livewire:navbar/>--}}

<div class="relative z-0 flex h-full w-full overflow-hidden">
    <div class="flex-shrink-0 overflow-x-hidden"
         x-cloak
         x-bind:class="{
            'w-[128px]': !sidebarOpen,
            'w-[260px] border-r border-darkgray': sidebarOpen
           }"
    >
        <button class="cursor-pointer h-[32px] m-4 mr-12" @click="sidebarOpen = !sidebarOpen">
            <x-icon.menu/>
        </button>
    </div>
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
        {{$slot}}
    </div>

</div>

@include('partials.modals')

@yield('modal')

</body>

</html>