<div class="relative z-0 h-screen w-full overflow-hidden bg-gray-900">
    <!-- Adding bg-gray-900 for the outer container if it needs to match the sidebar background -->
    <div class="fixed top-0 left-0 h-screen w-[300px] overflow-hidden bg-offblack">
        <!-- Assuming bg-gray-800 to match the sidebar color -->
        <div class="flex flex-col h-full">
            <!-- Removed the width class here as it is unnecessary; the width is already defined by the parent -->
            <button
                class="text-white hover:bg-lightgray px-4 py-2 mt-4 mb-2 ml-4 mr-4 rounded-lg transition-colors duration-300">
                <!-- Assuming you have a similar button style as x-button, replaced with a regular button for example -->
                New session
            </button>
            <div class="flex flex-col flex-grow overflow-y-auto px-3 pb-3.5">
                <!-- Changed to flex-grow from flex-1 for more explicit growth behavior -->
                <div class="mt-2 text-md">
                    <!-- Removed unnecessary classes and added margin-top for spacing -->
                    <p class="text-gray text-xs uppercase tracking-wider">Recent</p>
                    <!-- Styles for the 'Recent' label to match the image -->
                    <ul class="mt-2">
                        <!-- Use an unordered list for semantic structure -->
                        <li class="text-gray-300 py-1 hover:bg-darkgray rounded">
                            New session
                        </li>
                        <li class="text-gray-300 py-1 hover:bg-darkgray rounded">
                            Component system
                        </li>
                        <li class="text-gray-300 py-1 hover:bg-darkgray rounded">
                            Google tag manager
                        </li>
                        <li class="text-gray-300 py-1 hover:bg-darkgray rounded">
                            Backend testing
                        </li>
                    </ul>
                </div>
                <div class="mt-auto">
                    <!-- Push this to the bottom -->
                    <div class="flex items-center justify-between px-3 py-2 rounded-lg">
                        <!-- Styling for the 'Chris' section to match the bottom part of the sidebar in the image -->
                        <div class="flex items-center">
                            <div class="h-8 w-8 bg-gray rounded-full mr-2"></div>
                            <!-- Placeholder for the avatar circle -->
                            <span class="text-white text-sm">Chris</span>
                        </div>
                        <button class="text-white text-xs">
                            <!-- This button could be a dropdown or action trigger -->
                            <svg class="h-4 w-4 fill-white" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                <!-- SVG icon placeholder (e.g., dots for menu, could be replaced with actual icon code) -->
                                <path d="M10 12a2 2 0 110-4 2 2 0 010 4z" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>
