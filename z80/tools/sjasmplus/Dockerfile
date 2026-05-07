# Stage 1: build sjasmplus from source
FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    cmake \
    make \
    g++ \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

RUN git clone --depth 1 --recurse-submodules https://github.com/z00m128/sjasmplus.git .

RUN cmake -B build -DCMAKE_BUILD_TYPE=Release \
    && cmake --build build --parallel $(nproc)

# Stage 2: minimal runtime image
FROM debian:bookworm-slim

COPY --from=builder /src/build/sjasmplus /usr/local/bin/sjasmplus

ENTRYPOINT ["sjasmplus"]
