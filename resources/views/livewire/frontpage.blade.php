<div class="h-full flex flex-col justify-center items-center">
    <h1>Do something epic</h1>
    <h4 class="mt-4 mb-8 text-gray">What do you want to accomplish?</h4>


    <form wire:submit.prevent="sendMessage" class="w-[450px]">
        <x-input autofocus placeholder="I want an agent to..." :showIcon="true" iconName="send"
                 wire:model="body"
                 onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
        ></x-input>
        <button class="hidden" id="send-message" type="submit"/>
    </form>

</div>
