# Download Server - Agent Instructions

## Project Overview

Build a high-performance download server in Rust that runs in Docker on TrueNAS Scale. The server provides a web interface and API for downloading files via HTTP/HTTPS URLs and torrents.

## Tech Stack

- **Language**: Rust (latest stable)
- **Web Framework**: Axum (async, performant, type-safe)
- **Async Runtime**: Tokio
- **HTTP Client**: reqwest (for downloading), torrent library for torrent support
- **Frontend**: Simple HTML/CSS/JS (no heavy framework needed)
- **Container**: Docker with multi-stage build for minimal image size

## Project Structure

```
sdmserver/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, server setup
│   ├── api/
│   │   ├── mod.rs
│   │   ├── download.rs      # Download endpoints
│   │   └── torrent.rs       # Torrent management endpoints
│   ├── models/
│   │   ├── mod.rs
│   │   └── download.rs      # Data structures
│   ├── services/
│   │   ├── mod.rs
│   │   ├── downloader.rs    # HTTP download logic
│   │   └── torrent_service.rs
│   └── state.rs             # App state management
├── static/
│   ├── index.html           # Web UI
│   ├── style.css
│   └── app.js
├── Dockerfile
├── docker-compose.yaml
└── agents.md
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Web UI |
| GET | `/api/downloads` | List all downloads |
| POST | `/api/downloads` | Start new download (body: `{"url": "..."}`) |
| GET | `/api/downloads/:id` | Get download status |
| DELETE | `/api/downloads/:id` | Cancel/delete download |
| POST | `/api/torrents` | Add torrent file or magnet |
| GET | `/api/torrents` | List active torrents |
| DELETE | `/api/torrents/:id` | Remove torrent |
| GET | `/api/health` | Health check |

## Download Status Model

```rust
struct Download {
    id: Uuid,
    url: String,
    filename: String,
    total_bytes: u64,
    downloaded_bytes: u64,
    status: DownloadStatus, // Queued, Downloading, Paused, Completed, Failed
    created_at: DateTime<Utc>,
    file_path: Option<PathBuf>,
}
```

## Docker Configuration

### Dockerfile
- Use `rust:1.75-slim` for build stage
- Use `debian:bookworm-slim` for runtime (smaller than alpine, better compatibility)
- Multi-stage build to minimize image size (< 50MB target)
- Non-root user for security
- Health check configured

### docker-compose.yaml
```yaml
version: '3.8'
services:
  sdmserver:
    build: .
    container_name: sdmserver
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./downloads:/app/downloads
      - ./config:/app/config
    environment:
      - DOWNLOAD_DIR=/app/downloads
      - CONFIG_DIR=/app/config
      - MAX_CONCURRENT_DOWNLOADS=3
      - REQUEST_TIMEOUT=3600
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 5900 | Server port |
| `DOWNLOAD_DIR` | /app/downloads | Download output directory |
| `CONFIG_DIR` | /app/config | Configuration and state file directory |
| `MAX_CONCURRENT_DOWNLOADS` | 3 | Maximum concurrent downloads |
| `REQUEST_TIMEOUT` | 3600 | HTTP request timeout in seconds |
| `RUST_LOG` | info | Logging level |

## State Persistence

Download state is persisted to `CONFIG_DIR/downloads.json`. This allows:
- Downloads with status `Queued` or `Failed` to be restored on restart
- Resume interrupted downloads after container restart
- State is automatically saved on every download modification

## TrueNAS Scale Deployment Notes

1. Create a dataset `sdmserver` for persistence
2. Use TrueNAS Scale's Docker management or Portainer
3. Map volumes:
   - `/mnt/pool/sdmserver/downloads` -> `/app/downloads`
   - `/mnt/pool/sdmserver/config` -> `/app/config`
4. Set appropriate permissions (UID 1000 or match container user)
5. State is automatically persisted to `config/downloads.json`

## Implementation Priorities

1. Core HTTP download with progress tracking
2. Web UI with real-time status updates (SSE or polling)
3. Torrent support with DHT and peer exchange
4. File browser and download management
5. Authentication (optional, basic auth or token)

## Non-Functional Requirements

- Memory usage < 100MB idle
- Handle 10+ concurrent downloads
- Graceful shutdown handling
- Structured logging with tracing
- Configurable via environment variables

## Build Commands

```bash
# Development
cargo build

# Production build
cargo build --release

# Build Docker image
docker build -t sdmserver .

# Run with docker-compose
docker-compose up -d
```
