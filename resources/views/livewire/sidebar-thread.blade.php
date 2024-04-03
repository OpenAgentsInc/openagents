<div class="flex justify-between items-center">
    <a href="/chat/{{ $thread->id }}"
       wire:navigate
       class="text-white hover:text-white bg-black hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
        <div class="flex w-full">
            {{ $thread->title }}
        </div>
    </a>

    <!-- Popover Trigger and Content -->
    <div x-data="{ open: false }" @click.away="open = false" class="relative">
        <button @click="open = !open" class="p-2">
            <x-icon.dots class="h-4 w-4 text-white"/>
        </button>

        <ul x-show="open" x-popover:panel
            class="absolute right-0 mt-2 w-48 bg-white shadow-lg rounded-md overflow-hidden z-50"
            @keydown.escape.window="open = false">
            <li><a href="#" class="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-100">Edit</a></li>
            <li><a href="#" class="block px-4 py-2 text-sm text-gray-700 hover:bg-gray-100">Delete</a></li>
            <!-- Add more options as needed -->
        </ul>
    </div>
</div>