# Nostr Relay Admin Dashboard

The admin dashboard provides real-time monitoring and metrics visualization for the Nostr relay.

## Features

### Real-Time Metrics Display
- **Auto-refresh**: Updates every 2 seconds
- **Live status**: Connection health and uptime tracking
- **Visual feedback**: Color-coded metrics and progress bars

### Monitored Metrics

#### Connection Statistics
- Active connections count
- Blocked connections (banned IPs)
- Blocked connections (rate limiting)

#### Event Processing
- Total events received
- Events stored successfully
- Events rejected (validation failures)
- Events rejected (signature failures)
- Events rejected (rate limiting)
- Storage success rate (%)
- Events per second throughput

#### Subscription Management
- Active subscriptions
- Total subscription requests
- Total subscription closes
- Average subscriptions per connection

#### Bandwidth Monitoring
- Total bytes received
- Total bytes sent
- Automatic formatting (B, KB, MB, GB)

#### Database Health
- Total queries executed
- Query errors
- Error rate (%)

#### Security & Rate Limiting
- Number of banned IPs
- Connections blocked by IP bans
- Connections blocked by rate limits

## Accessing the Dashboard

### HTTP Endpoints

1. **Dashboard UI** (Primary Interface)
   ```
   http://localhost:7001/admin/dashboard
   ```
   Interactive HTML dashboard with real-time updates

2. **Health Check** (JSON)
   ```
   GET http://localhost:7001/admin/health
   ```
   Returns basic health status and uptime

3. **Statistics** (JSON)
   ```
   GET http://localhost:7001/admin/stats
   ```
   Returns complete statistics in JSON format

4. **Prometheus Metrics** (Text)
   ```
   GET http://localhost:7001/admin/metrics
   ```
   Returns metrics in Prometheus exposition format

### Default Configuration

The admin server runs on `127.0.0.1:7001` by default. Configure via `AdminConfig`:

```rust
use nostr_relay::AdminConfig;

let config = AdminConfig {
    bind_addr: "127.0.0.1:7001".parse().unwrap(),
};
```

## API Response Formats

### Health Endpoint
```json
{
  "status": "ok",
  "uptime_secs": 3600,
  "timestamp": 1700000000
}
```

### Stats Endpoint
```json
{
  "health": {
    "status": "ok",
    "uptime_secs": 3600,
    "timestamp": 1700000000
  },
  "connections": {
    "active": 42,
    "blocked_banned": 5,
    "blocked_rate_limit": 12
  },
  "events": {
    "received": 10000,
    "stored": 9500,
    "rejected_validation": 300,
    "rejected_rate_limit": 150,
    "rejected_signature": 50,
    "storage_success_rate": 95.0,
    "events_per_second": 2.78
  },
  "subscriptions": {
    "active": 128,
    "total_requests": 500,
    "total_closes": 372,
    "avg_per_connection": 3.05
  },
  "bandwidth": {
    "bytes_received": 52428800,
    "bytes_sent": 104857600
  },
  "database": {
    "queries": 50000,
    "errors": 10,
    "error_rate": 0.02
  },
  "rate_limiting": {
    "banned_ips": 5
  }
}
```

### Prometheus Metrics Format
```
# HELP nostr_relay_uptime_seconds Relay uptime in seconds
# TYPE nostr_relay_uptime_seconds gauge
nostr_relay_uptime_seconds 3600

# HELP nostr_relay_connections_active Number of active connections
# TYPE nostr_relay_connections_active gauge
nostr_relay_connections_active 42

# HELP nostr_relay_events_received_total Total events received
# TYPE nostr_relay_events_received_total counter
nostr_relay_events_received_total 10000

... (additional metrics)
```

## Integration with Monitoring Tools

### Prometheus

Add to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'nostr-relay'
    static_configs:
      - targets: ['localhost:7001']
    metrics_path: '/admin/metrics'
    scrape_interval: 15s
```

### Grafana

1. Add Prometheus data source
2. Import or create dashboard using metrics:
   - `nostr_relay_connections_active`
   - `nostr_relay_events_received_total`
   - `nostr_relay_events_stored_total`
   - `nostr_relay_subscriptions_active`
   - etc.

### Custom Monitoring

Poll `/admin/stats` endpoint:

```bash
# Curl
curl http://localhost:7001/admin/stats | jq

# Continuous monitoring
watch -n 2 'curl -s http://localhost:7001/admin/stats | jq .events'
```

## Dashboard Features

### Visual Elements

1. **Status Badge**: Shows relay status (ONLINE/ERROR)
2. **Uptime Display**: Formatted uptime (days, hours, minutes, seconds)
3. **Metric Cards**: Large values with labels and trends
4. **Progress Bars**: Visual representation of success rates
5. **Color Coding**:
   - Green: Successful operations
   - Red: Errors and rejections
   - Yellow: Rate limiting
   - Blue: Normal metrics

### Auto-Update Behavior

- Polls `/admin/stats` every 2 seconds
- Updates all metrics without page reload
- Shows last update timestamp
- Handles connection errors gracefully

## Security Considerations

### Access Control

The admin dashboard currently has NO authentication. Recommendations:

1. **Bind to localhost only** (default)
   - Only accessible from the same machine
   - Use SSH tunneling for remote access

2. **Use a reverse proxy** with authentication
   ```nginx
   location /admin {
       auth_basic "Admin Area";
       auth_basic_user_file /etc/nginx/.htpasswd;
       proxy_pass http://127.0.0.1:7001;
   }
   ```

3. **Firewall rules**
   - Block port 7001 from external access
   - Only allow localhost connections

4. **Future: API Keys**
   - Planned feature for API authentication
   - Token-based access control

### Production Deployment

For production environments:

```rust
// Only bind to localhost
let config = AdminConfig {
    bind_addr: "127.0.0.1:7001".parse().unwrap(),
};

// Access remotely via SSH tunnel
// ssh -L 7001:localhost:7001 user@relay-server
// Then visit http://localhost:7001/admin/dashboard
```

## Troubleshooting

### Dashboard not loading
- Check admin server is running: `curl http://localhost:7001/admin/health`
- Verify bind address in configuration
- Check firewall rules

### Metrics not updating
- Check browser console for JavaScript errors
- Verify `/admin/stats` endpoint is accessible
- Check CORS configuration if accessing from different domain

### High resource usage
- Reduce refresh interval in JavaScript (default 2s)
- Consider using Prometheus scraping instead
- Implement metric aggregation

## Development

### Testing Dashboard Locally

```bash
# Start relay with admin server
cargo run --package nostr-relay --example relay_server --features full

# Visit dashboard
open http://localhost:7001/admin/dashboard
```

### Customizing Dashboard

The dashboard HTML is located at `src/dashboard.html`. Modifications:

1. Edit `src/dashboard.html`
2. Rebuild crate (uses `include_str!` macro)
3. Restart relay

### Adding New Metrics

1. Add metric to `RelayMetrics` struct
2. Update `MetricsSnapshot` struct
3. Update Prometheus export format
4. Add to `StatsResponse` in admin module
5. Update dashboard HTML to display new metric

## Future Enhancements

Planned improvements:

- [ ] Historical metrics (time-series data)
- [ ] Chart visualizations (line graphs, bar charts)
- [ ] Alert thresholds and notifications
- [ ] WebSocket for real-time push updates
- [ ] Authentication and authorization
- [ ] Multi-relay dashboard aggregation
- [ ] Export metrics to CSV/JSON
- [ ] Custom metric dashboards
- [ ] Mobile-responsive design enhancements
