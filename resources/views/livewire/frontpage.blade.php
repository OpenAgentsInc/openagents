<div class="h-full">
    <livewire:navbar/>

    <div class="h-full flex flex-col justify-center items-center">
        <div class="fixed w-full px-8 sm:w-[584px]">
            <h2 class="-mt-8 mb-12 text-center">What can we do for you?</h2>
            <form wire:submit.prevent="sendFirstMessage">
                <x-chat.input autofocus placeholder="Message OpenAgents..." :showIcon="true" iconName="send"
                              wire:model="first_message"
                              onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                />
                <button class="hidden" id="send-message" type="submit"/>
            </form>
        </div>
    </div>

</div>