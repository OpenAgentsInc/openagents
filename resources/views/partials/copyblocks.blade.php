<script>
    function addCopyButton(codeBlock) {
        console.log('addCopyButton() called with codeBlock', codeBlock);

        if (!codeBlock) {
            console.log("no codeblock so bye")
            return;
        }

        // Check if the code block already has a "Copy" button
        const existingCopyButton = codeBlock.querySelector('.copy-button');
        if (!existingCopyButton) {
            // Add a new "Copy" button only if it doesn't exist
            const copyButton = document.createElement('button');
            copyButton.innerText = 'Copy';
            copyButton.classList.add('copy-button');
            copyButton.addEventListener('click', function () {
                const code = codeBlock.querySelector('code').innerText;
                navigator.clipboard.writeText(code).then(function () {
                    copyButton.innerText = 'Copied!';
                    setTimeout(function () {
                        copyButton.innerText = 'Copy';
                    }, 2000);
                }, function (err) {
                    console.error('Failed to copy: ', err);
                });
            });

            codeBlock.appendChild(copyButton);
            console.log('Added new "Copy" button');
        }
    }

    function processCodeBlocks() {
        console.log('processCodeBlocks() called');
        const codeBlocks = document.querySelectorAll('.markdown-content pre.shiki');
        console.log('Number of code blocks:', codeBlocks.length);

        codeBlocks.forEach(function (codeBlock, blockIndex) {
            console.log(`Processing code block ${blockIndex + 1}`);
            addCopyButton(codeBlock);
        });
    }

    function observeCodeBlocks() {
        const observer = new MutationObserver(function (mutations) {
            mutations.forEach(function (mutation) {
                if (mutation.type === 'childList') {
                    const addedNodes = mutation.addedNodes;
                    addedNodes.forEach(function (node) {
                        if (node.nodeType === Node.ELEMENT_NODE && node.matches('.markdown-content pre.shiki')) {
                            addCopyButton(node);
                        }
                    });
                }
            });
        });

        const observerOptions = {
            childList: true,
            subtree: true
        };

        const mainElement = document.querySelector('main');
        observer.observe(mainElement, observerOptions);
    }

    document.addEventListener('DOMContentLoaded', function () {
        console.log('DOMContentLoaded event triggered');
        processCodeBlocks();
        observeCodeBlocks();
    });

    window.addEventListener('message-created', function () {
        console.log('message-created event triggered');
        setTimeout(function () {
            processCodeBlocks();
        }, 0);
    });
</script>
