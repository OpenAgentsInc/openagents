<script src="https://unpkg.com/htmx.org@1.9.10"
    integrity="sha384-D1Kt99CQMDuVetoL1lrYwg5t+9QdHe7NLX/SoJYkXDFfX37iInKRy5xLSi8nO7UC" crossorigin="anonymous">
</script>
<script src="https://unpkg.com/htmx.org/dist/ext/sse.js"></script>

<div hx-get="{{ route('bitcoin-price') }}" hx-trigger="every 5s" id="bitcoin-price">
    @include('bitcoin-price', ['price' => $price])
</div>

<div hx-ext="sse" sse-connect="/bitcoin-price-ticker" sse-swap="message">
    Contents of this box will be updated in real time
    with every SSE message received from the chatroom.
</div>
