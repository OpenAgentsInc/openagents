@props(['name' => '', 'tags' => []])

{{-- {{dd($attributes->get('wire:model'))}} --}}

<div x-data="{
    inputs: @entangle($attributes->whereStartsWith('wire:model')->first()),
    input: '',
    choices: null,
    initChoices() {
        this.choices = new Choices('#tag-input', {
            delimiter: ',',
            editItems: true,
            removeItemButton: true,
            duplicateItemsAllowed: false,
            paste: true,
            placeholder: true,
            placeholderValue: 'Add tags',
        });



        this.choices.passedElement.element.addEventListener('change', () => {
            {{-- this.inputs = this.choices.getValue(true); --}}
            {{-- this.inputs.push(this.choices.getValue(true)); --}}
            {{-- datavalue = '{{ $attributes->whereStartsWith('wire:model')->first() }}';
            console.log(datavalue);
            @this.set( datavalue, this.inputs); --}}
            if (this.input.trim() !== '') {
                this.inputs.push(this.choices.setValue([this.input.trim()], 'value'));
                this.input = '';
            }
            {{-- if (this.input.trim() == '') {
                this.inputs = this.choices.getValue(true);
            } --}}
        });

        this.choices.passedElement.element.addEventListener('addItem', (event) => {
            this.inputs.push(event.detail.value);
            {{-- $wire.set('tags', this.tags); --}}
        });
    },
    addTag() {
        if (this.input.trim() !== '') {
            {{-- this.choices.setValue([this.input.trim()], 'value'); --}}
            this.inputs.push(this.choices.setValue([this.input.trim()], 'value'));
            this.input = '';

        }
    },
    removeTag(tag) {
        const index = this.inputs.indexOf(tag);
        if (index !== -1) {
            this.inputs.splice(index, 1);
            this.choices.removeActiveItemsByValue(tag);
            {{-- tags = '{!! $attributes->whereStartsWith('wire:model')->first() !!}';
            @this.set(tags, this.inputs); --}}
        }
    }
}" x-init="initChoices()">

    <div wire:ignore>
        <input type="text" id="tag-input" x-model="input" x-on:keydown.enter.prevent="addTag()" {{ $attributes }}
            class="px-4 py-2 my-2 border border-darkgray bg-black text-white rounded-md focus:outline-none focus:ring-2 focus:ring-white focus:border-white">
        <div>
            <div class="flex flex-wrap gap-2 mt-2">
                <template x-for="(tag, index) in inputs" :key="`${index}-${tag}`">
                    <div role="button" class="flex items-center px-3 py-1 rounded-md border border-[#cbcbcb] text-white hover:text-black hover:bg-white">
                        <span x-text="tag"></span>
                        <button @click="removeTag(tag)" @keydown.enter.stop @keyup.enter.stop @keydown.enter.prevent @keyup.enter.prevent tabindex="-1" class="ml-2 hover:text-black focus:outline-none">
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                            </svg>
                        </button>
                    </div>
                </template>
            </div>
        </div>
    </div>
</div>
