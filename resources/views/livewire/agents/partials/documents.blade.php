<div>
    {{-- Stop trying to control. --}}
    <div class="divide-y divide-offblack my-4">
        @foreach ($this->documents() as $document)
            <div class="w-full h-12 py-2 justify-start items-center gap-4 inline-flex">
                <div class=" px-1 py-px justify-center items-center flex">
                    <x-icon.file class="w-8 h-8" />
                </div>
                <div class="grow shrink basis-0 text-white text-base font-normal font-['JetBrains Mono']">
                    {{ $document->name }}</div>
                <div class="w-9 px-2 rounded justify-center items-center gap-2 flex">
                    <a role="button" class=" px-0.5 py-0.5 justify-center items-center flex"
                        x-on:click="Livewire.dispatch('openModal', { component: 'agents.modals.document-delete', arguments: { document: {{ $document->id }} } })">
                        <x-icon.cancel class="w-8 h-8" />
                    </a>
                </div>
            </div>
        @endforeach

    </div>
</div>
