<header>
    <nav class="bg-transparent">
        <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
            <div class="flex items-center justify-between h-16">
                <div class="flex items-center">
                    <div class="flex-shrink-0">
                        <a href="{{ route('home') }}">
                            <x-application-logo class="h-8 w-8" />
                        </a>
                    </div>
                    <div class="hidden md:block">
                        <div class="ml-10 flex items-baseline space-x-4">
                            <a href="{{ route('agents.index') }}"
                                class="text-white px-3 py-2 rounded-md text-sm font-medium">Agents</a>
                            <a href="{{ route('plugins.index') }}"
                                class="text-white px-3 py-2 rounded-md text-sm font-medium">Plugins</a>
                        </div>
                    </div>
                </div>
                <div class="hidden md:block">
                    <div class="ml-4 flex items-center md:ml-6">
                        @auth
                            <a href="{{ route('dashboard') }}"
                                class="text-white px-3 py-2 rounded-md text-sm font-medium">Dashboard</a>
                        @else
                            <a href="{{ route('login') }}"
                                class="text-white px-3 py-2 rounded-md text-sm font-medium">Login</a>
                            <a href="{{ route('register') }}"
                                class="text-white px-3 py-2 rounded-md text-sm font-medium">Register</a>
                        @endauth
                    </div>
                </div>
                <div class="-mr-2 flex md:hidden">
                    <button type="button"
                        class="bg-transparent inline-flex items-center justify-center p-2 rounded-md text-white hover:text-white hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-white"
                        aria-controls="mobile-menu" aria-expanded="false">
                        <span class="sr-only">Open main menu</span>
                        <svg class="block h-6 w-6" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                            stroke="currentColor" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                d="M4 6h16M4 12h16M4 18h16"></path>
                        </svg>
                        <svg class="hidden h-6 w-6" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                            stroke="currentColor" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                d="M6 18L18 6M6 6l12 12"></path>
                        </svg>
                    </button>
                </div>
            </div>
        </div>
    </nav>
</header>
