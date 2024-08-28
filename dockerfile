# Build the Rust services (api and live)
FROM rust:alpine AS builder

WORKDIR /usr/src/app
COPY . .

RUN apk add --no-cache musl-dev pkgconfig openssl libressl-dev

# only builds default members (live and api)
RUN cargo build --release

# Build the Next.js frontend
FROM node:20-alpine AS frontend-builder

WORKDIR /usr/src/app/dash
COPY ./dash .

RUN npm install
RUN npm run build

# Final stage: API service
FROM alpine:3 AS api
COPY --from=builder /usr/src/app/target/release/api /api
CMD [ "/api" ]

# Final stage: Live service
FROM alpine:3 AS live
COPY --from=builder /usr/src/app/target/release/live /live
CMD [ "/live" ]

# Final stage: Frontend service
FROM node:20-alpine AS frontend

WORKDIR /usr/src/app/dash
COPY --from=frontend-builder /usr/src/app/dash ./

ENV PORT 4080

EXPOSE 4080
CMD [ "npm", "run", "start" ]
