<div class="w-full relative z-50">
    <div class="justify-between flex gap-2 items-center overflow-hidden z-50">
        <div class="relative flex-1 text-right" x-data="{ dropdown: false }">
            <a href="/" wire:navigate>
                <button class="mt-3 p-1.5 rounded-md text-white">
                    <x-icon.plus class="h-6 w-6"></x-icon.plus>
                </button>
            </a>
        </div>
    </div>

    <div class="flex flex-col gap-2 mt-8 py-4 px-1">
        <span class="text-left text-sm text-[#777A82] px-2" x-cloak>
            Today
        </span>
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
    </div>
</div>