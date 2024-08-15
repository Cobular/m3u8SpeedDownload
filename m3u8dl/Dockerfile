# Use the official Rust image as a parent image
FROM rust:1.78 as builder

# Set the working directory in the container
WORKDIR /usr/src/app

# Copy the current directory contents into the container
COPY . .

# Build the application
RUN cargo build --release

# Use a smaller base image for the final image
FROM debian:bookworm-slim

# Install FFmpeg
RUN apt-get update && apt-get install -y ffmpeg libx264-dev ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/m3u8dl /usr/local/bin/m3u8_downloader

WORKDIR /app

# Set the binary as the entrypoint
ENTRYPOINT ["m3u8_downloader"]