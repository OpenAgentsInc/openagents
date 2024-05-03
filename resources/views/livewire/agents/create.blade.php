<div>
    {{-- Stop trying to control. --}}


    <div class="mt-10 p-5 y-5 mx-auto max-w-5xl">
        <h1 class="text-md md:text-3xl font-bold my-6 md:mb-10 text-center">Agent Builder</h1>
        <div class="my-5 mx-auto max-w-[534px]">
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
                             dusk="name"
                             required placeholder="Name your agent"/>
                    @error('name')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="about">Description</label>
                    <x-textarea wire:model='about' placeholder="Add a short description about what this agent does"
                                id="about" class="block mt-1 w-full"
                                dusk="description"
                                rows="1" min-rows="1" name="about" required/>
                    @error('about')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="prompt">Instructions</label>
                    <x-textarea wire:model='prompt'
                                placeholder="What does this agent do? How does it behave? What should it avoid doing?"
                                dusk="instructions"
                                id="about" class="block mt-1 w-full" rows="5" min-rows="5" name="prompt"
                                required/>
                    @error('prompt')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="my-5">
                    <label for="files"> Knowledge</label>
                    <div class="mt-1 border-2 border-darkgray rounded-md">
                        @error('files.*')
                        <span class="error">{{ $message }}</span>
                        @enderror
                        <x-filepond ref="myFilepond" wire:model="files" multiple allowFileTypeValidation
                                    imagePreviewMaxHeight="300"
                                    acceptedFileTypes="['application/pdf', 'application/msword', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', 'application/vnd.ms-excel', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', 'text/csv', 'text/plain']"
                                    allowFileSizeValidation maxFileSize="10MB"/>
                    </div>
                </div>

                <div class="mt-5 flex justify-end">
                    <x-button type="submit" class="text-center justify-center gap-2 py-2 my-4"
                              dusk="create-agent-button">
                        Create Agent
                    </x-button>
                </div>
            </form>
        </div>
    </div>
</div>
