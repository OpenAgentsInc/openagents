@props([
    'className' => '',
    'hasError' => false,
    'showIcon' => false,
    'iconName' => '',
    'minRows' => 1,
    'maxRows' => null,
    'default' => '',
    'wireModel' => '',
    'imageUpload' => false, // new prop
])

<div class="relative" x-data="{
    inputVal: @entangle($attributes->wire('model')),
    isDisabled: true,
    height: 'auto',
    minRows: @js($minRows),
    maxRows: @js($maxRows),
    viewportMaxHeight: window.innerHeight * 0.95,
    promptIndex: -1,
    draftPrompt: '',
    promptHistory: [],
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
            newHeight = Math.max(newHeight, this.minRows * this.lineHeight());
            this.$refs.textarea.style.height = `${newHeight}px`;
            this.$refs.textarea.style.overflowY = 'hidden';
        }
    },
    lineHeight() {
        return parseFloat(getComputedStyle(this.$refs.textarea).lineHeight);
    },
    scrollbarWidth() {
        return this.$refs.textarea.offsetWidth - this.$refs.textarea.clientWidth;
    },
    resetHeight() {
        const minHeight = Math.max(this.minRows * this.lineHeight(), 48);
        this.$refs.textarea.style.height = `${minHeight}px`;
        this.$refs.textarea.style.overflowY = 'hidden';
        this.isDisabled = true;
    },
    reusePreviousPrompt(event) {
        this.setupPromptHistory();
        --this.promptIndex;
        if (this.promptIndex < 0) {
            this.promptIndex = this.promptHistory.length - 1;
        }
        this.setInputFromPrompt();
    },
    reuseNextPrompt(event) {
        this.setupPromptHistory();
        ++this.promptIndex;
        if (this.promptIndex >= this.promptHistory.length) {
            this.promptIndex = 0;
        }
        this.setInputFromPrompt();
    },
    setupPromptHistory() {
        this.promptHistory = [];
        const prompts = document.getElementsByClassName('prompt');
        for (let i = 0; i < prompts.length; i++) {
            this.promptHistory.push(prompts[i].textContent.trim());
        }
        if (this.draftPrompt === '') {
            this.draftPrompt = this.$refs.textarea.value;
        }
        this.promptHistory.push(this.draftPrompt);
    },
    setInputFromPrompt() {
        if (this.promptHistory[this.promptIndex]) {
            this.$refs.textarea.value = this.promptHistory[this.promptIndex];
            this.update();
        }
    }
}" x-init="$nextTick(() => {
    $refs.textarea.style.height = `${minRows * lineHeight()}px`;
    update();
})" wire:key="{{ $wireModel }}"
>
<textarea
        x-ref="textarea"
        autocomplete="off"
        wire:ignore
        @input="update(); isDisabled = inputVal.length === 0"
        placeholder="{{ $default }}"
        x-model="inputVal"
        x-effect="if (inputVal === '') resetHeight()"
        :rows="minRows"
        @keyup.alt.up.prevent="reusePreviousPrompt"
        @keyup.alt.down.prevent="reuseNextPrompt"
    {{ $attributes->merge([
        'class' => "resize-none flex w-full rounded-md border-2 bg-transparent px-4 py-[0.65rem] " . ($imageUpload ? 'pl-10' : 'pl-4') . " pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white " . ($hasError ? 'border-red' : 'border-[#3D3E42]') . " $className transition-all duration-300 ease-in-out",
    ]) }}
>
    {{ $slot }}
</textarea>

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

    @if($imageUpload)
        {{-- Add the image upload button --}}
        <label for="imageUpload" class="absolute bottom-[10px] left-[10px] text-gray cursor-pointer"
               aria-label="Attach files">
            <div class="flex w-full gap-2 items-center justify-center">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none"
                     xmlns="http://www.w3.org/2000/svg">
                    <path fill-rule="evenodd" clip-rule="evenodd"
                          d="M9 7C9 4.23858 11.2386 2 14 2C16.7614 2 19 4.23858 19 7V15C19 18.866 15.866 22 12 22C8.13401 22 5 18.866 5 15V9C5 8.44772 5.44772 8 6 8C6.55228 8 7 8.44772 7 9V15C7 17.7614 9.23858 20 12 20C14.7614 20 17 17.7614 17 15V7C17 5.34315 15.6569 4 14 4C12.3431 4 11 5.34315 11 7V15C11 15.5523 11.4477 16 12 16C12.5523 16 13 15.5523 13 15V9C13 8.44772 13.4477 8 14 8C14.5523 8 15 8.44772 15 9V15C15 16.6569 13.6569 18 12 18C10.3431 18 9 16.6569 9 15V7Z"
                          fill="currentColor"></path>
                </svg>
            </div>
        </label>
        <input id="imageUpload" type="file" wire:model="images" multiple tabindex="-1"
               class="hidden">
    @endif
</div>
