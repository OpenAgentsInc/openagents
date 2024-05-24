@pro
<script src="https://unpkg.com/htmx.org@2.0.0-beta4/dist/htmx.min.js"></script>
<script src="https://unpkg.com/htmx-ext-sse@2.0.0/sse.js"></script>
<div class="select-none z-[9999] fixed top-[10px] right-[65px] p-2 border-offblack text-darkgray border shadow-lg rounded-lg">
    <div hx-ext="sse" sse-connect="/stream">
        ₿
        <span sse-swap="message">0</span>
    </div>
</div>
@endpro