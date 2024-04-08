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

    <div class="flex flex-col gap-2 mt-8 py-4 px-1" @thread_updated="$refresh">
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
    </div>

    <div class="flex flex-col gap-2 py-4 px-1 mt-auto">
        <ol>
            <li>
                <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                        <a class="flex items-center gap-2 py-2"
                           wire:navigate
                           href="/blog"
                        >
                            <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">
                                Blog
                            </div>
                        </a>
                    </div>
                </div>
            </li>
            @pro
            <li>
                <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                        <a href="/subscription" target="_blank" class="flex items-center gap-2 py-2">
                            <div class="relative grow overflow-hidden whitespace-nowrap">
                                Manage subscription
                            </div>
                        </a>
                    </div>
                </div>
            </li>
            @else
                @auth
                    <li>
                        <div class="relative z-[15]">
                            <div class="group relative rounded-lg active:opacity-90 px-3">
                                <a class="flex items-center gap-2 py-2"
                                   wire:click="$dispatch('openModal', { component: 'modals.upgrade' })"
                                >
                                    <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">
                                        Upgrade to Pro
                                    </div>
                                </a>
                            </div>
                        </div>
                    </li>
                @endauth
                @endpro
        </ol>
    </div>
</div>
