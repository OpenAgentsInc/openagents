<div class="relative z-[15]"
     style="opacity: 1; height: auto; overflow: hidden; transform: none; transform-origin: 50% 50% 0px;"
>
    <div class="group relative rounded-lg active:opacity-90"><a
                href="/chat/{{ $thread->id }}" class="flex items-center gap-2 py-2" wire:navigate>
            <div class="relative grow overflow-hidden whitespace-nowrap">
                {{ $thread->title }}
            </div>
        </a>
        <div x-popover class="absolute bottom-0 right-0 top-0 items-center gap-1.5 flex">
            <button class="flex items-center justify-center text-token-text-primary transition hover:text-token-text-secondary radix-state-open:text-token-text-secondary"
                    x-popover:button type="button" aria-haspopup="menu" aria-expanded="false">
                <x-icon.dots role='button' class="w-4 h-4"></x-icon.dots>
            </button>

            <ul x-popover:panel>
                <a href="#about">About Us</a>
                <a href="#team">Team</a>
            </ul>
        </div>
    </div>
</div>
