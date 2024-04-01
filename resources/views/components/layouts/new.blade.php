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

    <div class="flex-shrink-0 overflow-x-hidde sidebar"
         x-cloak
         x-bind:class="{
            'sidebar-open': sidebarOpen,
            'sidebar-closed': !sidebarOpen
           }"
    >
    <nav class="flex-grow w-full">
        <livewire:layouts.sidebar.content/>
    </nav>
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
