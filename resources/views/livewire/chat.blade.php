<div>

    <div class="flex h-screen w-full overflow-hidden py-15">
        {{-- <livewire:navbar/> --}}

        <x-slot:sidecontent>
            @livewire('layouts.sidebar.content')
        </x-slot:sidecontent>

        <x-slot:sidecontent_mobile>
            @livewire('layouts.sidebar.content')
        </x-slot:sidecontent_mobile>


        <div class="w-full h-screen flex flex-col" x-bind:class="{
                'lg:pl-[300px]': !collapsed,
                'lg:pl-16': collapsed,
                '-translate-x-full': !showSidebar
             }">
            {{-- <div class="fixed top-[80px] w-screen left-[0px] right-0 h-[40px] bg-gradient-to-b from-black to-transparent z-[9]"></div> --}}

            <div id="chatbox-container" x-ref="chatboxContainer"
                 class="pt-[80px] mb-[5px] pb-[60px] flex-1 overflow-auto bg-gray-900 text-white"
                 x-data="{ pending: @entangle('pending').live }" x-init="
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

            <div class="xl:fixed bottom-0 left-[0px] right-0 h-[80px] px-4 py-3 flex items-center z-10"
                 x-bind:class="{
                    'lg:pl-[300px]': !collapsed,
                    'lg:pl-16': collapsed,
                    '-translate-x-full': !showSidebar
                 }">
                <div class="fixed bottom-0 w-screen left-[0px] right-0 h-[70px] bg-black z-5"></div>
                <div class="fixed bottom-[70px] w-screen left-[0px] right-0 h-[40px] bg-gradient-to-t from-black to-transparent z-5"></div>
                <div class="w-full pt-2 md:pt-0 dark:border-white/20 md:border-transparent md:dark:border-transparent md:w-[calc(100%-.5rem)]">
                    <form wire:submit.prevent="sendMessage"
                          class="stretch mx-2 flex flex-row gap-3 last:mb-2 md:mx-4 md:last:mb-6 lg:mx-auto lg:max-w-2xl xl:max-w-3xl">
                        <div class="relative flex h-full flex-1 items-stretch md:flex-col">
                            <div class="flex w-full items-center text-white">
                                <div x-data x-init="$refs.answer.focus()"
                                     class="overflow-hidden [&amp;:has(textarea:focus)]:border-gray [&amp;:has(textarea:focus)]:shadow-[0_2px_6px_rgba(0,0,0,.05)] flex flex-col w-full dark:border-gray flex-grow relative border border-gray dark:text-white rounded-[6px]">
                                            <textarea x-ref="answer" id="message-input" name="message_input"
                                                      wire:model="message_input" autofocus
                                                      onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                                                      tabindex="0" rows="1"
                                                      placeholder="{{ 'Message ' . $agent->name . '...' }}" class=" outline-none m-0 w-full resize-none border-0 bg-transparent
                                              focus:ring-0 focus-visible:ring-0 dark:bg-transparent max-h-25 py-[10px] pr-10
                                              md:py-3.5 md:pr-12 placeholder-white/50 pl-10 md:pl-[22px]"
                                                      style="height: 52px; overflow-y: hidden;"></textarea>
                                    <button id="send-message" class="absolute bottom-1.5 right-2 rounded-lg border border-black bg-black p-0.5
                                        text-white transition-colors enabled:bg-black disabled:text-gray-400
                                        disabled:opacity-25 dark:border-white dark:bg-white dark:hover:bg-white md:bottom-3
                                        md:right-3">
                                                <span class="" data-state="closed">
                                                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none"
                                                         class="text-gray">
                                                        <path d="M7 11L12 6L17 11M12 18V7" stroke="currentColor"
                                                              stroke-width="2" stroke-linecap="round"
                                                              stroke-linejoin="round"></path>
                                                    </svg>
                                                </span>
                                    </button>
                                </div>
                            </div>
                        </div>
                    </form>
                </div>
            </div>
        </div>
    </div>

</div>
