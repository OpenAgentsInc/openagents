

<div
x-data="{}" x-init="
    const intervalId = setInterval(() => {
        @this.call('checkInvoiceStatus');
    }, 5000);

    @this.on('hook:destroyed', () => {
        clearInterval(intervalId);
    });
"
class="p-4 w-full flex-col items-center justify-center">



    <div>
        <h4>Please pay this invoice</h4>
        <div>{!! $qrCode !!}</div>
        <x-input type="text" value="{{ $invoice }}" class="block w-full text-xs"
        x-data x-ref="input" @click="$refs.input.select(); document.execCommand('copy');$dispatch('copiedToClipboard');" readonly />
    </div>
    <x-secondary-button class="mt-4 w-full"
    @click="window.webln?window.webln.enable().then(()=>window.webln.sendPayment('{{$invoice}}')):window.open(`lightning:{{$invoice}}`);"
    >Pay with app</x-secondary-button>
</div>
