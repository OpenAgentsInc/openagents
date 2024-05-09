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
                        <div class="mt-2 flex flex-row items-center">
                            <x-login-buttons/>
                        </div>
                    @endauth
                </div>
                <div class="xl:-ml-[50px] pt-8 chat">
                    @if(!$hasSelection)
                        <livewire:store/>
                    @elseif (count($messages) === 0 && $selectedAgent)
                        <div class="w-full h-[70vh] flex flex-col justify-center">
                            <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
                                <p class="text-[16px] text-gray">Now speaking with...</p>

                                <div class="max-w-[400px] border border-darkgray rounded p-4">
                                    <img src="{{ $selectedAgent['image'] }}" alt="{{ $selectedAgent['name'] }}"
                                         class="w-[100px] h-[100px] rounded-full object-cover">
                                    <h3 class="mt-4">{{ $selectedAgent['name'] }}</h3>
                                    <p class="text-[14px] text-gray mb-0">{{ $selectedAgent['description'] }}</p>
                                    @if (!empty($selectedAgent['capabilities']))
                                        <p class="text-[14px] text-gray mb-0">
                                            {{ json_encode($selectedAgent['capabilities']) }}</p>
                                    @endif
                                </div>
                            </div>
                        </div>
                    @elseif (count($messages) === 0 && $selectedModel)
                        <div class="w-full h-[70vh] flex flex-col justify-center">
                            <div
                                    class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
                                <p class="text-[16px] text-gray">Now speaking with...</p>

                                <div class="max-w-[400px] border border-darkgray rounded p-4">
                                    @php
                                        $modelDetail = Models::MODELS[$selectedModel];
                                        $userAccess = Models::getUserAccess($selectedModel);
                                        $indicator = Models::isProModelSelected($selectedModel) ? 'Pro' : 'Free';

                                    @endphp
                                    <img src="{{ Models::getModelPicture($selectedModel) }}"
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
                    @elseif (count($messages) === 0)
                        <livewire:store/>

                    @endif


                    @php
                        $models = Models::MODELS;
                    @endphp

                    @foreach ($messages as $message)
                        @php
                            // If message has an agent_id, use agent name, otherwise use 'You'
                            try {
                                if (!empty($message['agent_id'])) {
                                    $author = $message['agent']['name'] ?? 'You'; // hacky but whatever
                                } elseif (!empty($message['model']) && $message['model'] === null) {
                                    $author = 'You';
                                } else {
                                    $author = $models[$message['model']]['name'] ?? 'You'; // ?
                                }
                            } catch (Exception $e) {
                                $author = 'You';
                            }

                            $promptClass = $author === 'You' ? 'prompt' : '';
                        @endphp
                        <div class="pl-[50px]">
                            @php
                                // If $message['agent'] is set, dump the agent's image URL
$image = null;
$model_image = null;
if (isset($message['agent'])) {
    $agent = $message['agent'];
    if (isset($agent['image_url'])) {
        $image = $agent['image_url'];
    } elseif (isset($agent['image'])) {
        $image = $agent['image'];
    }
} elseif (isset($message['model'])) {
    // Use the model image
    // First get the gateway
    $gateway = $models[$message['model']]['gateway'];
    $model_image = asset('images/icons/' . $gateway . '.png');
                                }

                            @endphp

                        </div>
                        <x-chat.message :author="$author" :message="$message['body']" :promptClass="$promptClass"
                                        :agent-image="$image"
                                        :model-image="$model_image"></x-chat.message>
                    @endforeach

                    @if ($pending)
                        @php
                            // If there's a selected agent, use agent name, otherwise use $models[$selectedModel]['name']
$author = $selectedAgent ? $selectedAgent['name'] : $models[$selectedModel]['name'];
$image = $selectedAgent ? $selectedAgent['image'] : null;

$model_image = $selectedModel
    ? asset('images/icons/' . $models[$selectedModel]['gateway'] . '.png')
                                : null;
                        @endphp

                        <x-chat.messagestreaming :author="$author" :agent-image="$image" :model-image="$model_image"
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
                                     onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                                     class="flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white"/>
                    <button dusk="send-message" class="hidden" id="send-message" type="submit"></button>
                </form>
                <livewire:messages-remaining/>
            @endif
        </div>
    </div>
</div>
