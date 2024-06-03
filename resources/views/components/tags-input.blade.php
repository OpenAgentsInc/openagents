@props(['name' => 'tags', 'tags' => []])

{{--

<div x-data="tagsComponent()" class="flex flex-col">
    <input
        x-model="newTag"
        type="text"
        placeholder="Add a new tag..."
        class="px-4 py-2 my-2 border border-darkgray bg-black text-white rounded-md focus:outline-none focus:ring-2 focus:ring-white focus:border-white"
        @keyup="enterTag($event)"
        @blur="addTag(newTag)"
        @keydown.enter.stop
        @keyup.enter.stop
        @keydown.enter.prevent
        @keyup.enter.prevent
    >
    <div class="flex flex-wrap gap-2 mt-2">
        <template x-for="tag in tags" :key="tag">
            <div role="button" class="flex items-center px-3 py-1 rounded-md border border-[#cbcbcb] text-white hover:text-black hover:bg-white">
                <span x-text="tag"></span>
                <button
                    @click="removeTag(tag)"
                    @keydown.enter.stop
                    @keyup.enter.stop
                    @keydown.enter.prevent
                    @keyup.enter.prevent
                    tabindex="-1"
                    class="ml-2  hover:text-black focus:outline-none"
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                    </svg>
                </button>
            </div>
        </template>
    </div>
</div>

<script>
function tagsComponent() {
    return {
        newTag: '',
        tags: @json($tags),
        addTag(newTag) {
            newTag=newTag.trim();
            if (newTag !== '') {
                if(!this.tags.includes(newTag)){
                    this.tags.push(newTag);
                }
                this.newTag = '';
            }
        },
        removeTag(tag) {
            this.tags = this.tags.filter(t => t !== tag);
        },
        enterTag(event) {
            if (event.key === ',' || event.code === 'Comma' || event.code === 'Enter') {
                if(event.key === ',' || event.code === 'Comma' ){
                    this.addTag(this.newTag.slice(0, -1));
                }else{
                    this.addTag(this.newTag);
                }
                event.preventDefault();
                event.stopPropagation();
            }
        }
    }
}
</script> --}}

<div x-data="{
    inputs: @entangle( $attributes->whereStartsWith('wire:model')->first()),
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
            this.inputs = this.choices.getValue(true);
            data = this.inputs;
            tags = '{!! $attributes->whereStartsWith('wire:model')->first() !!}';
            @this.set(tags, data);
        });
    },
    addTag() {
        if (this.input.trim() !== '') {
            this.choices.setValue([this.input.trim()], 'value');
            this.input = '';

        }
    },
    removeTag(tag) {
        const index = this.inputs.indexOf(tag);
        if (index !== -1) {
            this.inputs.splice(index, 1);
            this.choices.removeActiveItemsByValue(tag);
            data = this.inputs;
            tags = '{!! $attributes->whereStartsWith('wire:model')->first() !!}';
            @this.set(tags, data);
        }
    }
}" x-init="initChoices()">

    <div wire:ignore>
        <input type="text" id="tag-input" x-model="input" x-on:keydown.enter.prevent="addTag()" {{ $attributes }}
            class="px-4 py-2 my-2 border border-darkgray bg-black text-white rounded-md focus:outline-none focus:ring-2 focus:ring-white focus:border-white">
        <div>
            <div class="flex flex-wrap gap-2 mt-2">
                <template x-for="tag in inputs" :key="tag">
                    {{-- <span x-text="tag" class="tag" style="margin-right: 5px;"></span> --}}
                    <div role="button"
                        class="flex items-center px-3 py-1 rounded-md border border-[#cbcbcb] text-white hover:text-black hover:bg-white">
                        <span x-text="tag"></span>
                        <button @click="removeTag(tag)" @keydown.enter.stop @keyup.enter.stop @keydown.enter.prevent
                            @keyup.enter.prevent tabindex="-1" class="ml-2  hover:text-black focus:outline-none">
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"
                                xmlns="http://www.w3.org/2000/svg">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                    d="M6 18L18 6M6 6l12 12"></path>
                            </svg>
                        </button>
                    </div>
                </template>
            </div>
        </div>
    </div>
</div>
