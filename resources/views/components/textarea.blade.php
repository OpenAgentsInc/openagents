@props([
    'className' => '',
    'hasError' => false,
    'iconName' => '',
    'minRows' => 1,
    'maxRows' => null,
    'placeholder' => '',
])

<div class="relative" x-data="{
    inputVal: '',
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
        } else {
            newHeight = Math.max(newHeight, 100);
            this.$refs.textarea.style.height = `${newHeight}px`;
            this.$refs.textarea.style.overflowY = 'hidden';
        }
    },
    lineHeight() {
        return parseFloat(getComputedStyle(this.$refs.textarea).lineHeight);
    },
    scrollbarWidth() {
        return this.$refs.textarea.offsetWidth - this.$refs.textarea.clientWidth;
    }
}" x-init="update()"
>
<textarea
        x-ref="textarea"
        autocomplete="off"
        @input="update(); inputVal = $event.target.value; isDisabled = inputVal.length === 0"
        x-init="setTimeout(() => update(), 10)"
        placeholder="{{ $placeholder }}"
        :rows="minRows"
            {{ $attributes->merge([
                'class' => "resize-none flex w-full rounded-md border-2 bg-transparent px-3 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white " . ($hasError ? 'border-red' : 'border-[#3D3E42]') . " $className transition-all duration-300 ease-in-out",
            ]) }}
    ></textarea>
</div>
