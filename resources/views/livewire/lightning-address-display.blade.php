<span class="inline-flex items-center w-full text-xs m-2" >
<span class="inline-flex items-center mr-1 w-4 h-4 text-xs justify-center
">🗲</span> <x-input type="text" value="{{ $lightningAddress }}" class="block w-full text-xs"
                      x-data x-ref="input" @click="$refs.input.select(); document.execCommand('copy');$dispatch('copiedToClipboard');" readonly />
</span>
