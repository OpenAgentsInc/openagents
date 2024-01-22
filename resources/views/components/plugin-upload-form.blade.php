@fragment('plugin-upload-form')
    <x-card class="my-8 mx-auto max-w-xl">
        <x-card-header>
            <x-card-title>Create Plugin</x-card-title>
            <x-card-description>Make a new agent plugin from an Extism .wasm file</x-card-description>
        </x-card-header>
        <x-card-content>
            <form id="upload-plugin" method="POST" action="{{ route('plugins.store') }}"
                enctype="multipart/form-data" class="space-y-4">
                @csrf
                <div>
                    <x-label for="name">Name</x-label>
                    <x-input id="name" name="name" placeholder="Plugin Name" class="mt-1 block w-full" />
                </div>
                <div>
                    <x-label for="description">Description</x-label>
                    <x-textarea id="description" name="description" class="!outline-none mt-1"
                        placeholder="Plugin Description">
                    </x-textarea>
                </div>
                <div>
                    <x-label for="wasm_url">Wasm URL</x-label>
                    <x-input id="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL" class="mt-1 block w-full" />
                </div>
                <div>
                    <x-label for="fee">Fee</x-label>
                    <div class="w-full">
                        <input type="hidden" name="fee" id="slider-input" value="0" />
                        <x-slider min="0" max="100" step="1" />
                    </div>
                </div>
                <div class="flex justify-center">
                    <x-button variant="outline" size="lg" type="submit">
                        Create
                    </x-button>
                </div>
            </form>
        </x-card-content>
    </x-card>

@endfragment
