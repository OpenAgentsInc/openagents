<div class="flex gap-1 items-center justify-center overflow-hidden" role="button" tabindex="0"
     @click="dropdown = !dropdown">
    <div class="flex flex-row gap-3 items-center select-none">
        <img src="{{ $picture }}" class="w-6 h-6">
        <span class="my-0 text-[18px]">{{ $formattedModelOrAgent }}</span>
    </div>

    <div class="relative flex-1 text-right bg-black">
        <button class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100 focus:outline-none">
            <x-icon.expand-down class="w-5 h-5"></x-icon.expand-down>
        </button>
        <div x-cloak x-show="dropdown" @click.away="dropdown = false"
             class="mt-3 -ml-[125px] fixed z-[50] divide-y divide-white/15 min-w-60 shadow-md rounded-lg p-2 bg-black border border-white/25 overflow-y-scroll sm:overflow-x-hidden max-h-80 sm:max-h-auto"
             aria-labelledby="hs-dropdown-with-header">
            <div class="py-0 first:pt-0 last:pb-0 bg-black">
                @if($showAgents)
                    <div role="button" x-on:click="Livewire.dispatch('openModal', { component: 'modals.explore-agents' })"
                       class="py-2 w-full mb-2 border-b border-offblack flex items-center gap-x-3.5 px-3 rounded-lg text-sm focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">
                        <x-icon.boxes class="w-5 h-5"></x-icon.boxes>
                        <div class="flex flex-col">
                            <span class="text-indigo-50 my-0 text-sm">Explore agents</span>
                        </div>
                    </div>
                @endif
                @foreach($models as $modelKey => $modelName)
                    @php
                        $userAccess = $this->getUserAccess();
                        $indicator = $this->getModelIndicator($modelKey, $userAccess);
                        $disabled = $indicator ? 'opacity-25' : '';
                        $gateway = $modelName['gateway'];
                        $imagePath = 'images/icons/' . $gateway . '.png';
                    @endphp
                    <a wire:click="selectModel('{{ $modelKey }}')"
                       class="flex items-center gap-x-3.5 py-1 px-3 rounded-lg text-sm focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15 {{ $disabled }}">
                        <img src="{{ asset($imagePath) }}" alt="{{ $gateway }}" class="w-6 h-6">
                        <div class="flex flex-col">
                            <span class="text-indigo-50 my-0 text-sm">{{ $modelName["name"] }}</span>
                        </div>
                        @if($indicator)
                            <span class="ml-auto text-gray-500">{{ $indicator }}</span>
                        @endif
                    </a>
                @endforeach
            </div>
        </div>
    </div>
</div>
