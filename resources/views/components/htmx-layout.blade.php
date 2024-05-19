<!doctype html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <script defer src="https://unpkg.com/alpinejs@3/dist/cdn.min.js"></script>
    @include('partials.vite')
    <link href="vendor/prism.css" rel="stylesheet"/>
    <link rel="stylesheet"
          href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@10.2.1/build/styles/default.min.css">
    <script type="text/javascript"
            src="https://cdnjs.cloudflare.com/ajax/libs/markdown-it/11.0.1/markdown-it.min.js "></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/10.2.1/highlight.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
    {{--    <link rel="stylesheet" href="https://unpkg.com/missing.css@1.1.1/prism">--}}
    <script src="{{ asset('vendor/prism.js') }}"></script>
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


        // Loop through all .message-body elements and apply marked
        document.querySelectorAll('.message-body').forEach(element => {
            const decodedContent = decodeHTML(element.innerHTML);
            // element.innerHTML = marked.parse(hljs.highlight('php', decodedContent));
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
    })
</script>

<script>
    // document.addEventListener('DOMContentLoaded', (event) => {
    //     const md = window.markdownit();
    //     md.set({
    //         highlight: function (str, lang) {
    //             if (lang && hljs.getLanguage(lang)) {
    //                 try {
    //                     return '<pre class="hljs"><code>' +
    //                         hljs.highlight(lang, str, true).value +
    //                         '</code></pre>';
    //                 } catch (__) {
    //                 }
    //             }
    //             return '<pre class="hljs"><code>' + md.utils.escapeHtml(str) + '</code></pre>';
    //         }
    //     });
    //
    //     function renderMarkdown() {
    //         console.log('rendering markdown?')
    //         document.querySelectorAll('.message-body').forEach(element => {
    //             const originalContent = element.innerText;
    //             const renderedContent = md.render(originalContent);
    //             console.log('original content', originalContent)
    //             console.log('rendered content', renderedContent)
    //             element.innerHTML = renderedContent;
    //             console.log("replaced content!")
    //         });
    //
    //         document.querySelectorAll('pre code').forEach((block) => {
    //             hljs.highlightElement(block);
    //         });
    //     }
    //
    //     // Initial run for existing content
    //     renderMarkdown();
    //
    //     // Render markdown for updated content after HTMX loads or updates
    //     document.body.addEventListener('htmx:afterSwap', (event) => {
    //         console.log('after swap../!')
    //         renderMarkdown();
    //     });
    // })


</script>

<script>
    // var md = window.markdownit();
    // md.set({
    //     highlight: function (str, lang) {
    //         if (lang && hljs.getLanguage(lang)) {
    //             try {
    //                 return '<pre class="hljs"><code>' +
    //                     hljs.highlight(lang, str, true).value +
    //                     '</code></pre>';
    //             } catch (__) {
    //             }
    //         }
    //
    //         return '<pre class="hljs"><code>' + md.utils.escapeHtml(str) + '</code></pre>';
    //     }
    // });
    //
    // var result = md.render('# markdown-it rulezz! \n\n ```html \n <pre><code class="js">function test();</code></pre>\n```');
    // document.getElementById('content').innerHTML = result;
</script>
</html>