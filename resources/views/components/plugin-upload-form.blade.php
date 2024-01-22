@fragment('plugin-upload-form')
    <x-card class="my-8 mx-auto max-w-xl">
        <x-card-header>
            <x-card-title>Create Plugin</x-card-title>
        </x-card-header>
        <x-card-content>
            <form id="upload-plugin" method="POST" action="{{ route('plugins.store') }}"
                enctype="multipart/form-data" class="space-y-4">
                @csrf
                <div>
                    <label for="name" class="block text-md font-medium">Name</label>
                    <x-input id="name" name="name" placeholder="Plugin Name" class="mt-1 block w-full" />
                </div>
                <div>
                    <label for="description" class="block text-md font-medium">Description</label>
                    <x-textarea id="description" name="description" class="!outline-none mt-1"
                        placeholder="Plugin Description">
                    </x-textarea>
                </div>
                <div>
                    <label for="wasm_url" class="block text-md font-medium">Wasm URL</label>
                    <x-input id="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL" class="mt-1 block w-full" />
                </div>

                <div>
                    <label for="fee" class="block text-md font-medium">Fee</label>
                    <div class="w-full">
                        <x-slider min="0" max="100" step="1" />
                    </div>
                </div>

                <div class="flex justify-center">
                    <x-button variant="default" type="submit">
                        Upload Plugin
                    </x-button>
                </div>
            </form>
        </x-card-content>
    </x-card>

@endfragment
