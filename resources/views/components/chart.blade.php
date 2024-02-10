<div id="chart-container" class="h-64 w-full">
    <div x-data="{
        labels: ['Jan', 'Feb', 'Mar', 'Apr', 'May'],
        values: [200, 150, 350, 225, 125],
        init() {
            let chart = new Chart(this.$refs.canvas.getContext('2d'), {
                type: 'line',
                data: {
                    labels: this.labels,
                    datasets: [{
                        data: this.values,
                        backgroundColor: 'white',
                        borderColor: 'white',
                    }],
                },
                options: {
                    interaction: { intersect: false },
                    scales: { y: { beginAtZero: true }},
                    plugins: {
                        legend: { display: false },
                        tooltip: {
                            displayColors: false,
                            callbacks: {
                                label(point) {
                                    return 'Sales: $'+point.raw
                                }
                            }
                        }
                    }
                }
            })

            this.$watch('values', () => {
                chart.data.labels = this.labels
                chart.data.datasets[0].data = this.values
                chart.update()
            })
        }
    }" class="w-full">
        <canvas x-ref="canvas" class="rounded-lg p-8"></canvas>
    </div>
</div>

@push('scripts')
    <script src="https://cdn.jsdelivr.net/npm/chart.js@3.5.1/dist/chart.min.js"></script>
@endpush
