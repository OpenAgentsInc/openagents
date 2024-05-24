@pro
<script src="https://unpkg.com/htmx.org@2.0.0-beta4/dist/htmx.min.js"></script>
<script src="https://unpkg.com/htmx-ext-sse@2.0.0/sse.js"></script>
<div class="select-none z-[9999] fixed top-[10px] right-[65px] p-2 border-offblack text-darkgray border shadow-lg rounded-lg">
    {{--    <div hx-ext="sse" sse-connect="/stream">--}}
    {{--        ₿--}}
    {{--        <span sse-swap="message">0</span>--}}
    {{--    </div>--}}
</div>

<script>
    const button = document.querySelector('button');
    const evtSource = new EventSource('/stream');
    console.log(evtSource.withCredentials);
    console.log(evtSource.readyState);
    console.log(evtSource.url);
    const eventList = document.querySelector('ul');

    evtSource.onopen = function () {
        console.log("Connection to server opened.");
    };

    evtSource.onmessage = function (e) {
        console.log(e.data)
        // const newElement = document.createElement("li");

        // newElement.textContent = "message: " + e.data;
        // eventList.appendChild(newElement);
    };

    evtSource.onerror = function (e) {
        console.log("EventSource failed.");
        console.log("error?", e)
    };
</script>
@endpro