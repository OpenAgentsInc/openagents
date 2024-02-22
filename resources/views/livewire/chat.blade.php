<div class="relative z-0 h-screen w-full overflow-hidden">
    <!-- Outer container with h-screen to ensure it fits the viewport height exactly -->
    <div class="fixed h-screen w-[300px] overflow-hidden bg-offblack">
        <!-- Fixed positioning with h-screen to match the viewport height, overflow-hidden to contain everything -->
        <div class="flex flex-col h-full w-full">
            <!-- Flex container to structure the content, ensuring it's within the fixed container's bounds -->
            <x-button variant="ghost" size="lg" class="my-4">New session</x-button>
            <!-- Your button stays at the top -->
            <div class="flex flex-col h-full w-full px-3 pb-3.5 overflow-y-auto">
                <!-- Scrollable area for content, overflow-y-auto to allow vertical scrolling within this div -->
                <div class="flex flex-col flex-1 transition-opacity duration-500 -mr-2 pr-2">
                    <!-- Content section with flex-1 to fill available space, ensuring scrolling as needed -->
                    <p class="text-gray">Recent</p>
                    <p>You chattin about stuff</p>
                    <p>Component system</p>
                    <p>Google tag manager</p>
                    <p>Backend testing</p>
                </div>
                <div class="flex flex-col pt-2">
                    <!-- Additional content or footer area -->
                    hello
                </div>
            </div>
        </div>
    </div>
</div>
