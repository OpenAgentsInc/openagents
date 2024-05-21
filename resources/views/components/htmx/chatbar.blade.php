<div class="w-full px-3">
    <div class="sm:w-[584px] lg:w-[768px] mx-auto">
        <textarea id="message-input" minRows="1" default="Message OpenAgents..."
                  :showIcon="true" iconName="send" min-rows="1" max-rows="12"
                  wire:model="message_input"
                  wireModel="message_input"
                  :image-upload="auth()->check() && auth()->user()->isPro"
                  wire:ignore dusk="message-input"
                  onkeydown="if(event.keyCode === 13 && !event.shiftKey && window.innerWidth > 768) { event.preventDefault(); document.getElementById('send-message').click(); }"
                  class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white"></textarea>
    </div>
</div>