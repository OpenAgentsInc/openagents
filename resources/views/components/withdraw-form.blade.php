@fragment('withdrawal-form')
    <div class="my-8 mx-auto max-w-xl">
        <div>
            <h2>Withdraw Funds</h2>
            <p>Enter the amount you wish to withdraw</p>
        </div>
        <div>
            <div id="withdraw-message"></div>
            <form id="withdraw-funds" hx-post="{{ route('withdraw') }}" hx-target="#withdraw-message"
                hx-swap="outerHTML" class="space-y-4">
                @csrf
                <div>
                    <label for="amount">Amount</label>
                    <div class="w-full">
                        <input type="hidden" name="amount" id="slider-input" value="0" />
                        <x-slider min="0" max="100" step="1" />
                    </div>
                </div>
                <div class="flex justify-center">
                    <button variant="outline" size="lg" type="submit" class="mt-4">
                        Withdraw
                    </button>
                </div>
            </form>
        </div>
    </div>
@endfragment
