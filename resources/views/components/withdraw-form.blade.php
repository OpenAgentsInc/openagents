@fragment('withdrawal-form')
    <x-card class="my-8 mx-auto max-w-xl">
        <x-card-header>
            <x-card-title>Withdraw Funds</x-card-title>
            <x-card-description>Enter the amount you wish to withdraw</x-card-description>
        </x-card-header>
        <x-card-content>
            <form id="withdraw-funds" hx-post="{{ route('withdraw') }}" hx-target="#withdraw-message" hx-swap="outerHTML" class="space-y-4">
                @csrf
                <div>
                    <x-label for="amount">Amount</x-label>
                    <div class="w-full">
                        <input type="hidden" name="amount" id="slider-input" value="0" />
                        <x-slider min="0" max="100" step="1" />
                    </div>
                </div>
                <div class="flex justify-center">
                    <x-button variant="outline" size="lg" type="submit">
                        Withdraw
                    </x-button>
                </div>
            </form>
<div id="withdraw-message">
    @if (isset($successMessage))
        <div class="bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded relative" role="alert">
            <span class="block sm:inline">{{ $successMessage }}</span>
        </div>
    @elseif (isset($errorMessage))
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative" role="alert">
            <span class="block sm:inline">{{ $errorMessage }}</span>
        </div>
    @endif
    </div>
        </x-card-content>
    </x-card>
@endfragment
