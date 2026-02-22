# ── Build ──────────────────────────────────────────────────
FROM golang:1.21-alpine AS builder
WORKDIR /build
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o cardvault .

# ── Runtime ────────────────────────────────────────────────
FROM alpine:3.20
RUN addgroup -S app && adduser -S app -G app
WORKDIR /app
COPY --from=builder /build/cardvault ./
RUN mkdir -p data uploads && chown -R app:app /app
USER app
EXPOSE 8080
VOLUME ["/app/data", "/app/uploads"]
ENV PORT=8080 \
    CARDVAULT_DB=/app/data/cardvault.db \
    CARDVAULT_UPLOADS=/app/uploads
CMD ["./cardvault"]
