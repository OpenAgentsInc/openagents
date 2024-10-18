document.addEventListener('DOMContentLoaded', function() {
    const form = document.getElementById('message-form');
    const messageList = document.getElementById('message-list');

    form.addEventListener('submit', function(e) {
        e.preventDefault();
        const formData = new FormData(form);
        const url = form.getAttribute('data-send-message-url');

        fetch(url, {
            method: 'POST',
            body: formData,
            headers: {
                'X-Requested-With': 'XMLHttpRequest',
                'Accept': 'text/event-stream',
            },
        }).then(response => {
            const reader = response.body.getReader();
            const decoder = new TextDecoder();

            let systemMessageElement = null;

            function readStream() {
                reader.read().then(({ done, value }) => {
                    if (done) {
                        console.log('Stream complete');
                        return;
                    }

                    const chunk = decoder.decode(value);
                    const lines = chunk.split('\n');
                    
                    lines.forEach(line => {
                        if (line.startsWith('data: ')) {
                            const data = line.slice(6);
                            if (data === '[DONE]') {
                                console.log('Stream ended');
                            } else {
                                try {
                                    const parsedData = JSON.parse(data);
                                    console.log('Received message:', parsedData);
                                    
                                    if (parsedData.type === 'user' || parsedData.type === 'system') {
                                        const tempDiv = document.createElement('div');
                                        tempDiv.innerHTML = parsedData.html;
                                        const newMessage = tempDiv.firstElementChild;
                                        messageList.insertBefore(newMessage, messageList.firstChild);

                                        if (parsedData.type === 'system') {
                                            systemMessageElement = newMessage.querySelector('.message-content');
                                        }
                                    } else if (parsedData.type === 'word') {
                                        if (systemMessageElement) {
                                            systemMessageElement.textContent += parsedData.content;
                                        }
                                    }
                                } catch (error) {
                                    console.error('Error parsing JSON:', error);
                                }
                            }
                        }
                    });

                    readStream();
                });
            }

            readStream();
        }).catch(error => {
            console.error('Fetch error:', error);
        });

        form.reset();
    });
});