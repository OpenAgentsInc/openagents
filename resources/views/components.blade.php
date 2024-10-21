<x-layouts.app>
    <main class="flex flex-col items-center justify-center min-h-screen p-8 bg-black text-white overflow-hidden">
        <h1 class="text-5xl font-bold mb-12 pointer-events-none select-none">Component Library</h1>

        <!-- Buttons Section -->
        <section class="w-full max-w-4xl mb-16">
            <h2 class="text-3xl font-semibold mb-6">Buttons</h2>
            <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
                <x-button>
                    Default Button
                </x-button>

                <x-button variant="destructive">
                    Destructive
                </x-button>

                <x-button variant="outline">
                    Outline
                </x-button>

                <x-button variant="secondary">
                    Secondary
                </x-button>

                <x-button variant="ghost">
                    Ghost
                </x-button>

                <x-button variant="link">
                    Link
                </x-button>

                <x-button size="sm">
                    Small
                </x-button>

                <x-button size="lg">
                    Large
                </x-button>
            </div>
        </section>

        <!-- Cards Section -->
        <section class="w-full max-w-4xl">
            <h2 class="text-3xl font-semibold mb-6">Cards</h2>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                <!-- Default Card -->
                <x-card>
                    <x-card-header>
                        <x-card-title>Default Card</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p>This is a default card with a title and some content.</p>
                    </x-card-content>
                </x-card>

                <!-- Card with Footer -->
                <x-card>
                    <x-card-header>
                        <x-card-title>Card with Footer</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p>This card has a footer with a button.</p>
                    </x-card-content>
                    <x-card-footer>
                        <x-button variant="secondary" class="w-full">
                            Action
                        </x-button>
                    </x-card-footer>
                </x-card>

                <!-- Card with Image -->
                <x-card>
                    <img src="https://via.placeholder.com/400x200" alt="Placeholder" class="w-full h-48 object-cover">
                    <x-card-header>
                        <x-card-title>Card with Image</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p>This card includes an image at the top.</p>
                    </x-card-content>
                </x-card>

                <!-- Interactive Card -->
                <x-card class="hover:shadow-lg transition-shadow duration-300">
                    <x-card-header>
                        <x-card-title>Interactive Card</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p>Hover over this card to see an interactive effect.</p>
                    </x-card-content>
                    <x-card-footer>
                        <x-button variant="outline" class="w-full">
                            Learn More
                        </x-button>
                    </x-card-footer>
                </x-card>
            </div>
        </section>
    </main>
</x-layouts.app>
