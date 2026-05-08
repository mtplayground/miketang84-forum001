# miketang84-forum001

Rust forum application built with Axum, SQLx, Askama, Tailwind, and PostgreSQL.

## Runtime Environment

The application expects these environment variables at runtime:

- `DATABASE_URL`: required PostgreSQL connection string.
- `SESSION_SECRET`: required secret used for session and auth-related state.
- `BIND_ADDR`: socket address for the HTTP listener. The Docker image defaults this to `0.0.0.0:8080`.
- `RUST_LOG`: tracing filter for structured logs. The Docker image defaults this to `info`.

## Docker

The repository includes a multi-stage [`Dockerfile`](./Dockerfile) for self-hosted deployment. The final runtime image is based on `debian:bookworm-slim` and contains:

- the compiled `miketang84-forum001` binary
- the `migrations/` directory
- the Askama `templates/` directory
- the generated `static/` asset bundle

Build the image:

```bash
docker build -t miketang84-forum001 .
```

Run the container:

```bash
docker run --rm -p 8080:8080 \
  -e DATABASE_URL=postgresql://user:password@host:5432/forum001 \
  -e SESSION_SECRET=replace-with-a-long-random-secret \
  -e BIND_ADDR=0.0.0.0:8080 \
  -e RUST_LOG=info \
  miketang84-forum001
```
