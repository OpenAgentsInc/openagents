@fragment('withdrawal-form')
    <x-card class="my-8 mx-auto max-w-xl">
        <x-card-header>
            <x-card-title>Withdraw Funds</x-card-title>
            <x-card-description>Enter the amount you wish to withdraw</x-card-description>
        </x-card-header>
        <x-card-content>
            <form id="withdraw-funds" method="POST" action="{{ route('withdraw') }}" class="space-y-4">
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
        </x-card-content>
    </x-card>
@endfragment
