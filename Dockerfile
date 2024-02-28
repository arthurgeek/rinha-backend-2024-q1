# Base layer with chef, lld and clang
FROM lukemathwalker/cargo-chef:latest-rust-1.75 AS chef
WORKDIR /app
RUN apt update && apt install lld clang -y

# Used to compute a lock-file
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder for local development
FROM chef AS builder-local
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json

# Now, prepare to build our application
ENV DB_HOST=db
COPY . .
RUN cargo build --release --bin rinha

# Builder for release
FROM chef AS builder-release
COPY --from=planner /app/recipe.json recipe.json

# Build our project dependencies, not our application!
ENV RUSTFLAGS="-C target-cpu=icelake-server"
RUN cargo chef cook --release --target=x86_64-unknown-linux-gnu --recipe-path recipe.json

# Now, prepare to build our application
ENV DB_HOST=/var/run/postgresql
COPY . .
RUN cargo build --release --target=x86_64-unknown-linux-gnu --bin rinha

# Runtime base for local development and release
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update -y && \
    apt-get install -y --no-install-recommends openssl ca-certificates curl && \
    apt-get autoremove -y && \
    apt-get clean -y && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash app
USER app
WORKDIR /app

# Runtime for local development
FROM runtime-base AS runtime-local

COPY --from=builder-local /app/target/release/rinha rinha

CMD ./rinha

# Runtime for release
FROM runtime-base AS runtime-release

COPY --from=builder-release /app/target/x86_64-unknown-linux-gnu/release/rinha rinha

CMD ./rinha
