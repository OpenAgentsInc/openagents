<div>
    <div class="flex h-screen w-full relative py-15">
        <div class="mx-auto w-full lg:max-w-[768px] relative">
            <div class="flex flex-col mb-20">
                <div id="chatbox-container" x-ref="chatboxContainer"
                     class="mt-[100px] mb-[5px] pb-[100px] overflow-auto bg-gray-900 text-white">
                    <div class="relative "
                         x-data="{ pending: @entangle('pending').live }"
                         x-init="
                            let chatbox = $refs.chatboxContainer;
                            setTimeout(() => {
                                chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                            }, 500);
                            let lastScrollHeight = chatbox.scrollHeight;

                            $watch('pending', value => {
                                if (value) {
                                    let scrollInterval = setInterval(() => {
                                        if (chatbox.scrollHeight != lastScrollHeight) {
                                            chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                                            lastScrollHeight = chatbox.scrollHeight;
                                        }

                                        if (!pending) clearInterval(scrollInterval);
                                    }, 1);
                                }
                    });">
                        @foreach($messages as $message)
                            @php
                                $author = $message['agent_id'] ? 'OpenAgents' : 'You';
                            @endphp
                            <x-chat.message :author="$author" :message="$message['body']"/>
                        @endforeach

                        @if($pending)
                            <x-chat.messagestreaming :author="$agent->name ?? 'Agent'"/>
                        @endif
                    </div>
                </div>

                <div class="absolute bottom-0 left-0 right-0 px-8">
                    <form wire:submit.prevent="sendMessage">
                        <x-chat.input autofocus placeholder="Message OpenAgents..." :showIcon="true"
                                      iconName="send"
                                      wire:model="message_input"
                                      onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                        />
                        <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
                    </form>

                    <x-chat-warning/>
                </div>

            </div>
        </div>

    </div>
</div>


</div>
