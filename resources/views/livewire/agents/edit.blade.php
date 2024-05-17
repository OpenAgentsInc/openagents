<div>
    {{-- If you look to others for fulfillment, you will never truly be fulfilled. --}}


    <div class="mt-10 p-5 y-5 mx-auto w-full max-w-5xl md:max-w-[800px] lg:max-w-5xl">
        <div class="flex flex-col md:flex-row md:items-center md:gap-4 border-b py-4">
            <div class="order-2 flex-1">
                <h1 class="text-[25px] md:text-3xl font-bold">{{ $agent->name }}</h1>
                <h5 class="text-sm font-semibold">Edit agent</h5>
            </div>
            <a href="{{ route('agents') }}" wire:navigate class="order-1 mb-4 md:mb-0 md:text-left">
                <h3 class="text-[16px] text-gray">&larr; Back</h3>
            </a>
        </div>
        <div class="my-5 mx-auto max-w-5xl">
            <form wire:submit.prevent="submit">

                <div class="col-span-full flex items-center gap-x-8 my-5">
                    @if ($image)
                        <img src="{{ $image->temporaryUrl() }}"
                             class="h-24 w-24 flex-none rounded-lg bg-gray-800 object-cover">
                    @else
                        <img src="{{ $agent->image_url }}" alt=""
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
                             required placeholder="Agent name"/>
                    @error('name')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="about"> About</label>
                    <x-textarea wire:model='about' placeholder="about" id="about" class="block mt-1 w-full"
                                rows="1" min-rows="1" name="about" required/>
                    @error('about')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="mt-5">
                    <label for="prompt"> Instructions</label>
                    <x-textarea wire:model='prompt'
                                placeholder="What does this agent do? How does it behave? What should it avoid doing?"
                                id="about" class="block mt-1 w-full" rows="5" min-rows="5" name="prompt"
                                required/>
                    @error('prompt')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                {{--                <div class="mt-5">--}}
                {{--                    <label for="rag_prompt"> RAG Prompt</label>--}}
                {{--                    <x-textarea wire:model='rag_prompt' placeholder="Add your RAG prompt?" id="rag_prompt"--}}
                {{--                        class="block mt-1 w-full" rows="5" min-rows="5" name="rag_prompt" required />--}}
                {{--                    @error('rag_prompt')--}}
                {{--                        <span class="text-red mt-2 text-xs">{{ $message }}</span>--}}
                {{--                    @enderror--}}
                {{--                </div>--}}

                {{--                <div class="mt-5">--}}
                {{--                    <label for="message"> Welcome Message</label>--}}
                {{--                    <x-textarea wire:model='message' placeholder="How this agent starts conversation." id="message"--}}
                {{--                                class="block mt-1 w-full" rows="5" min-rows="5" name="message" required/>--}}
                {{--                    @error('message')--}}
                {{--                    <span class="text-red mt-2 text-xs">{{ $message }}</span>--}}
                {{--                    @enderror--}}
                {{--                </div>--}}


                <div class="my-6 block">
                    <label for="files">Knowledge Files</label>
                    <div class="my-2 text-neutral-400 text-sm font-normal font-['JetBrains Mono']">If you upload files
                        under Knowledge, conversations with your Agent may include its contents.
                    </div>
                    @livewire('agents.partials.documents',['agent_id' => $agent->id])
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
                            'text/csv' => '.csv'
                            ]) }}"
                                    allowFileSizeValidation maxFileSize="20MB"/>
                    </div>
                </div>

                <div class="my-6 block">
                    <label for="urls">Knowledge URLs</label>
                    <div class="my-2 text-neutral-400 text-sm font-normal font-['JetBrains Mono']">
                        If you add URLs under Knowledge, conversations with your Agent may include its contents.
                    </div>
                    <x-textarea wire:model='urls' dusk="urls" id="urls" class="block mt-1 w-full" min-rows="3"
                                name="urls"/>
                    @error('urls')
                    <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </div>

                <div class="my-5 rounded border border-gray p-3">
                    <x-switch label="Visibility" description="Make this agent personal or for public use"
                              wire:model='is_public'/>
                </div>

                <div class="mt-5 w-full text-center">
                    <x-button type="submit" class="text-center justify-center gap-2 py-2 my-4">
                        Update Agent
                    </x-button>
                </div>
            </form>
        </div>
    </div>


</div>
