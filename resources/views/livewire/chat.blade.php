<div class="flex h-screen w-full overflow-hidden bg-gray-900">
    <div class="fixed top-0 left-0 h-screen w-[300px] bg-offblack z-10">
        <div class="flex flex-col h-full">
            <button
                class="text-white hover:bg-lightgray px-4 py-2 mt-4 mb-2 ml-4 mr-4 rounded-lg transition-colors duration-300">
                New session
            </button>
            <div class="flex flex-col flex-grow overflow-y-auto px-3 pb-3.5">
                <div class="mt-2 text-md">
                    <p class="text-gray px-3 tracking-wider">Recent</p>
                    <ul class="mt-2 cursor-pointer">
                        <li class="text-white px-3 py-1 hover:bg-darkgray rounded-[6px]">
                            New session
                        </li>
                        <li class="text-white px-3 py-1 hover:bg-darkgray rounded-[6px]">
                            Component system
                        </li>
                        <li class="text-white px-3 py-1 hover:bg-darkgray rounded-[6px]">
                            Google tag manager
                        </li>
                        <li class="text-white px-3 py-1 hover:bg-darkgray rounded-[6px]">
                            Backend testing
                        </li>
                    </ul>
                </div>
                <div class="mt-auto">
                    <div class="flex items-center justify-between px-3 py-2 rounded-lg">
                        <div class="flex items-center">
                            <div class="h-8 w-8 bg-gray rounded-full mr-2"></div>
                            <span class="text-white text-sm">Chris</span>
                        </div>
                        <button class="text-white text-xs">
                            <svg class="h-4 w-4 fill-white" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                <path d="M10 12a2 2 0 110-4 2 2 0 010 4z" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <div class="pl-[300px] w-full h-screen flex flex-col">
        <div class="fixed top-0 left-[300px] right-0 h-[60px] bg-black border-b border-offblack z-10">
            top bar
        </div>

        <div class="pt-[60px] flex-1 overflow-auto">
            main content
        </div>

        <div class="fixed bottom-0 left-[300px] right-0 h-[60px] bg-black px-4 py-3 flex items-center z-10">
            <input type="text" placeholder="Write a message to Junior Developer..."
                class="flex-1 rounded-lg px-4 py-2 bg-darkgray text-white placeholder-lightgray focus:outline-none focus:ring focus:border-blue-300" />
            <button
                class="ml-4 bg-blue-600 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded-lg transition-colors duration-300">
                Send
            </button>
        </div>
    </div>
</div>
