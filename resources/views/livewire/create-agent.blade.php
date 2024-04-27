<div class="mt-10 p-5 y-5 mx-auto max-w-5xl">
    <div class="my-5 mx-auto max-w-[534px]">
        <form wire:submit.prevent="submit">
            <div class="flex flex-col gap-y-[32px]">

                <h3 class="font-bold text-center">Create Agent</h3>

                <div>
                    <x-label for="description">Name</x-label>
                    <x-chat.input id="name" class="block mt-1 w-full" type="text" name="name"
                                  wire:model='name'
                                  dusk="name"
                                  required placeholder="Name your agent."/>
                </div>

                <div>
                    <x-label for="description">Description</x-label>
                    <x-chat.textarea id="description" class="block mt-1 w-full" type="text" name="description"
                                     wire:model='description'
                                     dusk="description"
                                     min-rows="3"
                                     required default="Add a short description about what this agent does."/>
                </div>

                <div>
                    <x-label for="instructions">Instructions</x-label>
                    <x-chat.textarea id="description" class="block mt-1 w-full" type="text" name="instructions"
                                     wire:model='instructions'
                                     dusk="instructions"
                                     min-rows="6"
                                     required
                                     default="What does this agent do? How does it behave? What should it avoid doing?"/>
                </div>

                <div>
                    <x-label for="knowledge">Knowledge</x-label>
                    <div class="mt-1">
                        @error('files.*')
                        <span class="error">{{ $message }}</span>
                        @enderror
                        <x-filepond ref="myFilepond" wire:model="files" multiple allowFileTypeValidation
                                    imagePreviewMaxHeight="300"
                                    acceptedFileTypes="['application/pdf', 'application/msword', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', 'application/vnd.ms-excel', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', 'text/csv', 'text/plain']"
                                    allowFileSizeValidation maxFileSize="10MB"
                        />
                    </div>
                </div>

                <div class="w-full text-center">
                    <x-button>Create</x-button>
                </div>


            </div>
        </form>
    </div>
</div>