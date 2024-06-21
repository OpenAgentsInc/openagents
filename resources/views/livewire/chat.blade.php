@php use App\AI\Models; @endphp
<div role="presentation" tabindex="0" class="flex flex-col h-full min-h-screen">
    <div class="flex-1 overflow-hidden">
        <x-chatbox :autoscroll="auth()->check() ? auth()->user()->autoscroll : true">

            <div class="h-[52px] sticky top-0 flex flex-row items-center justify-between z- px-5 z-10 bg-black">
                <div class="absolute left-1/2 -translate-x-1/2"></div>
                <livewire:model-selector :thread="$thread"/>

            </div>
            <div class="w-full overflow-y-auto flex flex-col items-center">
                <div class="w-full prose prose-invert messages max-w-4xl flex flex-col text-sm pb-9" style="">
                    <div class="xl:-ml-[50px] pt-8 chat">
                        @if($showNoMoreMessages)
                            <!-- spacer -->
                            <div class="h-[40vh]"></div>
                        @else
                            @if (count($messages) === 0 && $thread->agent )
                                @livewire('agents.partials.card', ['selectedAgent' => $thread->agent, 'show_chat_button' => false])
                            @elseif (count($messages) === 0 && $thread->model)
                                <div class="w-full h-[70vh] flex flex-col justify-center">
                                    <div
                                            class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
                                        <p class="text-[16px] text-gray">Now speaking with...</p>
                                        <div class="max-w-[400px] border border-darkgray rounded p-4">
                                            @php
                                                $modelDetail = Models::MODELS[$thread->model];
                                                $userAccess = Models::getUserAccess($thread->model);
                                                $indicator = Models::isProModelSelected($thread->model) ? 'Pro' : 'Free';
                                            @endphp
                                            <img src="{{ Models::getModelPicture($thread->model) }}"
                                                alt="{{ $modelDetail['name'] }}"
                                                class="w-[100px] h-[100px] rounded-full object-cover">
                                            <div class="flex gap-4 items-center">
                                                <h3 class="mt-4">{{ $modelDetail['name'] }}</h3>
                                                <span
                                                        @if ($indicator == 'Free') class="bg-opacity-15 bg-white rounded-md px-2 py-1 text-green text-sm flex justify-center items-center w-[44px] h-[20px]"
                                                        @elseif($indicator == 'Pro')
                                                            class="bg-opacity-15 bg-white rounded-md px-1 py-1 text-gray-500 text-sm flex justify-center items-center w-[56px] h-[20px]" @endif>
                                                @if ($indicator == 'Pro')
                                                        <x-icon.logo class="w-[12px] h-[12px] mr-[4px]"/>
                                                    @endif
                                                    {{ $indicator }}
                                            </span>
                                            </div>
                                            <p class="text-[14px] text-gray mb-0">{{ $modelDetail['description'] }}</p>
                                            @if (!empty($modelDetailModel['capabilities']))
                                                <p class="text-[14px] text-gray mb-0">
                                                    {{ json_encode($modelDetail['capabilities']) }}</p>
                                            @endif
                                        </div>
                                    </div>
                                </div>
                            @endif
                        @endif

                        @php
                            $models = Models::MODELS;
                        @endphp

                        @foreach ($messages as $message)
                            @php
                                // If message has an agent_id, use agent name, otherwise use 'You'
                                // dd($message);
                                try {
                                    if (
                                        !empty($message["agent_id"]) &&
                                        !empty($message["input_tokens"]) &&
                                        !empty($message["output_tokens"])
                                    ) {
                                        if(!$message["agent"]){
                                            $author = 'Unknown Agent';
                                        }else{
                                            $author = $message["agent"]["name"]; // hacky but whatever
                                        }
                                    } elseif (
                                        !empty($message["model"]) &&
                                        empty($message["agent_id"]) &&
                                        is_numeric($message["output_tokens"])
                                    ) {
                                        $author = $models[$message["model"]]['name'] ?? 'Model';
                                    }
                                    else{
                                        $author = 'You';
                                    }
                                } catch (Exception $e) {
                                    $author = 'Unknown';
                                    dd($e);
                                }

                                $promptClass = $author === 'You' ? 'prompt' : '';
                            @endphp
                            <div class="pl-[50px]">
                                @php
                                    $image = null;
                                    if (isset($message["agent"])) {

                                        $image = $message["agent"]["image_url"];
                                    } elseif (isset($message["model"])) {
                                        $gateway = $models[$message["model"]]['gateway'];
                                        $image = asset('images/icons/' . $gateway . '.png');
                                    }

                                @endphp

                            </div>
                            <x-chat.message :author="$author" :message="$message['body']" :promptClass="$promptClass"
                                            :image="$image"
                                            ></x-chat.message>
                        @endforeach

                        @if ($pending)
                            @php
                                // If there's a selected agent, use agent name, otherwise use $models[$selectedModel]['name']
                                $author = null;
                                $image = null;
                                if($thread->agent){
                                    $image = $thread->agent->getImageUrlAttribute();
                                    $author = $thread->agent->name;
                                }else{
                                    $image = $thread->model? asset('images/icons/' . $models[$thread->model]['gateway'] . '.png') : null;
                                    $author = $models[$thread->model]['name'];
                                }
                            @endphp

                            <x-chat.messagestreaming :author="$author" :image="$image"
                                                     :thread="$thread->id">
                            </x-chat.messagestreaming>
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
            </div>
        </x-chatbox>
    </div>
    <div class="w-full lg:-ml-[25px] px-3">
        <div class="sm:w-[584px] lg:w-[768px] mx-auto">
            @if ($showNoMoreMessages)
            @else
                <form wire:submit.prevent="sendMessage">

                    <div class="sm:w-[584px] lg:w-[768px] mx-auto">
                        @if ($images)
                            <div class="absolute bottom-[90px] left-[20%] right-[20%] flex flex-wrap justify-center">
                                @foreach ($images as $image)
                                    <img src="{{ $image->temporaryUrl() }}" alt="Image to upload"
                                         class="w-[160px] h-[160px] object-cover border-2 border-darkgray m-2">
                                @endforeach
                            </div>
                        @endif
                    </div>

                    <x-chat.textarea id="message-input" minRows="1" default="Message OpenAgents..."
                                     :showIcon="true" iconName="send" min-rows="1" max-rows="12"
                                     wire:model="message_input"
                                     wireModel="message_input" :image-upload="auth()->check() && auth()->user()->isPro"
                                     wire:ignore dusk="message-input"
                                     onkeydown="if(event.keyCode === 13 && !event.shiftKey && window.innerWidth > 768) { event.preventDefault(); document.getElementById('send-message').click(); }"
                                     class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white"/>
                    <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
                </form>
                {{-- If selected agent, show agent usage component. Otherwise show messages-remaining--}}
                @if ($thread->agent)
                    <livewire:agent-usage :selectedAgent="$thread->agent"/>
                @else
                    <livewire:messages-remaining/>
                @endif
            @endif
        </div>
    </div>
</div>
