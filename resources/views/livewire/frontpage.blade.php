<div class="h-full">
    <livewire:navbar/>

    <div class="h-full flex flex-col justify-center items-center">
        <div class="fixed w-full px-8 sm:w-[584px]">
            <h2 class="-mt-8 mb-12 text-center">How can we help you today?</h2>

            <form wire:submit.prevent="sendFirstMessage">
                <x-chat.input dusk="first-message-input" autofocus placeholder="Message OpenAgents..." :showIcon="true"
                              iconName="send"
                              wire:model="first_message"
                              onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                />
                <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>

            </form>
            <p class="text-center text-gray mt-12 text-sm">
                All conversations are public (read & write).<br/>Don't share sensitive info.
            </p>

        </div>
    </div>

</div>