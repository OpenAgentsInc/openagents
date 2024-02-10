<x-app-layout>
    <div class="my-12 mx-auto w-full">
        <div class="text-white max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
            <h1 class="text-[32px] font-bold mb-[32px]">Earnings Dashboard</h1>

            <nav class="flex space-x-[32px] mb-[32px] text-normal font-bold border-b-2 border-darkgray">
                <a href="{{ route('dashboard') }}"
                    class="-mb-[3px] pb-1 border-b-4 border-white">Overview</a>
                <a href="#">Agents</a>
                <a href="#">Jobs</a>
                <a href="#">Revenue</a>
            </nav>

            <div class="grid grid-cols-3 gap-4 mb-4">
                <x-stat-card title="Monthly earnings" value="$5,000.00" change="+10%" />
                <x-stat-card title="Plugin uses" value="3,000" change="+5%" />
                <x-stat-card title="Average rating" value="4.5" change="+0.5" />
            </div>

            <div class="mb-4">
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
    </div>
</x-app-layout>
