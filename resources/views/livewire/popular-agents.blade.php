<div>
    @if (count($agents) > 0)
        <div class="hidden sm:block md:w-[800px] border border-offblack p-8 mt-12 rounded">
            <div>
                <div class="flex justify-between items-center">
                    <h4 class="font-bold">Trending</h4>
                </div>
                <p class="text-gray leading-relaxed">The most popular agents from our community</p>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-4">
                @foreach($agents as $agent)
                    <livewire:agent-card :agent="$agent" :key="$agent['id']"/>
                @endforeach
            </div>
        </div>
    @endif
</div>
