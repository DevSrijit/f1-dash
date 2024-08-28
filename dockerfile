# Build the Rust services (api and live)
FROM rust:alpine AS builder

WORKDIR /usr/src/app
COPY . .

RUN apk add --no-cache musl-dev pkgconfig openssl libressl-dev

# only builds default members (live and api)
RUN cargo b -r

# Build the Next.js frontend
FROM node:20-alpine AS frontend-builder

WORKDIR /usr/src/app/dash
COPY ./dash .

RUN npm install
RUN npm run build

# API service
FROM alpine:3 as api
COPY --from=builder /usr/src/app/target/release/api /api
CMD [ "/api" ]

# Live service
FROM alpine:3 as live
COPY --from=builder /usr/src/app/target/release/live /live
CMD [ "/live" ]

# Frontend service
FROM alpine:3 as frontend
RUN apk add --no-cache nodejs npm

WORKDIR /usr/src/app/dash
COPY --from=frontend-builder /usr/src/app/dash ./

ENV PORT 4080

EXPOSE 4080
CMD [ "npm", "run", "start" ]
