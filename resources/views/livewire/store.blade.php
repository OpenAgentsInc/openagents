@php use App\AI\Models; @endphp
<div>
    <div class="px-4 py-2 flex flex-row justify-between">
        <x-logomark size="2"/>
        <x-login-buttons/>
    </div>

    <div class="w-[768px] mx-auto">
        <div class="text-center mt-[64px] p-[15px]">
            <h1>Who would you like to<br/>speak with today?</h1>

            <p class="text-gray">Discover, try, and create AI chat agents...<br/>
                on the world’s <span class="italic">openest</span> open AI platform.</p>
        </div>

        <h2 class="font-bold">LLMs</h2>
        <p class="text-gray">Large language models (LLMs) are general-purpose chat agents that can help with a wide
            variety
            of tasks.</p>

        <div class="grid grid-cols-2 gap-4 mt-4">
            @foreach($models as $modelKey => $modelDetails)
                <div class="p-4 rounded-lg relative">
                    <div class="absolute top-2 right-2">
                        @php
                            $userAccess = Models::getUserAccess();
                            $indicator = Models::getModelIndicator($modelKey, $userAccess);
                        @endphp
                        @if($indicator)
                            <span class="text-gray-500">{{ $indicator }}</span>
                        @endif
                    </div>
                    <div class="flex">
                        <div class="w-[80px] h-[80px]">
                            <img src="{{ asset('images/icons/' . $modelDetails['gateway'] . '.png') }}"
                                 alt="{{ $modelDetails['gateway'] }}" class="w-full h-full">
                        </div>
                        <div class="flex-1 pl-4">
                            <h3 class="text-lg font-bold">{{ $modelDetails['name'] }}</h3>
                            <span class="text-gray">Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.</span>
                        </div>
                    </div>
                </div>
            @endforeach
        </div>

    </div>
</div>
