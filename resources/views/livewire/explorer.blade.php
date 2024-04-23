<div>
    @foreach($prismSinglePayments as $payment)
        <p>{{ $payment->status }}</p>
    @endforeach
</div>
