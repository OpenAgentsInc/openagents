<div class="select-auto pointer-events-auto border border-offblack rounded-lg p-4 flex flex-col leading-normal"
>
    <div class="items-center gap-1.5 pr-2  group-hover:flex">
        @auth
            @if ($agent->user_id == auth()->user()->id)
                <div x-data="{ isOpen: false }" class="relative flex-1 text-right">
                    <button @click="isOpen = !isOpen"
                            class="p-1.5 rounded-md text-gray hover:bg-[#262626]">
                        <x-icon.dots role='button' class="w-4 h-4"></x-icon.dots>
                    </button>
                    <div x-show="isOpen" @click.away="isOpen = false"
                         class="absolute z-10 top-12 right-0 w-64 rounded-lg bg-black border border-offblack shadow-md text-sm text-gray">
                        <div class="p-2 text-left">
                            <a href="{{ route('agents.edit', ['agent' => $agent]) }}"
                               wire:navigate
                               class="block w-full p-2 text-left rounded-md hover:text-white  hover:bg-[#262626] duration-150"
                               rel="nofollow">Edit</a>
                            <a role="button"
                               x-on:click="Livewire.dispatch('openModal', { component: 'agents.modals.delete', arguments: { agent: {{ $agent->id }} } })"
                               class="block w-full p-2 text-left rounded-md text-red hover:bg-[#262626] duration-150"
                               rel="nofollow">Delete</a>
                        </div>
                    </div>
                </div>
            @endif
        @endauth
    </div>

    <div class="mt-1 mb-3 w-[20px] h-[20px] sm:w-[60px] sm:h-[60px]">
        <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">
    </div>

    <div class="font-bold text-xl">{{ $agent['name'] }}</div>

    <a href="/u/{{ $agent->creator_username }}">
        <div class="flex items-center hover:underline text-gray">
            <div class="text-xs">
                <p class="text-sm leading-none">From: {{ $agent->creator_username }}</p>
            </div>
            <img class="w-5 h-5 rounded-full mx-4" src="{{ $agent->creator_picture }}"
                 alt="Avatar of {{ $agent->creator_username }}">
        </div>
    </a>
    <div class="flex-grow">
        <p class="text-sm text-text my-1">{{ $agent['about'] }}</p>
    </div>

    <div class="text-gray mt-4 gap-x-6 flex justify-end items-center">
        <div class="flex items-center">
            <x-icon.bitcoin class="w-4 h-4 mr-1"/>
            <span>{{ $agent->sats_earned ?? 0 }}</span>
        </div>
        <div class="flex items-center">
            <x-icon.chats class="w-4 h-4 mr-1"/>
            <span>{{ $agent->thread_count }}</span>
        </div>
        <div class="flex items-center">
            <x-icon.user class="w-4 h-4 mr-1"/>
            <span>{{ $agent->unique_users_count }}</span>
        </div>
    </div>
</div>
