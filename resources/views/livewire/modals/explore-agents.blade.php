<div>
    {{-- The Master doesn't talk, he acts. --}}
    @php use App\AI\Models; @endphp
    <div class="max-h-[600px] lg:max-h-900 max-w-[900px]:  overflow-y-scroll">
        <div class=" mx-auto ">
            <div class="text-center mt-[48px] px-[15px]">
                <h1>How can we help you today?</h1>

                <p class="text-gray">Discover, try, and create AI chat agents...<br/>
                    on the world’s <span class="italic">openest</span> open AI platform.</p>
            </div>

            <h2 class="font-bold mt-12">Chat Agents</h2>
            <p class="text-gray">Chat Agents combine instructions, extra knowledge, and any LLM on OpenAgents to create
                custom AI agents.</p>

            <div class="grid grid-cols-2 gap-4 mt-4">
                @foreach($agents as $agent)
                    <a href="/chat?agent={{ $agent["id"] }}" wire:navigate>
                        <div class="p-4 rounded-lg relative">
                            <div class="flex">
                                <div class="w-[80px] h-[80px]">
                                    <!-- Add agent icon or image here -->
                                    <img src="{{ $agent->image_url }}" alt="Agent" class="w-full h-full rounded">

                                </div>
                                <div class="flex-1 pl-4">
                                    <h4 class="text-lg font-bold">{{ $agent['name'] }}</h4>
                                    <span class="text-gray">{{ $agent['about'] }}</span>
                                </div>
                            </div>
                        </div>
                    </a>
                @endforeach
            </div>


            <h2 class="font-bold mt-12">LLMs</h2>
            <p class="text-gray">Large language models (LLMs) are general-purpose chat agents that can help with a wide
                variety
                of tasks.</p>

            <div class="grid grid-cols-2 gap-4 mt-4">
                @foreach($models as $modelKey => $modelDetails)
                    <a href="/chat?model={{ $modelKey }}" wire:navigate>
                        <div class="p-4 rounded-lg relative">
                            <div class="absolute top-[18px] right-4">
                                @php
                                    $userAccess = Models::getUserAccess();
                                    $indicator = Models::isProModelSelected($modelKey) ? 'Pro' : 'Free';
                                @endphp
                                <span
                                        @if($indicator == 'Free')
                                            class="bg-opacity-15 bg-white rounded-md px-2 py-1 text-green text-sm flex justify-center items-center w-[44px] h-[20px]"
                                        @elseif($indicator == 'Pro')
                                            class="bg-opacity-15 bg-white rounded-md px-1 py-1 text-gray-500 text-sm flex justify-center items-center w-[56px] h-[20px]"
                                @endif
                            >
                                @if($indicator == 'Pro')
                                        <x-icon.logo class="w-[12px] h-[12px] mr-[4px]"/>
                                    @endif
                                    {{ $indicator }}
                            </span>
                            </div>
                            <div class="flex">
                                <div class="w-[80px] h-[80px]">
                                    <img src="{{ asset('images/icons/' . $modelDetails['gateway'] . '.png') }}"
                                        alt="{{ $modelDetails['gateway'] }}" class="w-full h-full">
                                </div>
                                <div class="flex-1 pl-4">
                                    <h4 class="text-lg font-bold">{{ $modelDetails['name'] }}</h4>
                                    <span class="text-gray">Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.</span>
                                </div>
                            </div>
                        </div>
                    </a>
                @endforeach
            </div>

        </div>
    </div>
</div>
