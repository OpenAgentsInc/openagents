@props([
    'data' => [],
    'placeholder' => 'Select an option',
    'limit' => 1000,
])

<div x-data="AlpineSelect({
    data: {{ json_encode($data) }},
    selected: @entangle($attributes->wire('model')),
    placeholder: '{{ $placeholder }}',
    multiple: {{ isset($attributes['multiple']) ? 'true' : 'false' }},
    disabled: {{ isset($attributes['disabled']) ? 'true' : 'false' }},
    limit: {{ $limit }},
})" x-init="init()" x-on:click.away="closeSelect()" @keydown.escape="closeSelect()"
    @keydown.arrow-down.prevent="increaseIndex()" @keydown.arrow-up.prevent="decreaseIndex()" {{-- @keydown.enter="selectOption(Object.keys(options)[currentIndex])" --}}
    class="mt-1">

    <div class="relative content-center w-full p-1 text-left  border rounded-md sm:text-sm sm:leading-5"
        x-bind:class="{ 'border-gray ring ring-lightgray ring-opacity-50': open, 'bg-gray-200 cursor-default': disabled }"
        x-on:click.prevent="toggleSelect()">
        <div id="placeholder">
            <div class="inline-block m-1" x-show="selected.length === 0" x-text="placeholder">&nbsp;</div>
        </div>
        @isset($attributes['multiple'])
            <div class="flex flex-wrap space-x-1" x-cloak x-show="selected.length > 0">
                <template x-for="(key, index) in selected" :key="index">
                    <div class="text-gray-800 rounded-full truncate bg-gray px-2 py-0.5 my-0.5 flex flex-row items-center">
                        <div class="px-2 truncate" x-text="data[key]"></div>
                        <div x-show="!disabled" x-bind:class="{ 'cursor-pointer': !disabled }" class="w-4"
                            x-on:click.prevent.stop="deselectOption(index)"><svg xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24" class = 'h-4 fill-current'>
                                <path
                                    d="M12 2c5.514 0 10 4.486 10 10s-4.486 10-10 10-10-4.486-10-10 4.486-10 10-10zm0-2c-6.627 0-12 5.373-12 12s5.373 12 12 12 12-5.373 12-12-5.373-12-12-12zm6 16.538l-4.592-4.548 4.546-4.587-1.416-1.403-4.545 4.589-4.588-4.543-1.405 1.405 4.593 4.552-4.547 4.592 1.405 1.405 4.555-4.596 4.591 4.55 1.403-1.416z" />
                            </svg></div>
                    </div>
                </template>
            </div>
        @else
            <div class="flex flex-wrap" x-cloak x-show="selected">
                <div class="text-gray-800 rounded-full truncate bg-gray px-2 py-0.5 my-0.5 flex flex-row items-center">
                    <div class="px-2 truncate" x-text="data[selected]"></div>
                    <div x-show="!disabled" x-bind:class="{ 'cursor-pointer': !disabled }" class="h-4"
                        x-on:click.prevent.stop="deselectOption()"><svg xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24" class = 'h-4 fill-current'>
                            <path
                                d="M12 2c5.514 0 10 4.486 10 10s-4.486 10-10 10-10-4.486-10-10 4.486-10 10-10zm0-2c-6.627 0-12 5.373-12 12s5.373 12 12 12 12-5.373 12-12-5.373-12-12-12zm6 16.538l-4.592-4.548 4.546-4.587-1.416-1.403-4.545 4.589-4.588-4.543-1.405 1.405 4.593 4.552-4.547 4.592 1.405 1.405 4.555-4.596 4.591 4.55 1.403-1.416z" />
                        </svg></div>
                </div>
            </div>
            @endif

            <div class="mt-0.5 w-full bg-darkgray border-gray rounded-b-md border absolute top-full left-0 z-30"
                x-show="open" x-cloak>

                <div class="relative z-30 w-full p-2 bg-black">
                    {{-- <input type="search" x-model="search" x-on:click.prevent.stop="open=true" class="block w-full p-2 border border-gray rounded-md focus:border-gray focus:ring focus:ring-lightgray focus:ring-opacity-50 sm:text-sm sm:leading-5"> --}}
                    <x-input type="search" x-model="search" x-on:click.prevent.stop="open=true"
                        class="block w-full p-2 border border-gray rounded-md focus:border-gray focus:ring focus:ring-lightgray focus:ring-opacity-50 sm:text-sm sm:leading-5" />
                </div>

                <div x-ref="dropdown" class="relative z-30 p-2 overflow-y-auto max-h-60">
                    <div x-cloak x-show="Object.keys(options).length === 0" x-text="emptyOptionsMessage">Gragr</div>
                    <template x-for="(key, index) in Object.keys(options)" :key="index">
                        @isset($attributes['multiple'])
                            <div class="px-2 py-1"
                                x-bind:class="{
                                    'bg-gray text-white hover:none': selected.includes(
                                        key),
                                    'hover:bg-white hover:text-black cursor-pointer': !(selected.includes(key)),
                                    {{-- 'bg-blue-500 text-white': currentIndex == index --}}
                                }"
                                x-on:click.prevent.stop="selectOption(key)" x-text="Object.values(options)[index]">
                            </div>
                        @else
                            <div class="px-2 py-1"
                                x-bind:class="{
                                    'bg-gray text-white hover:none': selected ==
                                        key,
                                    'hover:bg-white hover:text-black cursor-pointer': !(selected ==
                                        key),
                                    {{-- 'bg-blue-500 text-white': currentIndex == index --}}
                                }"
                                x-on:click.prevent.stop="selectOption(key)" x-text="Object.values(options)[index]">
                            </div>
                        @endisset
                    </template>
                </div>
            </div>
        </div>
    </div>

    @once
        <script>
            function AlpineSelect(config) {
                return {
                    data: config.data ?? [],
                    open: false,
                    search: '',
                    options: {},
                    emptyOptionsMessage: 'No results match your search.',
                    placeholder: config.placeholder,
                    selected: config.selected,
                    multiple: config.multiple,
                    currentIndex: 0,
                    isLoading: false,
                    disabled: config.disabled ?? false,
                    limit: config.limit ?? 40,

                    init: function() {
                        if (this.selected == null) {
                            if (this.multiple)
                                this.selected = []
                            else
                                this.selected = ''
                        }
                        if (!this.data) this.data = {}


                        this.resetOptions()

                        this.$watch('search', ((values) => {
                            if (!this.open || !values) {
                                this.resetOptions()
                                return
                            }

                            // setTimeout(() => {
                            //     $wire.call('search', e.detail.value);
                            // }, 300);

                            this.options = Object.keys(this.data)
                                .filter((key) => this.data[key].toLowerCase().includes(values.toLowerCase()))
                                .slice(0, this.limit)
                                .reduce((options, key) => {
                                    options[key] = this.data[key]
                                    return options
                                }, {})


                            this.currentIndex = 0
                        }))

                    },

                    resetOptions: function() {
                        this.options = Object.keys(this.data)
                            .slice(0, this.limit)
                            .reduce((options, key) => {
                                options[key] = this.data[key]
                                return options
                            }, {})
                    },

                    closeSelect: function() {
                        this.open = false
                        this.search = ''
                    },

                    toggleSelect: function() {
                        if (!this.disabled) {
                            if (this.open) return this.closeSelect()

                            this.open = true
                        }
                    },

                    deselectOption: function(index) {
                        if (this.multiple) {
                            this.selected.splice(index, 1)
                        } else {
                            this.selected = ''
                        }

                    },



                    selectOption: function(value) {
                        if (!this.disabled) {
                            // If multiple push to the array, if not, keep that value and close menu
                            if (this.multiple) {
                                // If it's not already in there
                                if (!this.selected.includes(value)) {
                                    this.selected.push(value)
                                }
                            } else {
                                this.selected = value
                                this.closeSelect()
                            }
                        }
                    },

                    increaseIndex: function() {
                        if (this.currentIndex == Object.keys(this.options).length)
                            this.currentIndex = 0
                        else
                            this.currentIndex++
                    },

                    decreaseIndex: function() {
                        if (this.currentIndex == 0)
                            this.currentIndex = Object.keys(this.options).length - 1
                        else
                            this.currentIndex--;
                    },
                }
            }
        </script>

    @endonce
