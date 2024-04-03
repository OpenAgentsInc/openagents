<div>
    <a href="/chat/{{ $thread->id }}"
       wire:navigate
       class="text-white hover:text-white bg-black  hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
        <div class="flex w-full">
            {{ $thread->title }}
        </div>
    </a>
</div>