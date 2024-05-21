<div class="w-full px-3">
    <div class="sm:w-[584px] lg:w-[768px] mx-auto relative">
        <textarea id="message-input" minRows="1" default="Message OpenAgents..."
                  :showIcon="true" iconName="send" min-rows="1" max-rows="12"
                  wire:model="message_input"
                  :image-upload="auth()->check() && auth()->user()->isPro"
                  wire:ignore dusk="message-input"
                  onkeydown="if(event.keyCode === 13 && !event.shiftKey && window.innerWidth > 768) { event.preventDefault(); document.getElementById('send-message').click(); }"
                  class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81]
                   focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white
                   resize-none"></textarea>
        <button
                class="absolute bottom-[10px] right-[10px] text-black shadow rounded bg-white hover:bg-white/90"
                dusk="send-message-button"
        >
            <x-icon name="send" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
        </button>
    </div>
</div>