<div class="h-full flex flex-col justify-center items-center">
    <div class="fixed w-full px-8 sm:w-[584px]">
        <x-logomark size="1"/>
        <h2 class="mt-4 mb-12 text-[#D7D8E5]">Welcome!<br/>
            How can we help you?</h2>
        <form wire:submit.prevent="sendFirstMessage">
            <x-chat.input autofocus placeholder="Message OpenAgents..." :showIcon="true" iconName="send"
                          wire:model="first_message"
                          onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
            />
            <button class="hidden" id="send-message" type="submit"/>
        </form>
    </div>
</div>
