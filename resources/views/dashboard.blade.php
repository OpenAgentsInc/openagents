<x-layout>
    <div class="relative min-h-screen overflow-hidden bg-black">
        @include('dashboard.background')
        @include('dashboard.top-buttons')
        @include('dashboard.main-content')
    </div>

    @include('dashboard.styles')
</x-layout>