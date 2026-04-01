# SDM Download Server

A high-performance download server built with Rust, running in Docker on TrueNAS Scale.

## Features

- **HTTP/HTTPS Downloads** - Download files from any URL
- **Real-time Progress** - Live progress tracking with speed indicator
- **Concurrent Downloads** - Handle multiple downloads simultaneously (configurable up to 20)
- **Batch Downloads** - Add multiple URLs from a text file
- **Web Interface** - Clean, responsive UI with filter counts
- **REST API** - Full API for automation
- **State Persistence** - Download state survives container restarts
- **Docker Ready** - Optimized multi-stage build

## Tech Stack

- **Rust** with Axum framework
- **Tokio** async runtime
- **Docker** with multi-stage build
- **TrueNAS Scale** deployment ready

## Quick Start

### Docker Compose

```yaml
services:
  sdmserver:
    image: sdmserver
    container_name: sdmserver
    restart: unless-stopped
    ports:
      - "5900:5900"
    volumes:
      - ./downloads:/app/downloads
      - ./config:/app/config
    environment:
      - DOWNLOAD_DIR=/app/downloads
      - CONFIG_DIR=/app/config
      - MAX_CONCURRENT_DOWNLOADS=3
      - REQUEST_TIMEOUT=3600
      - RUST_LOG=info
```

```bash
docker-compose up -d
```

### Build from Source

```bash
# Build Docker image
docker build -t sdmserver .

# Run
docker run -d --name sdmserver -p 5900:5900 -v $(pwd)/downloads:/app/downloads sdmserver
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 5900 | Server port |
| `DOWNLOAD_DIR` | /app/downloads | Download storage directory |
| `CONFIG_DIR` | /app/config | Configuration and state directory |
| `MAX_CONCURRENT_DOWNLOADS` | 3 | Maximum simultaneous downloads (1-20) |
| `REQUEST_TIMEOUT` | 3600 | HTTP request timeout in seconds |
| `RUST_LOG` | info | Log level |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/` | Web interface |
| GET | `/api/health` | Health check |
| GET | `/api/downloads` | List all downloads |
| POST | `/api/downloads` | Start new download |
| POST | `/api/downloads/batch` | Add multiple URLs (one per line) |
| GET | `/api/downloads/:id` | Get download status |
| DELETE | `/api/downloads/:id` | Remove from list |
| DELETE | `/api/downloads/:id/file` | Remove download and file |
| POST | `/api/downloads/:id/cancel` | Cancel download |
| POST | `/api/downloads/:id/retry` | Retry failed download |
| GET | `/api/settings` | Get current settings |
| POST | `/api/settings` | Update settings |

### Create Download

```bash
curl -X POST http://localhost:5900/api/downloads \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com/file.zip"}'
```

### Batch Download

```bash
curl -X POST http://localhost:5900/api/downloads/batch \
  -H "Content-Type: text/plain" \
  --data-binary @urls.txt
```

### List Downloads

```bash
curl http://localhost:5900/api/downloads
```

### Cancel Download

```bash
curl -X POST http://localhost:5900/api/downloads/{id}/cancel
```

### Retry Failed Download

```bash
curl -X POST http://localhost:5900/api/downloads/{id}/retry
```

### Delete Download

```bash
curl -X DELETE http://localhost:5900/api/downloads/{id}
```

### Update Settings

```bash
curl -X POST http://localhost:5900/api/settings \
  -H "Content-Type: application/json" \
  -d '{"max_concurrent": 5}'
```

## Web Interface

The web UI provides:
- Add downloads via URL or batch file
- Real-time progress with speed indicators
- Filter by status (All, Queued, Downloading, Completed, Failed)
- Count display for each filter
- Cancel, retry, and delete actions
- Settings panel for concurrent download limit

## State Persistence

Download state is saved to `CONFIG_DIR/downloads.json`. On restart:
- Queued and Failed downloads are restored
- Completed downloads remain in the list for reference

Cancelled downloads are NOT restored (removed from state on cancellation).

## TrueNAS Scale Deployment

1. Create a dataset `sdmserver` for persistence
2. Use TrueNAS Scale's Docker management or Portainer
3. Map volumes:
   - `/mnt/pool/sdmserver/downloads` → `/app/downloads`
   - `/mnt/pool/sdmserver/config` → `/app/config`
4. Set appropriate permissions (UID 1000 or match container user)

## Build

```bash
# Development build
cargo build

# Production build
cargo build --release

# Docker build
docker build -t sdmserver .
```

## Project Structure

```
sdmserver/
├── src/
│   ├── main.rs          # Entry point
│   ├── api/             # API routes
│   │   ├── mod.rs
│   │   └── download.rs
│   ├── models/          # Data structures
│   │   ├── mod.rs
│   │   └── download.rs
│   ├── services/        # Download logic
│   │   ├── mod.rs
│   │   └── downloader.rs
│   └── state.rs         # App state
├── static/              # Web assets
│   ├── index.html
│   ├── app.js
│   └── style.css
├── Dockerfile           # Multi-stage build
├── docker-compose.yaml  # Deployment config
└── README.md
```

## License

MIT
