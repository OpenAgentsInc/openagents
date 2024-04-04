@props([
    'className' => '',
    'hasError' => false,
    'showIcon' => false,
    'iconName' => '',
    'minRows' => 1,
    'maxRows' => null,
    'default' => '', // Placeholder content
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
        console.log(newHeight);
        let maxHeight = this.viewportMaxHeight;

        if (this.maxRows !== null) {
            let maxRowsHeight = this.maxRows * this.lineHeight() + this.scrollbarWidth();
            maxHeight = Math.min(maxHeight, maxRowsHeight);
        }

        if (newHeight > maxHeight) {
            console.log('if - maxHeight', maxHeight);
            this.$refs.textarea.style.height = `${maxHeight}px`;
            this.$refs.textarea.style.overflowY = 'auto';
        } else {
            console.log('else - newHeight', newHeight);
            // If newHeight is less than 48px, set it to 48px
            newHeight = Math.max(newHeight, 48);
            console.log('else - newHeight', newHeight);

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
        spellcheck="false"
        @input="update(); inputVal = $event.target.value; isDisabled = inputVal.length === 0"
        placeholder="{{ $default }}"
        :rows="minRows"
            {{ $attributes->merge([
                'class' => "flex w-full rounded-md border-2 bg-transparent px-3 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white " . ($hasError ? 'border-red' : 'border-[#3D3E42]') . " $className transition-all duration-300 ease-in-out",
                'style' => "'resize: none; overflow: hidden;'"
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

<script>
    function autosizeTextarea() {
        return {
            height: 'auto',
            minRows: @js($minRows),
            maxRows: @js($maxRows),
            viewportMaxHeight: window.innerHeight * 0.4,
            init() {
                this.$nextTick(() => this.update());
            },
            update() {
                this.$refs.textarea.style.height = 'auto';
                let newHeight = this.$refs.textarea.scrollHeight;
                let maxHeight = this.viewportMaxHeight;

                if (this.maxRows !== null) {
                    // If maxRows is defined, calculate maxHeight based on line height and maxRows
                    let maxRowsHeight = this.maxRows * this.lineHeight() + this.scrollbarWidth();
                    maxHeight = Math.min(maxHeight, maxRowsHeight);
                }

                if (newHeight > maxHeight) {
                    this.$refs.textarea.style.height = `${maxHeight}px`;
                    this.$refs.textarea.style.overflowY = 'auto'; // Ensure scrollbar is shown when needed
                } else {
                    this.$refs.textarea.style.height = `${newHeight}px`;
                    this.$refs.textarea.style.overflowY = 'hidden'; // Hide scrollbar when content fits within constraints
                }
            },
            lineHeight() {
                return parseFloat(getComputedStyle(this.$refs.textarea).lineHeight);
            },
            scrollbarWidth() {
                // Calculate scrollbar width to adjust maxHeight if necessary
                return this.$refs.textarea.offsetWidth - this.$refs.textarea.clientWidth;
            }
        }
    }
</script>


