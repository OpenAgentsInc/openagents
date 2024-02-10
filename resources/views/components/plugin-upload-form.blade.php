@fragment('plugin-upload-form')
    <div class="my-8 mx-auto w-[480px] p-[32px] border-2 border-offblack rounded-[16px]">
        <div class="mb-[32px]">
            <div class="font-bold text-[24px]">Create Plugin</div>
            <div class="mt-1 text-[14px] text-gray">Make a new agent plugin from an Extism .wasm file</div>
        </div>
        <div>
            <form id="upload-plugin" method="POST" action="{{ route('plugins.store') }}"
                enctype="multipart/form-data" class="space-y-4">
                @csrf
                <div>
                    <x-input-label for="name">Name</x-input-label>
                    <x-input id="name" name="name" placeholder="Plugin Name" class="mt-1 block w-full" autofocus />
                </div>
                <div>
                    <x-input-label for="description">Description</x-input-label>
                    <x-textarea id="description" name="description" class="!outline-none mt-1"
                        placeholder="Plugin Description">
                    </x-textarea>
                </div>
                <div>
                    <x-input-label for="wasm_url">Wasm URL</x-input-label>
                    <x-input id="wasm_url" name="wasm_url" placeholder="Plugin Wasm URL" class="mt-1 block w-full" />
                </div>
                <div>
                    <x-input-label for="fee">Fee (satoshis)</x-input-label>
                    <div class="relative mt-1">
                        <x-input id="fee" name="fee" type="number" placeholder="Plugin Fee"
                            class="block w-full pl-3 pr-10" />
                        <div class="absolute inset-y-0 right-0 pr-3 flex items-center text-[20px] text-lightgray">
                            ₿
                        </div>
                    </div>
                </div>
                <div class="flex justify-center">
                    <x-button variant="default" size="lg" type="submit" class="w-full mt-[22px]">
                        Create
                    </x-button>
                </div>
            </form>
        </div>
    </div>
@endfragment
