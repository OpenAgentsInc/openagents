<a href="/" wire:navigate>
    <!-- Hidden when collapsed -->
    <div {{ $attributes->class(["hidden-when-collapsed"]) }}>
        <div class="flex items-center gap-2">
            <x-icons.openagents class="w-4 text-white" />
            <span class="font-bold text-lg">
                OpenAgents
            </span>
        </div>
    </div>

    <!-- Display when collapsed -->
    <div class="display-when-collapsed hidden mx-5 mt-4 lg:mb-6 h-[28px]">
        <x-icons.openagents class="w-full h-5" />
    </div>
</a>