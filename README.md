# SDM Download Server

A high-performance download server built with Rust, running in Docker on TrueNAS Scale.

## Features

- **HTTP/HTTPS Downloads** - Download files from any URL
- **Real-time Progress** - Live progress tracking with speed indicator
- **Concurrent Downloads** - Handle multiple downloads simultaneously (configurable)
- **Web Interface** - Clean, responsive UI
- **REST API** - Full API for automation
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
      - MAX_CONCURRENT_DOWNLOADS=3
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
| `MAX_CONCURRENT_DOWNLOADS` | 3 | Maximum simultaneous downloads |
| `RUST_LOG` | info | Log level |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/` | Web interface |
| GET | `/api/health` | Health check |
| GET | `/api/downloads` | List all downloads |
| POST | `/api/downloads` | Start new download |
| GET | `/api/downloads/:id` | Get download status |
| DELETE | `/api/downloads/:id` | Delete download |
| POST | `/api/downloads/:id/cancel` | Cancel download |

### Create Download

```bash
curl -X POST http://localhost:5900/api/downloads \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com/file.zip"}'
```

### List Downloads

```bash
curl http://localhost:5900/api/downloads
```

### Delete Download

```bash
curl -X DELETE http://localhost:5900/api/downloads/{id}
```

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
│   ├── models/          # Data structures
│   ├── services/        # Download logic
│   └── state.rs         # App state
├── static/              # Web assets
├── Dockerfile           # Multi-stage build
├── docker-compose.yaml  # Deployment config
└── README.md
```

## License

MIT
