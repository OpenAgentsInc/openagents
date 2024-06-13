<div>
    <div class="w-full h-[70vh] flex flex-col justify-center">
        <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
            <p class="text-[16px] text-gray">Now speaking with...</p>
            <div class="w-[400px]">
                <livewire:agent-card :agent="$agent" :key="$agent->id" :show_chat_button="false" />
            </div>
        </div>
    </div>
</div>
