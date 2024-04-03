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

<body class="h-screen bg-black antialiased" x-cloak x-data="{ sidebarOpen: false, collapsed: false }">

<div class="relative z-0 flex h-full w-full overflow-hidden min-h-screen">
    <div class="flex-shrink-0 overflow-x-hidden sidebar"
         x-bind:class="{
            'sidebar-open': sidebarOpen,
            'sidebar-closed': !sidebarOpen
         }"
    >
        <div class="h-full w-[260px]">
            <div class="flex h-full min-h-0 flex-col">
                <div class="relative h-full w-full flex-1 items-start">
                    <div class="flex h-full w-full flex-col px-3 pb-3.5">
                        <div class="flex-col flex-1 transition-opacity duration-500 -mr-2 pr-2 overflow-y-auto">
                            <livewire:layouts.sidebar.content/>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
        <main class="relative h-full w-full flex-1 overflow-auto transition-width">
            <div class="fixed left-0 top-1/2 z-40"
                 style="transform: translateX(260px) translateY(-50%) rotate(0deg) translateZ(0px);">
                <button
                        @click="sidebarOpen = !sidebarOpen"
                ><span class="" data-state="closed">
                        <div class="flex h-[72px] w-8 items-center justify-center">
                            <div
                                    class="flex h-6 w-6 flex-col items-center">
                                <div class="h-3 w-1 rounded-full"
                                     style="background: #ffffff; transform: translateY(0.15rem) rotate(0deg) translateZ(0px);"></div>
                                <div
                                        class="h-3 w-1 rounded-full"
                                        style="background: #ffffff; transform: translateY(-0.15rem) rotate(0deg) translateZ(0px);">

                                </div>
                            </div>
                        </div>
                        <span
                                style="position: absolute; border: 0px; width: 1px; height: 1px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; overflow-wrap: normal;">Close sidebar</span>
                    </span>
                </button>
            </div>
            {{$slot}}
        </main>
    </div>
</div>

@livewire('wire-elements-modal')

</body>

</html>
