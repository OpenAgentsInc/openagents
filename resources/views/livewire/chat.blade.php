<div role="presentation" tabindex="0" class="flex flex-col h-full min-h-screen">
    <div class="flex-1 overflow-hidden">
        <div id="chatbox-container" x-ref="chatboxContainer" class="overflow-y-scroll h-full">
            <div class="relative"
                 x-data="{ pending: @entangle('pending').live }"
                 x-init="
                    let chatbox = $refs.chatboxContainer;
                    setTimeout(() => {
                        chatbox.scrollTo({ top: chatbox.scrollHeight, behavior: 'smooth' });
                    }, 250);
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
                    });

                    $nextTick(() => {
                        document.getElementById('message-input').focus(); // Focus the textarea
                    });
                ">
                <div class="flex flex-col text-sm pb-9" style="">
                    <div class="sticky top-0 mb-1.5 flex items-center justify-between z-10 h-14 p-2 px-5 font-semibold bg-black">
                        <div class="absolute left-1/2 -translate-x-1/2"></div>
                        <livewire:model-selector/>

                        @auth
                            <div class="flex flex-row items-center">
                                <x-icon.share class="cursor-pointer w-[24px] h-[24px] mr-[56px]"/>
                                <a href="/logout">
                                    <div class="select-none cursor-pointer bg-darkgray w-[32px] h-[32px] rounded-full text-[#d7d8e5] flex items-center justify-center">
                                        C
                                    </div>
                                </a>
                            </div>

                        @else
                            <x-login-button/>
                        @endauth
                    </div>
                    <div class="xl:-ml-[50px]">
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
            </div>
        </div>
    </div>
    <div class="w-full -ml-[25px]">
        <div class="sm:w-[584px] lg:w-[768px] mx-auto">
            <form wire:submit.prevent="sendMessage">
                <x-chat.input dusk="message-input" autofocus placeholder="Message OpenAgents..." :showIcon="true"
                              id="message-input"
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