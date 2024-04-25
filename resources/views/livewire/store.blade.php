@php use App\AI\Models; @endphp
<div class="pb-24">
    <div class="px-4 py-2 flex flex-row justify-between">
        <a href="/" wire:navigate>
            <x-logomark size="2"/>
        </a>
        <x-login-buttons/>
    </div>

    <div class="w-[800px] mx-auto">
        <div class="text-center mt-[48px] px-[15px]">
            <h1>Who would you like to<br/>speak with today?</h1>

            <p class="text-gray">Discover, try, and create AI chat agents...<br/>
                on the world’s <span class="italic">openest</span> open AI platform.</p>
        </div>

        <h2 class="font-bold mt-12">Chat Agents</h2>
        <p class="text-gray">Chat Agents combine instructions, extra knowledge, and any LLM on OpenAgents to create
            custom AI agents.</p>

        <div class="grid grid-cols-2 gap-4 mt-4">
            @foreach($agents as $agent)
                <div class="p-4 rounded-lg relative">
                    <div class="flex">
                        <div class="w-[80px] h-[80px]">
                            <!-- Add agent icon or image here -->
                            <img src="https://placekitten.com/200/200" alt="Agent" class="w-full h-full rounded">

                        </div>
                        <div class="flex-1 pl-4">
                            <h4 class="text-lg font-bold">{{ $agent['title'] }}</h4>
                            <span class="text-gray">{{ $agent['description'] }}</span>
                        </div>
                    </div>
                </div>
            @endforeach
        </div>


        <h2 class="font-bold mt-12">LLMs</h2>
        <p class="text-gray">Large language models (LLMs) are general-purpose chat agents that can help with a wide
            variety
            of tasks.</p>

        <div class="grid grid-cols-2 gap-4 mt-4">
            @foreach($models as $modelKey => $modelDetails)
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
            @endforeach
        </div>

    </div>
</div>
