<div class="flex flex-col w-full relative z-50 h-full">
    <div class="justify-between flex gap-2 items-center overflow-hidden z-50">
        <div class="relative flex-1 text-right" x-data="{ dropdown: false }">
            <a href="/" wire:navigate>
                <button class="mt-2 p-1.5 rounded-md text-white">
                    <x-icon.plus class="h-6 w-6"></x-icon.plus>
                </button>
            </a>
        </div>
    </div>

    <div class="flex flex-col flex-grow gap-2 mt-8 py-4 px-1" @thread_updated="$refresh">
        <span class="text-left text-sm text-[#777A82] px-2" x-cloak>
            Today
        </span>
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
    </div>

    <!-- New section for links -->
    <div class="flex flex-col gap-2 py-4 px-1 mt-auto">
        <ol>
            @auth
                <li class="ml-4"><a target="_blank" href="/billing">Billing</a></li>
            @endauth
        </ol>
    </div>
</div>
