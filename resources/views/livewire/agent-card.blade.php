<a class="pointer-events-auto select-auto" href="/chat?agent={{ $agent["id"] }}"
   wire:navigate>
    <div class="p-4 rounded-lg relative">
        <div class="flex">
            <div class="mt-1 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">
                <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">
            </div>
            <div class="flex-1 pl-4">
                <h4 class="text-lg font-bold">{{ $agent['name'] }}</h4>
                <span class="text-gray">{{ $agent['about'] }}</span>
                <div>
                    <p>Threads: {{ $agent->thread_count }},
                        Users: {{ $agent->unique_users_count }}</p>
                    <p>By: {{ $agent->creator_username }}</p>
                </div>
            </div>
        </div>
    </div>
</a>