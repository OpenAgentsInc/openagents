@props([
    'className' => '',
    'hasError' => false,
    'showIcon' => false,
    'iconName' => '',
    'minRows' => 1,
    'maxRows' => null,
    'default' => '',
    'wireModel' => '',
])

<div class="relative" x-data="{
    inputVal: @entangle($attributes->wire('model')),
    isDisabled: true,
    height: 'auto',
    minRows: @js($minRows),
    maxRows: @js($maxRows),
    viewportMaxHeight: window.innerHeight * 0.95,
    update() {
        this.$refs.textarea.style.height = 'auto';
        let newHeight = this.$refs.textarea.scrollHeight;
        let maxHeight = this.viewportMaxHeight;

        if (this.maxRows !== null) {
            let maxRowsHeight = this.maxRows * this.lineHeight() + this.scrollbarWidth();
            maxHeight = Math.min(maxHeight, maxRowsHeight);
        }

        if (newHeight > maxHeight) {
            this.$refs.textarea.style.height = `${maxHeight}px`;
            this.$refs.textarea.style.overflowY = 'auto';
            console.log('1- Setting height to ', maxHeight);
        } else {
            newHeight = Math.max(newHeight, 48);
            this.$refs.textarea.style.height = `${newHeight}px`;
            this.$refs.textarea.style.overflowY = 'hidden';
            console.log('2- Setting height to ', newHeight);
        }
    },

        update2() {
            this.$refs.textarea.style.height = 'auto';
            let newHeight = this.$refs.textarea.scrollHeight;
            let maxHeight = this.viewportMaxHeight;

            if (this.maxRows !== null) {
                let maxRowsHeight = this.maxRows * this.lineHeight() + this.scrollbarWidth();
                maxHeight = Math.min(maxHeight, maxRowsHeight);
            }

            if (newHeight > maxHeight) {
                this.$refs.textarea.style.height = `${maxHeight}px`;
                this.$refs.textarea.style.overflowY = 'auto';
            } else if (this.minRows === 1) {
                this.$refs.textarea.style.height = '48px';
                this.$refs.textarea.style.overflowY = 'hidden';
            } else if (newHeight <= this.minRows * this.lineHeight()) {
                this.$refs.textarea.style.height = `${Math.max(48, this.minRows * this.lineHeight())}px`;
                this.$refs.textarea.style.overflowY = 'hidden';
            } else {
                let textareaStyles = getComputedStyle(this.$refs.textarea);
                let padding = parseFloat(textareaStyles.paddingTop) + parseFloat(textareaStyles.paddingBottom);
                let border = parseFloat(textareaStyles.borderTopWidth) + parseFloat(textareaStyles.borderBottomWidth);
                let contentHeight = newHeight - padding - border;
                this.$refs.textarea.style.height = `${Math.max(48, contentHeight)}px`;
                this.$refs.textarea.style.overflowY = 'hidden';
            }
        },
    lineHeight() {
        // return 48;
        return parseFloat(getComputedStyle(this.$refs.textarea).lineHeight);
    },
    scrollbarWidth() {
        return this.$refs.textarea.offsetWidth - this.$refs.textarea.clientWidth;
    },
    resetHeight() {
        this.$refs.textarea.style.height = '48px';
        this.$refs.textarea.style.overflowY = 'hidden';
        this.isDisabled = true;
    }
}" x-init="update();" wire:key="{{ $wireModel }}"
>
<textarea
        x-ref="textarea"
        autocomplete="off"
        wire:ignore
        @input="update(); isDisabled = inputVal.length === 0"
        placeholder="{{ $default }}"
        x-model="inputVal"
        x-effect="if (inputVal === '') resetHeight()"
        {{ $attributes->merge([
            'class' => "resize-none flex w-full rounded-md border-2 bg-transparent px-3 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white " . ($hasError ? 'border-red' : 'border-[#3D3E42]') . " $className transition-all duration-300 ease-in-out",
        ]) }}
    ></textarea>

    @if($showIcon && $iconName)
        <button
                :disabled="isDisabled"
                class="absolute bottom-[10px] right-[10px] text-black shadow rounded"
                :class="isDisabled ? 'bg-gray' : 'bg-white hover:bg-white/90'"
        >
            {{-- Assuming <x-icon> is a component you have for rendering icons --}}
            <x-icon :name="$iconName" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
        </button>
    @endif
</div>
