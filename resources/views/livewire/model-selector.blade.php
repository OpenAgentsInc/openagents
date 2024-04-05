<ol class="ms-3 flex items-center whitespace-nowrap" aria-label="Breadcrumb">
    <li class="text-sm font-semibold text-gray-800 truncate select-none" x-data="{ dropdown: false }"
        aria-current="page">
        <div class="flex gap-2 items-center justify-center overflow-hidden" role="button"
             tabindex="0" @click="dropdown= !dropdown">
            {{--            <x-icon.chatgpt class="w-[28px] h-[28px]"></x-icon.chatgpt>--}}

            <div class="flex flex-col">
                <span class="my-0 text-[18px]">{{ $this->getModelName() }}</span>
            </div>

            <div class="relative flex-1 text-right bg-black">
                <div>
                    <button class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100 focus:outline-none">
                        <x-icon.expand-down class="w-5 h-5"></x-icon.expand-down>
                    </button>
                    <div
                            x-cloak
                            x-show="dropdown" @click.away="dropdown= false"
                            class="mt-4 -ml-[125px] fixed z-[50] divide-y divide-white/15 min-w-60 shadow-md rounded-lg p-2 bg-black border border-white/25"
                            aria-labelledby="hs-dropdown-with-header">
                        <div class="py-0 first:pt-0 last:pb-0 bg-black">
                            <a wire:click="selectModel('mistral-large-latest')"
                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">
                                <div class="flex flex-col">
                                    <span class="text-indigo-50 my-0 text-sm">Mistral Large</span>
                                </div>
                            </a>
                            <a wire:click="selectModel('mixtral-8x7b-32768')"
                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">
                                <div class="flex flex-col">
                                    <span class="text-indigo-50 my-0 text-sm">Mixtral (Groq) </span>
                                </div>
                            </a>


                            {{--                            <a wire:click="selectModel('gpt-4')"--}}
                            {{--                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">--}}
                            {{--                                <x-icon.chatgpt class="w-[28px] h-[28px]"></x-icon.chatgpt>--}}
                            {{--                                <div class="flex flex-col">--}}
                            {{--                                    <span class="text-indigo-50 my-0 text-sm">GPT-4 </span>--}}
                            {{--                                </div>--}}
                            {{--                            </a>--}}
                            {{--                            <a wire:click="selectModel('claude')"--}}
                            {{--                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">--}}
                            {{--                                <x-icon.claude class="w-[28px] h-[28px]"></x-icon.claude>--}}
                            {{--                                <div class="flex flex-col">--}}
                            {{--                                    <span class="text-indigo-50 my-0 text-sm">Claude </span>--}}

                            {{--                                </div>--}}
                            {{--                            </a>--}}
                            {{--                            <a wire:click="selectModel('gemini')"--}}
                            {{--                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">--}}
                            {{--                                <x-icon.gemini class="w-[28px] h-[28px]"></x-icon.gemini>--}}
                            {{--                                <div class="flex flex-col">--}}
                            {{--                                    <span class="text-indigo-50 my-0 text-sm">Gemini </span>--}}

                            {{--                                </div>--}}
                            {{--                            </a>--}}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </li>
</ol>