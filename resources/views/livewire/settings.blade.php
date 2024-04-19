<div class="p-12 mx-auto flex flex-col justify-center w-full items-center" x-data="{ dropdown: false }">
    <div class="max-w-3xl min-w-[600px]">
        <x-pane title="Default model">
            <livewire:model-dropdown :selected-model="$this->formattedDefaultModel" :models="$models"
                                     action="setDefaultModel"/>
        </x-pane>
    </div>
</div>