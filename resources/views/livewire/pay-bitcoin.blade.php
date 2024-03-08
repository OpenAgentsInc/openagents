<div class="mt-24 text-center flex flex-col justify-center items-center">
    <h1>Add agent credit</h1>
    <h4 class="mt-6 text-gray">Pay with Bitcoin Lightning</h4>

    <div class="mt-16 w-[350px] flex flex-col text-left">
        <x-label for="fee">Amount (sats)</x-label>
        <div class="w-full">
            <input type="hidden" name="fee" id="slider-input" value="0"/>
            <x-slider min="10" max="5000" step="10" value="100"/>
        </div>
    </div>

    @if (!!$qr)
        <img class="mt-12" src="{{ $qr }}" alt="QR code">
    @endif
</div>
