<div class="h-full flex flex-col">
    <livewire:navbar/>

    <!-- Fixed container for centering the content vertically and horizontally -->
    <div class="fixed pointer-events-none top-0 bottom-0 left-0 right-0 flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <x-logomark :size="2"/>
        <h3 class="mt-[16px] mb-12">How can we help you today?</h3>
    </div>

    <!-- Fixed input bar at the bottom -->
    <div class="fixed bottom-0 left-0 right-0 px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <form wire:submit.prevent="sendFirstMessage">
            <x-chat.input dusk="first-message-input" autofocus placeholder="Message OpenAgents..." :showIcon="true"
                          iconName="send"
                          wire:model="first_message"
                          onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
            />
            <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
        </form>
        <x-chat-warning/>
    </div>
</div>
