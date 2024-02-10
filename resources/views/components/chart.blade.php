<div id="chart-container" class="h-[60vh] w-full">
    <div x-data="{
        labels: ['', '', '', '', '', '', ''],
        values: [2100, 1500, 1254, 3500, 2250, 4250, 5019],
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
                    responsive: true,
                    maintainAspectRatio: false,
                    interaction: { intersect: false },

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
    }" class="w-full h-full">
        <canvas x-ref="canvas" class="w-full h-full rounded-lg p-8"></canvas>
    </div>
</div>

@push('scripts')
    <script src="https://cdn.jsdelivr.net/npm/chart.js@3.5.1/dist/chart.min.js"></script>
@endpush
