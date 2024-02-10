<div id="chart-container" class="h-64 w-full">
    chart goes here
    <canvas id="chart"></canvas>
</div>

@push('scripts')

    <script>
        // document.addEventListener('DOMContentLoaded', function () {
        //     const ctx = document.getElementById('chart').getContext('2d');
        //     const myChart = new Chart(ctx, {
        //         type: 'line',
        //         data: {
        //             labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July'],
        //             datasets: [{
        //                 label: 'Dataset 1',
        //                 data: [65, 59, 80, 81, 56, 55, 40],
        //                 fill: false,
        //                 borderColor: 'rgb(75, 192, 192)',
        //                 tension: 0.1
        //             }]
        //         },
        //         options: {
        //             scales: {
        //                 y: {
        //                     beginAtZero: true
        //                 }
        //             }
        //         }
        //     });
        // });

    </script>
@endpush
