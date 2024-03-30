<div class="h-full flex flex-col">
    <livewire:navbar/>

    <!-- Container for centering the content vertically and horizontally -->
    <div class="flex-grow flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <x-logomark :size="2"/>
        <h3 class="mt-[16px] mb-12">How can we help you today?</h3>
    </div>

    <!-- Input bar at the bottom -->
    <div class="w-full px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <form wire:submit.prevent="sendFirstMessage" class="mb-6">
            <x-chat.input dusk="first-message-input" autofocus placeholder="Message OpenAgents..." :showIcon="true"
                          iconName="send"
                          wire:model="first_message"
                          onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
            />
            <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
        </form>
        <p class="text-center text-gray text-sm">
            Chat agents make mistakes. Don't share sensitive info.
        </p>
    </div>
</div>
