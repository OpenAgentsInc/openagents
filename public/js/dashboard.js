function focusTextarea() {
    const textarea = document.getElementById('message-textarea');
    if (textarea) {
        setTimeout(() => {
            textarea.focus();
        }, 100);
    } else {
        console.log('Textarea not found');
    }
}

function setupChatLoadedListener() {
    document.body.addEventListener('chatLoaded', function() {
        focusTextarea();
    });
}

document.addEventListener('DOMContentLoaded', function() {
    setupChatLoadedListener();
    focusTextarea();

    // MutationObserver setup
    const observer = new MutationObserver(function(mutations) {
        mutations.forEach(function(mutation) {
            if (mutation.type === 'childList') {
                const addedNodes = mutation.addedNodes;
                for (let i = 0; i < addedNodes.length; i++) {
                    if (addedNodes[i].id === 'message-textarea') {
                        console.log('Textarea added to DOM');
                        focusTextarea();
                        observer.disconnect(); // Stop observing once we've found the textarea
                        break;
                    }
                }
            }
        });
    });

    // Start observing the document with the configured parameters
    observer.observe(document.body, { childList: true, subtree: true });
});
