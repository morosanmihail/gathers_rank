FROM rust:1-bookworm AS builder
WORKDIR /app

# Cache dependency compilation separately from source.
# Dummy main so cargo can compile all deps without our source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release \
    && rm -f target/release/deps/league_scoring* target/release/league_scoring

# Build the real binary.
# Migrations are embedded at compile time by sqlx::migrate!
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

# ── Runtime image ─────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/league_scoring .
COPY static ./static

EXPOSE 3000
ENV PORT=3000
ENV DATABASE_URL=sqlite:/app/data/league.db
ENV RUST_LOG=info

# Data volume for the SQLite DB file
VOLUME ["/app/data"]

CMD ["./league_scoring"]
