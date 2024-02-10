<x-app-layout>
    <div class="text-white">
        <h1 class="text-3xl font-bold mb-4">Earnings Dashboard</h1>

        <nav class="flex space-x-4 mb-4">
            <a href="{{ route('dashboard') }}" class="text-lg">Overview</a>
            <a href="#" class="text-lg">Agents</a>
            <a href="#" class="text-lg">Jobs</a>
            <a href="#" class="text-lg">Revenue</a>
        </nav>

        <hr class="mb-4" />

        <div class="grid grid-cols-3 gap-4 mb-4">
            <x-stat-card title="Monthly earnings" value="$5,000.00" change="+10%" />
            <x-stat-card title="Plugin uses" value="3,000" change="+5%" />
            <x-stat-card title="Average rating" value="4.5" change="+0.5" />
        </div>

        <div class="mb-4">
            {{-- Replace this comment with your chart implementation. --}}
            <x-chart />
        </div>

        <div class="flex space-x-2">
            <x-tab-link
                href="{{ route('dashboard', ['period' => '1w']) }}"
                :active="request('period') === '1w'">1W</x-tab-link>
            <x-tab-link
                href="{{ route('dashboard', ['period' => '1m']) }}"
                :active="request('period') === '1m'">1M</x-tab-link>
            <x-tab-link
                href="{{ route('dashboard', ['period' => '3m']) }}"
                :active="request('period') === '3m'">3M</x-tab-link>
            <x-tab-link
                href="{{ route('dashboard', ['period' => '1y']) }}"
                :active="request('period') === '1y'">1Y</x-tab-link>
            <x-tab-link
                href="{{ route('dashboard', ['period' => 'all']) }}"
                :active="request('period') === 'all'">ALL</x-tab-link>
        </div>
    </div>
</x-app-layout>
