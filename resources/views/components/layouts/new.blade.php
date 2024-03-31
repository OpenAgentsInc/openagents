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

    <style>
        /* Custom CSS for transitioning the sidebar */
        .sidebar {
            /* Apply transition to both border-color and width */
            transition: border-color 0.3s ease-in-out, width 0.3s ease-in-out;
        }

        .hmmm {
            transition: margin-left 0.3s ease-in-out;
        }

        .sidebar-open {
            width: 260px;
            border-right: 1px solid rgba(255, 255, 255, 0.15);
        }

        .sidebar-closed {
            width: 0px; /* Collapsed width */
            border-right: 1px solid rgba(0, 0, 0, 0); /* Fully transparent when closed */
        }
    </style>
</head>

<body class="h-full min-h-screen bg-black" x-cloak x-data="{ sidebarOpen: true, collapsed: false }">

<div class="relative z-0 flex h-full w-full overflow-hidden min-h-screen">
    <button class="z-50 absolute top-0 left-0 cursor-pointer h-[32px] m-4 mr-12" @click="sidebarOpen = !sidebarOpen">
        <x-icon.menu/>
    </button>
    <div class="flex-shrink-0 overflow-x-hidden sidebar"
         x-cloak
         x-bind:class="{
            'sidebar-open': sidebarOpen,
            'sidebar-closed': !sidebarOpen
           }"
    >
        <livewire:layouts.sidebar.content/>
    </div>
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden hmmm"
         :style="`margin-left: ${sidebarOpen ? '0' : '50px'}`"
    >
        <main class="relative h-full w-full flex-1 overflow-auto transition-width">
            {{$slot}}
        </main>
    </div>

</div>

@include('partials.modals')

@yield('modal')

</body>

</html>