<div role="presentation" tabindex="0" class="flex flex-col h-full min-h-screen">
    <div class="flex-1 overflow-hidden">
        <div class="scroll-me h-full">
            <div class="scroll-me">
                <div class="flex flex-col text-sm pb-9" style="">
                    <div class="sticky top-0 mb-1.5 flex items-center justify-between z-10 h-14 p-2 font-semibold bg-token-main-surface-primary">
                        <div class="absolute left-1/2 -translate-x-1/2"></div>
                        <livewire:model-selector/>
                        <x-icon.share class="cursor-pointer w-[24px] h-[24px] mr-[56px]"/>
                    </div>
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
    <div class="w-full">
        <div class="px-8 sm:w-[584px] lg:w-[768px] mx-auto">
            <form wire:submit.prevent="sendMessage">
                <x-chat.input dusk="message-input" autofocus placeholder="Message OpenAgents..." :showIcon="true"
                              iconName="send"
                              wire:model="message_input"
                              onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                />
                <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
            </form>
            <x-chat-warning/>
        </div>
    </div>
    {{--    <div class="w-full pt-2 md:pt-0 dark:border-white/20 md:border-transparent md:dark:border-transparent md:w-[calc(100%-.5rem)]"--}}
    {{--         style="padding-left: 0px; padding-right: 0px;">--}}
    {{--        <form class="stretch mx-2 flex flex-row gap-3 last:mb-2 md:mx-4 md:last:mb-6 lg:mx-auto lg:max-w-2xl xl:max-w-3xl">--}}
    {{--            <div class="relative flex h-full flex-1 flex-col">--}}
    {{--                <div class="absolute bottom-full left-0 right-0"></div>--}}
    {{--                <div class="flex w-full items-center">--}}
    {{--                    <div class="overflow-hidden [&amp;:has(textarea:focus)]:border-token-border-xheavy [&amp;:has(textarea:focus)]:shadow-[0_2px_6px_rgba(0,0,0,.05)] flex flex-col w-full flex-grow relative border dark:text-white rounded-2xl bg-token-main-surface-primary border-token-border-medium">--}}
    {{--                        <textarea id="prompt-textarea" tabindex="0"--}}
    {{--                                  rows="1"--}}
    {{--                                  placeholder="Message OpenAgents…"--}}
    {{--                                  class="m-0 w-full resize-none border-0 bg-transparent focus:ring-0 focus-visible:ring-0 dark:bg-transparent py-[10px] pr-10 md:py-3.5 md:pr-12 max-h-[25dvh] max-h-52 placeholder-black/50 dark:placeholder-white/50 pl-10 md:pl-[55px]"--}}
    {{--                                  style="height: 100px; overflow-y: hidden;"></textarea>--}}
    {{--                        <div type="button" aria-haspopup="dialog" aria-expanded="false" aria-controls="radix-:rdm:"--}}
    {{--                             data-state="closed" class="absolute bottom-2 md:bottom-3 left-2 md:left-4">--}}
    {{--                            <div class="flex">--}}
    {{--                                <button class="btn relative p-0 text-black dark:text-white" aria-label="Attach files">--}}
    {{--                                    <div class="flex w-full gap-2 items-center justify-center">--}}
    {{--                                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none"--}}
    {{--                                             xmlns="http://www.w3.org/2000/svg">--}}
    {{--                                            <path fill-rule="evenodd" clip-rule="evenodd"--}}
    {{--                                                  d="M9 7C9 4.23858 11.2386 2 14 2C16.7614 2 19 4.23858 19 7V15C19 18.866 15.866 22 12 22C8.13401 22 5 18.866 5 15V9C5 8.44772 5.44772 8 6 8C6.55228 8 7 8.44772 7 9V15C7 17.7614 9.23858 20 12 20C14.7614 20 17 17.7614 17 15V7C17 5.34315 15.6569 4 14 4C12.3431 4 11 5.34315 11 7V15C11 15.5523 11.4477 16 12 16C12.5523 16 13 15.5523 13 15V9C13 8.44772 13.4477 8 14 8C14.5523 8 15 8.44772 15 9V15C15 16.6569 13.6569 18 12 18C10.3431 18 9 16.6569 9 15V7Z"--}}
    {{--                                                  fill="currentColor"></path>--}}
    {{--                                        </svg>--}}
    {{--                                    </div>--}}
    {{--                                </button>--}}
    {{--                                <input multiple="" type="file" tabindex="-1" class="hidden" style="display: none;">--}}
    {{--                            </div>--}}
    {{--                        </div>--}}
    {{--                        <button class="absolute bottom-1.5 right-2 rounded-lg border border-black bg-black p-0.5 text-white transition-colors enabled:bg-black disabled:text-gray-400 disabled:opacity-10 dark:border-white dark:bg-white dark:hover:bg-white md:bottom-3 md:right-3"--}}
    {{--                                data-testid="send-button"><span class="" data-state="closed"><svg width="24" height="24"--}}
    {{--                                                                                                  viewBox="0 0 24 24"--}}
    {{--                                                                                                  fill="none"--}}
    {{--                                                                                                  class="text-white dark:text-black"><path--}}
    {{--                                            d="M7 11L12 6L17 11M12 18V7" stroke="currentColor" stroke-width="2"--}}
    {{--                                            stroke-linecap="round" stroke-linejoin="round"></path></svg></span></button>--}}
    {{--                    </div>--}}
    {{--                </div>--}}
    {{--            </div>--}}
    {{--        </form>--}}
    {{--        <x-chat-warning/>--}}
    {{--    </div>--}}
</div>