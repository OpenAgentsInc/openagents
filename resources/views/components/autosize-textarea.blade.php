@props(['minRows' => 1, 'maxRows' => null, 'default' => ''])

<div x-data="autosizeTextarea()" x-init="init">
    <textarea
            x-ref="textarea"
            @input="update"
            :style="'resize: none; overflow: hidden; height: ' + height + 'px; lineHeight: 1; fontSize: 10px; box-sizing: border-box;'"
            :rows="minRows"
            placeholder="{{ $default }}"
            {{ $attributes->merge([
                'class' => "transition-all duration-300 ease-in-out"
            ]) }}
    ></textarea>
</div>

<script>
    function autosizeTextarea() {
        return {
            height: 'auto',
            minRows: @js($minRows),
            maxRows: @js($maxRows),
            init() {
                this.$nextTick(() => this.update());
            },
            update() {
                this.$refs.textarea.style.height = 'auto';
                let newHeight = this.$refs.textarea.scrollHeight;
                if (this.maxRows !== null) {
                    let maxHeight = this.maxRows * this.lineHeight() + this.scrollbarWidth();
                    if (newHeight > maxHeight) {
                        // Keep at maxHeight instead of collapsing
                        this.$refs.textarea.style.height = `${maxHeight}px`;
                        this.$refs.textarea.style.overflowY = 'auto'; // Ensure scrollbar is shown when needed
                    } else {
                        this.$refs.textarea.style.height = `${newHeight}px`;
                        this.$refs.textarea.style.overflowY = 'hidden'; // Hide scrollbar when content fits within maxRows
                    }
                } else {
                    // No maxRows set, just adjust height directly
                    this.$refs.textarea.style.height = `${newHeight}px`;
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

