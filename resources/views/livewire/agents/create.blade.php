<div>
    <div class="mt-10 p-5 y-5 mx-auto w-full max-w-5xl md:max-w-[800px]">
        <h1 class="text-md md:text-3xl font-bold my-6 md:mb-10 text-center">Create an agent</h1>
        <div class="my-5 mx-auto max-w-5xl">
            <form wire:submit.prevent="submit">
                <div class="col-span-full flex items-center gap-x-8 my-5">
                    @if ($image)
                        <img src="{{ $image->temporaryUrl() }}"
                            class="h-24 w-24 flex-none rounded-lg bg-gray-800 object-cover">
                    @else
                        <img src="{{ url('/images/no-image.jpg') }}" alt=""
                            class="h-24 w-24 flex-none rounded-lg bg-gray-800 object-cover">
                    @endif

                    <div>
                        <button type="button" x-on:click="$refs.imageUpload.click()"
                            class="rounded-md bg-white/10 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-white/20">
                            Change
                            image
                        </button>
                        <input type="file" x-ref="imageUpload" class="hidden" accept="image/jpg, image/png"
                            wire:model="image">
                        <p class="mt-2 text-xs leading-5 text-gray-400">JPG, PNG. 2MB max.</p>
                        @error('image')
                            <div class="my-2">
                                <span class="error">{{ $message }}</span>
                            </div>
                        @enderror
                    </div>
                </div>

                <div class="mt-5">
                    <label for="name">Name</label>
                    <x-input id="name" class="block mt-1 w-full " type="text" name="name" wire:model='name'
                        dusk="name" required placeholder="Name your agent" />
                    @error('name')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="about">Description</label>
                    <x-textarea wire:model='about' placeholder="Add a short description about what this agent does"
                        id="about" class="block mt-1 w-full" dusk="description" min-rows="3" name="about"
                        required />
                    @error('about')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="prompt">Instructions</label>
                    <x-textarea wire:model='prompt'
                        placeholder="What does this agent do? How does it behave? What should it avoid doing?"
                        dusk="instructions" id="about" class="block mt-1 w-full" min-rows="3" name="prompt"
                        required />
                    @error('prompt')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5 select-none">
                    <label for="files">Knowledge Files</label>
                    <div class="mt-1 border-2 border-darkgray rounded-md">
                        @error('files.*')
                            <span class="error">{{ $message }}</span>
                        @enderror
                        <x-filepond ref="myFilepond" wire:model="files" multiple allowFileTypeValidation
                            imagePreviewMaxHeight="300"
                            acceptedFileTypes="['application/pdf', 'text/markdown','text/html', 'text/csv', 'text/plain']"
                            fileValidateTypeLabelExpectedTypesMap="{{ json_encode([
                                'application/pdf' => '.pdf',
                                'text/plain' => '.txt',
                                'text/markdown' => '.md',
                                'text/html' => '.html',
                                'text/csv' => '.csv',
                            ]) }}"
                            allowFileSizeValidation maxFileSize="20MB" />

                    </div>
                </div>
                <div class="mt-5">
                    <label for="urls">Knowledge URLs</label>
                    <x-textarea wire:model='urls'
                        placeholder="If you have any URLs with content that you would like to add to this agent, please add them here one per line."
                        dusk="urls" id="urls" class="block mt-1 w-full" min-rows="3" name="urls" />
                    @error('urls')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                {{--                <div class="my-5"> --}}
                {{--                    <label for="capabilities">Capabilities</label> --}}
                {{--                    <div class="mt-1"> --}}
                {{--                        <label class="inline-flex items-center"> --}}
                {{--                            <input type="checkbox" wire:model="codebase_search" dusk="codebase_search" --}}
                {{--                                class="text-offblack focus:ring-0 active:bg-offblack focus:bg-offblack checked:bg-offblack rounded bg-black border-darkgray shadow" /> --}}

                {{--                            <span class="ml-2 select-none text-[#777A81]">Codebase Search</span> --}}
                {{--                        </label> --}}
                {{--                    </div> --}}
                {{--                </div> --}}


                <div class="mt-5">
                    <label for="model">AI model for chats</label>
                    <select wire:model="model" id="model" class="block mt-1 w-full p-2 border rounded">
                        @foreach ($models as $modelKey => $modelData)
                            @if ($modelData['access'] !== 'pro')
                                <option value="{{ $modelKey }}">{{ $modelData['name'] }}</option>
                            @endif
                        @endforeach
                    </select>
                    @error('model')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="pro_model">AI model for pro-chats</label>
                    <select wire:model="pro_model" id="pro_model" class="block mt-1 w-full p-2 border rounded">
                        @foreach ($models as $modelKey => $modelData)
                            @if ($modelData['access'] === 'pro')
                                <option value="{{ $modelKey }}">{{ $modelData['name'] }}</option>
                            @endif
                        @endforeach
                    </select>
                    @error('pro_model')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>




                <div class="my-5 w-full ">
                        <label for="plugins" class="my-1">Select plugins</label>
                        <x-select-search :data="$this->list_plugins" wire:model="plugins" placeholder="Select something!" multiple/>
                </div>


                <div class="mt-5 w-full text-center">
                    <x-button type="submit" class="text-center justify-center gap-2 py-2 my-4"
                        dusk="create-agent-button">
                        Create Agent
                    </x-button>
                </div>
            </form>
        </div>
    </div>
</div>
