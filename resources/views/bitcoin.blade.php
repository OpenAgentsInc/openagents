<script src="https://unpkg.com/htmx.org@1.9.10"
    integrity="sha384-D1Kt99CQMDuVetoL1lrYwg5t+9QdHe7NLX/SoJYkXDFfX37iInKRy5xLSi8nO7UC" crossorigin="anonymous">
</script>
<script src="https://unpkg.com/htmx.org/dist/ext/sse.js"></script>


<h1 hx-ext="sse" sse-connect="/bitcoin-price-ticker" sse-swap="message">
    Loading bitcoin price...
</h1>
