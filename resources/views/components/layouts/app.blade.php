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

<body class="h-screen bg-black antialiased" x-cloak x-data="{ sidebarOpen: true, collapsed: false }">

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
                            <div class="w-full">
                                <div class="flex gap-2 items-center overflow-hidden fixed z-50" x-bind:class="{
                                    'justify-between': sidebarOpen,
                                    'justify-center': collapsed
                                   }">
                                    <button class="z-50 absolute top-0 left-0 cursor-pointer h-[28px] w-[28px] m-4 mt-[18px] mr-12"
                                            @click="sidebarOpen = !sidebarOpen">
                                        <x-icon.menu/>
                                    </button>

                                    <div class="relative flex-1 text-right mr-6" x-data="{ dropdown: false }">
                                        <button @click="dropdown= !dropdown" x-cloak
                                                class="mt-4 p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                                            <x-icon.plus class="h-6 w-6"></x-icon.plus>
                                        </button>
                                    </div>
                                </div>
                            </div>
                            <livewire:layouts.sidebar.content/>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden hmmm"
         :style="`margin-left: ${sidebarOpen ? '0' : '50px'}`"
    >
        <main class="relative h-full w-full flex-1 overflow-auto transition-width">
            {{$slot}}
        </main>
    </div>
</div>

@livewire('wire-elements-modal')

</body>

</html>
