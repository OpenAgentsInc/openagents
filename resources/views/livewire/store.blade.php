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

        <!-- grid of two columns of cards -->
        <div class="grid grid-cols-2 gap-4 mt-4">
            @foreach($models as $modelKey => $modelDetails)
                <div class="bg-darkgray p-4 rounded-lg">
                    <h3 class="text-lg font-bold">{{ $modelDetails['name'] }}</h3>
                    <p class="text-gray">Gateway: {{ $modelDetails['gateway'] }}</p>
                    <p class="text-gray">Access: {{ $modelDetails['access'] }}</p>
                    <p class="text-gray">Max Tokens: {{ $modelDetails['max_tokens'] }}</p>
                    @php
                        $userAccess = Models::getUserAccess();
                        $indicator = Models::getModelIndicator($modelKey, $userAccess);
                    @endphp
                    @if($indicator)
                        <span class="ml-auto text-gray-500">{{ $indicator }}</span>
                    @endif
                </div>
            @endforeach
        </div>

    </div>
</div>