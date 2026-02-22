# ── Build ──────────────────────────────────────────────────
FROM rust:1-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /build

# Cache dependencies — dummy binary so dep layers survive source changes
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src/

# Build the real binary (deps are already compiled above)
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# ── Runtime ────────────────────────────────────────────────
FROM alpine:3.20
RUN addgroup -S app && adduser -S app -G app
WORKDIR /app
COPY --from=builder /build/target/release/cardvault ./
RUN mkdir -p data uploads && chown -R app:app /app
USER app
EXPOSE 8080
VOLUME ["/app/data", "/app/uploads"]
ENV PORT=8080 \
    CARDVAULT_DB=/app/data/cardvault.db \
    CARDVAULT_UPLOADS=/app/uploads
CMD ["./cardvault"]
