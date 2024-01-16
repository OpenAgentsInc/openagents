<script src="https://unpkg.com/htmx.org@1.9.10"
    integrity="sha384-D1Kt99CQMDuVetoL1lrYwg5t+9QdHe7NLX/SoJYkXDFfX37iInKRy5xLSi8nO7UC" crossorigin="anonymous">
</script>

<div hx-get="{{ route('bitcoin-price') }}" hx-trigger="every 5s" id="bitcoin-price">
    @include('bitcoin-price', ['price' => $price])
</div>
