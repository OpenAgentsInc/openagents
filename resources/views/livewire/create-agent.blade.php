<div class="mt-10 p-5 y-5 mx-auto max-w-5xl">
    <div class="my-5 mx-auto max-w-[534px]">
        <form wire:submit.prevent="submit">
            <div class="flex flex-col gap-y-[32px]">

                <h3 class="font-bold">New Agent</h3>


                <div>
                    <x-label for="description">Description</x-label>
                    <x-chat.textarea id="description" class="block mt-1 w-full" type="text" name="description"
                                     wire:model='description'
                                     min-rows="3"
                                     required default="Add a short description about what this agent does."/>
                </div>

                <div>
                    <x-label for="instructions">Instructions</x-label>
                    <x-chat.textarea id="description" class="block mt-1 w-full" type="text" name="instructions"
                                     wire:model='instructions'
                                     min-rows="6"
                                     required
                                     default="What does this agent do? How does it behave? What should it avoid doing?"/>
                </div>
            </div>
        </form>
    </div>
</div>