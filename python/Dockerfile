# ── Builder: install deps with uv ──────────────────────────
FROM python:3.11-slim-bookworm AS builder
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv
WORKDIR /app
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev --no-install-project

# ── Runtime ────────────────────────────────────────────────
FROM python:3.11-slim-bookworm
RUN groupadd --system app && useradd --system --no-create-home --gid app app
WORKDIR /app
COPY --from=builder /app/.venv .venv
COPY main.py store.py models.py ./
COPY static/ static/
RUN mkdir -p data uploads && chown -R app:app /app
USER app
EXPOSE 8080
VOLUME ["/app/data", "/app/uploads"]
ENV PORT=8080 \
    CARDVAULT_DB=/app/data/cardvault.db \
    CARDVAULT_UPLOADS=/app/uploads \
    PATH="/app/.venv/bin:$PATH"
CMD ["python", "main.py"]
