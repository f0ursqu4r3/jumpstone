# Environment Configuration Guide

This guide explains how to set up local environment variables for the OpenGuild project.

## Quick Start

1. **Backend Setup:**

   ```bash
   cd backend
   cp .env.example .env
   # Edit .env with your configuration
   ```

2. **Frontend Setup:**

   ```bash
   cd frontend  
   cp .env.example .env
   # Edit .env with your configuration
   ```

## Database Setup

Before running the backend, you need a PostgreSQL database:

```bash
# Using Docker:
docker run -d \
  --name openguild-postgres \
  -e POSTGRES_USER=openguild \
  -e POSTGRES_PASSWORD=openguild \
  -e POSTGRES_DB=openguild \
  -p 5432:5432 \
  postgres:16

# Or use the docker-compose setup:
cd deploy
docker-compose up -d postgres
```

## Backend Configuration

### Required Variables

- `DATABASE_URL`: PostgreSQL connection string
- `OPENGUILD_SERVER__SESSION__SIGNING_KEY`: JWT signing key (generate with `openssl rand -base64 32`)

### Optional Services

The backend can work with these optional services:

- **MinIO/S3** for file storage (MEDIA_* variables)
- **NATS** for messaging (NATS_URL)
- **Metrics** for monitoring (METRICS_* variables)

## Frontend Configuration

The frontend needs to know where to reach the backend API:

- `NUXT_PUBLIC_API_BASE_URL`: Backend HTTP API URL
- `NUXT_PUBLIC_WS_BASE_URL`: Backend WebSocket URL

## Running the Services

1. **Backend:**

   ```bash
   cd backend
   cargo run --bin openguild-server
   ```

2. **Frontend:**

   ```bash
   cd frontend
   npm run dev
   ```

## Development vs Production

### Development

- Use `.env` files for local configuration
- Enable debug logging (`RUST_LOG=debug`)
- Use HTTP URLs for local development

### Production

- Use environment variables or secure secret management
- Reduce log levels (`RUST_LOG=info`)
- Use HTTPS URLs
- Generate secure signing keys
- Configure proper database credentials

## Security Notes

- **Never commit `.env` files** - they contain secrets
- **Generate unique signing keys** for each environment
- **Use strong database passwords** in production
- **Enable HTTPS** in production environments
