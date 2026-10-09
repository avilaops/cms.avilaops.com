# syntax=docker/dockerfile:1
# Imagem do CMS: um binário só, `servidor`. Contexto `.`, linux/amd64.
# Não recebe segredo de build; o único argumento é GIT_SHA.

FROM rust:1-bookworm AS base
RUN cargo install cargo-chef --locked
WORKDIR /app

# A receita muda só quando as dependências mudam: a camada que as compila
# fica em cache entre um build e outro.
FROM base AS plano
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS construcao
# As consultas são conferidas contra o diretório `.sqlx`, sem banco.
ENV SQLX_OFFLINE=true
COPY --from=plano /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --bin servidor

FROM debian:bookworm-slim
ARG GIT_SHA=desconhecido
# `curl` é do healthcheck. O binário não usa OpenSSL nem outra biblioteca
# dinâmica além da libc.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home cms \
    && install -d -o cms -g cms /var/lib/cms/midia
COPY --from=construcao /app/target/release/servidor /usr/local/bin/servidor
ENV GIT_SHA=$GIT_SHA \
    PORTA=3090 \
    DIRETORIO_DE_MIDIA=/var/lib/cms/midia
USER cms
EXPOSE 3090
VOLUME /var/lib/cms/midia
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORTA}/api/saude" || exit 1
CMD ["servidor", "servir"]
