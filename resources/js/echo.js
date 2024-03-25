import Echo from 'laravel-echo';

import Pusher from 'pusher-js';

window.Pusher = Pusher;

window.Echo = new Echo({
    broadcaster: 'pusher',
    key: import.meta.env.VITE_PUSHER_APP_KEY,
    cluster: import.meta.env.VITE_PUSHER_APP_CLUSTER,
    forceTLS: true
});


document.addEventListener('livewire:load', function () {
    window.Echo.channel('payments')
        .listen('.PaymentCreated', (e) => {
            console.log('Payment created', e);
        });
});

