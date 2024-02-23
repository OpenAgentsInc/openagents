<x-blank-layout>
    <div class="p-8">
        <!-- Title and Logo Mark at the top -->
        <x-logomark />
        <h1 class="title my-4">Design system</h1>

        <!-- Grid Layout for Sections -->
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4 my-12">
            <!-- Buttons Section -->
            <div class="my-2 p-8 rounded-[8px] bg-offblack">
                <h3 class="mb-4 text-gray">Buttons</h3>

                <div class="flex flex-row space-x-6">
                    <x-button variant="primary" size="lg">Primary</x-button>
                    <x-button variant="secondary" size="lg">Secondary</x-button>
                    <x-button variant="ghost" size="lg">Ghost</x-button>
                </div>

                <div class="my-8 flex flex-row space-x-6">
                    <x-button variant="primary" size="md">Primary</x-button>
                    <x-button variant="secondary" size="md">Secondary</x-button>
                    <x-button variant="ghost" size="md">Ghost</x-button>
                </div>

                <div class="flex flex-row space-x-6">
                    <x-button variant="primary" size="sm">Primary</x-button>
                    <x-button variant="secondary" size="sm">Secondary</x-button>
                    <x-button variant="ghost" size="sm">Ghost</x-button>
                </div>
            </div>


            <!-- Brand Section -->
            <div class="my-2 p-8 rounded-[8px] bg-offblack">
                <div class="grid grid-cols-2 gap-4">
                    <!-- Logo Column -->
                    <div>
                        <h3 class="mb-4 text-gray">Logo</h3>
                        <x-logo />
                    </div>
                    <!-- Icon Column -->
                    <div class="flex justify-center">
                        <div>
                            <h3 class="mb-4 text-gray">Icon</h3>
                            <div class="bg-black p-4 rounded-[14px]">
                                <x-logo />
                            </div>
                        </div>
                    </div>
                </div>
                <!-- Mark Section -->
                <h3 class="mt-8 mb-2 text-gray">Mark</h3>
                <div class="space-y-4">
                    <x-logomark size="1" />
                    <x-logomark size="2" />
                    <x-logomark size="3" />
                    <x-logomark size="4" />
                    <x-logomark size="5" />
                </div>
            </div>

            <!-- Typography Section -->
            <div class="my-2 p-8 rounded-[8px] bg-offblack">
                <h3 class="mb-4 text-gray">Headings</h3>
                <div class="space-y-4">
                    <h1>Heading 1</h1>
                    <h2>Heading 2</h2>
                    <h3>Heading 3</h3>
                    <h4>Heading 4</h4>
                    <h5>Heading 5</h5>
                    <h6>Heading 6</h6>
                </div>
                <h3 class="mt-8 mb-4 text-gray">Body</h3>
                <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt
                    ut labore et dolore magna aliqua.</p>
            </div>

            <!-- Colors Section -->
            <div class="my-2 p-8 rounded-[8px] bg-offblack">
                <h3 class="mb-4 text-gray">Primary</h3>
                <div class="flex space-x-4">
                    <div class="w-12 h-12 bg-black rounded-[8px]"></div>
                    <div class="w-12 h-12 bg-offblack border border-black rounded-[8px]"></div>
                    <div class="w-12 h-12 bg-darkgray rounded-[8px]"></div>
                    <div class="w-12 h-12 bg-gray rounded-[8px]"></div>
                    <div class="w-12 h-12 bg-lightgray rounded-[8px]"></div>
                    <div class="w-12 h-12 bg-white rounded-[8px]"></div>
                </div>
            </div>
        </div>
    </div>
</x-blank-layout>
