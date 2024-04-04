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
                    let maxHeight = this.maxRows * this.lineHeight();
                    if (newHeight > maxHeight) {
                        newHeight = maxHeight;
                        this.$refs.textarea.style.overflow = 'auto';
                    } else {
                        this.$refs.textarea.style.overflow = 'hidden';
                    }
                }
                this.height = newHeight;
            },
            lineHeight() {
                return parseFloat(getComputedStyle(this.$refs.textarea).lineHeight);
            }
        }
    }
</script>
