@php use App\AI\Models; @endphp
<div role="presentation" tabindex="0" class="flex flex-col h-full min-h-screen">
    <div class="flex-1 overflow-hidden">
        <x-chatbox :autoscroll="auth()->check() ? auth()->user()->autoscroll : true">
            <div class="flex flex-col text-sm pb-9" style="">
                <div class="h-[52px] sticky top-0 flex flex-row items-center justify-between z-10 px-5 bg-black">
                    <div class="absolute left-1/2 -translate-x-1/2"></div>
                    <livewire:model-selector :thread="$thread"/>

                    @auth
                        <x-user-menu/>
                    @else
                        <div class="flex flex-row items-center">
                            <x-login-buttons/>
                        </div>
                    @endauth
                </div>
                <div class="xl:-ml-[50px] pt-8 chat">
                    @if (count($messages) === 0)
                        <div class="w-full h-[70vh] flex flex-col justify-center">
                            <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
                                <x-logomark :size="1"></x-logomark>
                                <h3 class="mt-[36px] text-center leading-relaxed">How can we help you today?</h3>
                                <p class="pointer-events-auto select-auto mt-[30px] text-gray">Blog: <a
                                            class="underline" href="/goodbye-chatgpt" wire:navigate>Goodbye
                                        ChatGPT</a></p>
                            </div>
                        </div>
                    @endif

                    @php
                        $models = Models::MODELS;
                    @endphp

                    @foreach($messages as $message)
                        @php
                            $author = !empty($message['model']) ? $models[$message['model']]["name"] : 'You';
                        @endphp
                        <x-chat.message :author="$author" :message="$message['body']"></x-chat.message>
                    @endforeach

                    @if($pending)
                        <x-chat.messagestreaming
                                :author="$models[$selectedModel]['name']"></x-chat.messagestreaming>
                    @endif

                    @if ($showNoMoreMessages)
                        @auth
                            @if (count($messages) === 0)
                                <div class="-mt-[15%]"></div>
                            @endif
                            <div class="px-[24px] py-[32px] pb-8 w-[600px] mx-auto border border-[#3C3E42] rounded-[12px]">
                                <h2 class="font-bold text-[32px]">Upgrade to continue</h2>
                                <div class="flex flex-col justify-center items-center w-full">
                                    <p class="px-1 my-[32px] leading-relaxed text-text">Upgrade to Pro for
                                        $10/month
                                        and receive 100 responses per day. Secure billing via Stripe.</p>
                                    <a class="w-full" href="/upgrade">
                                        <x-button class="w-full justify-center font-medium">Upgrade plan
                                        </x-button>
                                    </a>
                                </div>
                            </div>
                        @else
                            <div class="px-[24px] py-[32px] pb-8 w-[600px] mx-auto border border-[#3C3E42] rounded-[12px]">
                                <h2 class="font-bold text-[32px]">Sign up to continue</h2>
                                <div class="flex flex-col justify-center items-center w-full">
                                    <p class="px-1 my-[32px] leading-relaxed text-text">Sign up for OpenAgents
                                        and
                                        receive 10
                                        free
                                        responses per day
                                        from
                                        the world's
                                        leading chat
                                        agents.</p>
                                    <a wire:click="$dispatch('openModal', { component: 'auth.join' })"
                                       class="my-1 w-full">
                                        <x-button class="w-full justify-center font-medium">Sign up</x-button>
                                    </a>
                                </div>
                            </div>
                        @endauth
                    @endif
                </div>
            </div>
        </x-chatbox>
    </div>
    <div class="w-full lg:-ml-[25px] px-3">
        <div class="sm:w-[584px] lg:w-[768px] mx-auto">
            @if ($showNoMoreMessages)

            @else
                <form wire:submit.prevent="sendMessage">
                    <x-chat.textarea id="message-input" minRows="1" default="Message OpenAgents..."
                                     :showIcon="true"
                                     iconName="send"
                                     min-rows="1"
                                     max-rows="12"
                                     wire:model="message_input"
                                     wireModel="message_input"
                                     wire:ignore
                                     onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                                     class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white"/>

                    <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
                </form>
                <livewire:messages-remaining/>
            @endif
        </div>
    </div>
</div>