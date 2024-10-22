console.log('Dashboard.js loaded');

function focusTextarea() {
    console.log('Attempting to focus textarea');
    const textarea = document.getElementById('message-textarea');
    if (textarea) {
        console.log('Textarea found, focusing');
        textarea.focus();
    } else {
        console.log('Textarea not found');
    }
}

function setupChatLoadedListener() {
    console.log('Setting up chatLoaded listener');
    document.body.addEventListener('chatLoaded', function() {
        console.log('chatLoaded event received');
        focusTextarea();
    });
}

document.addEventListener('DOMContentLoaded', function() {
    console.log('DOM fully loaded');
    setupChatLoadedListener();
    focusTextarea();
});

// Immediate execution
(function() {
    console.log('Immediate function executed');
    setupChatLoadedListener();
    focusTextarea();
})();

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

console.log('Dashboard.js script end');
