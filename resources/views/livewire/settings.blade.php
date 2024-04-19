<div class="p-12 mx-auto flex flex-col justify-center w-full items-center">
    <div class="max-w-3xl min-w-[600px]">
        <x-pane title="Default model">
            <div x-data="{ dropdown: false }" class="relative">
                <div class="flex gap-1 items-center justify-center overflow-hidden cursor-pointer select-none"
                     role="button"
                     tabindex="0"
                     @click="dropdown = !dropdown">
                    <div class="flex flex-col">
                        <span class="my-0 text-[18px]">{{ $this->formattedDefaultModel }}</span>
                    </div>

                    <div class="relative flex-1 text-right">
                        <button class="p-1.5 rounded-md text-gray-500 hover:bg-gray-100 active:bg-gray-200 focus:outline-none">
                            <x-icon.expand-down class="w-5 h-5"></x-icon.expand-down>
                        </button>
                    </div>
                </div>

                <div x-cloak x-show="dropdown" @click.away="dropdown = false"
                     class="absolute z-10 mt-2 w-full bg-black rounded-lg shadow-lg border border-darkgray">
                    <div class="py-1">
                        @foreach($models as $modelKey => $modelName)
                            @php
                                $userAccess = $this->getUserAccess();
                                $indicator = $this->getModelIndicator($modelKey, $userAccess);
                                $disabled = $indicator ? 'opacity-25' : '';
                                $gateway = $modelName['gateway'];
                                $imagePath = 'images/icons/' . $gateway . '.png';
                            @endphp
                            <a wire:click="$action('{{ $modelKey }}')"
                               class="flex items-center gap-x-3.5 py-1 px-3 rounded-lg text-sm focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15 {{ $disabled }} cursor-pointer">
                                <div class="flex items-center gap-x-3.5">
                                    <img src="{{ asset('images/icons/' . $modelName['gateway'] . '.png') }}"
                                         alt="{{ $modelName['gateway'] }}" class="w-6 h-6">
                                    <span>{{ $modelName["name"] }}</span>
                                </div>
                            </a>
                        @endforeach
                    </div>
                </div>
            </div>
        </x-pane>
    </div>
</div>
