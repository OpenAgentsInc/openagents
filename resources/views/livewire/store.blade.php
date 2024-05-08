<div class="w-full h-[70vh] flex flex-col justify-center">
    <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <x-logomark :size="1"></x-logomark>
        <h3 class="mt-[36px] text-center leading-relaxed">How can we help you today?</h3>

        @if (count($agents) > 0)
            <div class="border border-offblack p-8 mt-12 rounded">
                <div>
                    <h4 class="font-bold">Featured Agents</h4>
                    <p class="text-gray leading-relaxed">Chat Agents combine instructions, extra knowledge, and any LLM
                        on
                        OpenAgents to
                        create
                        custom AI agents.</p>
                </div>

                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-4">

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
            </div>
        @endif
    </div>
</div>
