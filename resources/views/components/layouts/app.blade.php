<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <script defer src="https://unpkg.com/@alpinejs/ui@3.13.8-beta.0/dist/cdn.min.js"></script>
    <script defer src="https://unpkg.com/@alpinejs/focus@3.13.8/dist/cdn.min.js"></script>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
    <x-social-tags/>

    <link rel="stylesheet" href="{{asset('vendor/swal-dark.min.css')}}">
</head>

<body class="h-screen bg-black antialiased" x-cloak
      x-data="{ sidebarOpen: window.innerWidth > 768, collapsed: false }">

<div class="relative z-0 flex h-full w-full overflow-hidden min-h-screen">
    <div class="h-[54px] flex justify-between items-center absolute top-0 left-0 py-2 px-4 w-[259px]">
        <button class="cursor-pointer text-white z-[9999]"
                @click="sidebarOpen = !sidebarOpen">
            <x-icon.menu class="h-6 w-6"/>
        </button>
        <a href="/" wire:navigate x-show="sidebarOpen"
           x-transition:enter="transition ease-out duration-200 delay-200"
           x-transition:enter-start="opacity-0"
           x-transition:enter-end="opacity-100"
           x-transition:leave="transition ease-in duration-100"
           x-transition:leave-start="opacity-100"
           x-transition:leave-end="opacity-0"
           class="z-[9999]"
        >
            <button class="p-1.5 pr-0 rounded-md text-white z-[9999]">
                <x-icon.plus class="h-6 w-6"></x-icon.plus>
            </button>
        </a>
    </div>


    <div class="flex-shrink-0 overflow-x-hidden sidebar"
         x-bind:class="{
            'sidebar-open': sidebarOpen,
            'sidebar-closed': !sidebarOpen
         }"
    >
        <div class="relative h-full w-[260px]">
            <div class="flex h-full min-h-0 flex-col">
                <div class="relative h-full w-full flex-1 items-start">
                    <div class="flex h-full w-full flex-col px-1 pb-3.5">
                        <div class="flex-col flex-1 transition-opacity duration-500 overflow-y-auto">
                            <livewire:layouts.sidebar-content/>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden hmmm"
         x-bind:style="`margin-left: ${sidebarOpen ? '0' : (window.innerWidth <= 768 ? '0' : '50px')}`"
    >
        <main class="relative h-full w-full flex-1 overflow-auto transition-width z-[1]">
            {{$slot}}
        </main>
    </div>
</div>

@livewire('wire-elements-modal')
<x-livewire-alert::scripts/>

@include('partials.copyblocks')

</body>

</html>
