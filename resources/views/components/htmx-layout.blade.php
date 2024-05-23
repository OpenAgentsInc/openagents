<!doctype html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/sse.js"></script>
    <script defer src="https://unpkg.com/alpinejs@3/dist/cdn.min.js"></script>
    @include('partials.vite')
    <link rel="stylesheet" href="{{ asset('vendor/tokyo-night-dark.min.css') }}">
    <script type="text/javascript"
            src="https://cdnjs.cloudflare.com/ajax/libs/markdown-it/11.0.1/markdown-it.min.js "></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/10.2.1/highlight.min.js"></script>
</head>
<body>
{{ $slot }}
</body>

<script>
    document.addEventListener('DOMContentLoaded', (event) => {
        const decodeHTML = (str) => {
            let txt = document.createElement('textarea');
            txt.innerHTML = str;
            return txt.value;
        };

        function renderMarkdown() {
            document.querySelectorAll('.message-body').forEach(element => {
                const decodedContent = decodeHTML(element.innerHTML);
                const md = window.markdownit()
                md.set({
                    highlight: function (str, lang) {
                        if (lang && hljs.getLanguage(lang)) {
                            try {
                                return '<pre class="hljs"><code>' +
                                    hljs.highlight(lang, str, true).value +
                                    '</code></pre>';
                            } catch (__) {
                            }
                        }

                        return '<pre class="hljs"><code>' + md.utils.escapeHtml(str) + '</code></pre>';
                    }
                })

                element.innerHTML = md.render(decodedContent);
            });
        }

        document.body.addEventListener('htmx:afterSwap', (event) => {
            renderMarkdown();
        });
    })
</script>
</html>