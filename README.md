# Media service for MiSArch

_Important: Consider setting secure MinIO credentials for production use._

### Quickstart (DevContainer)

1. Open VSCode Development Container
2. `cargo run` starts the GraphQL service + GraphiQL on port `8080`

### Quickstart (Docker Compose)

1. `docker compose -f docker-compose-dev.yaml up --build` in the repository root directory. **IMPORTANT:** MinIO credentials should be configured for production.

### What it can do

1. Serves static files to any domain with Nginx
2. Features file uploads via GraphQL
3. Generates pre-signed URLs for files at a GraphQL endpoint
4. Allows configuration of the pre-signed URL domain

### How to configure the MinIO pre-signed URL domain

The media services uses a Nginx reverse proxy to make the fixed domain of MinIO pre-signed URLs configurable through environment variables.

The service parses the following environment variables:

- `MINIO_ENDPOINT`: configurable domain of the MinIO instance.
- `PATH_EXPIRATION_TIME`: validity duration of MinIO pre-signed URLs in seconds.
- `PROXY_PATH`: path to include in the pre-signed URLs
