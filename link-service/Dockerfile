# 构建阶段
FROM rust:1.86.0 AS builder
WORKDIR /app
COPY . .
WORKDIR /app/link-service
# 启用 sqlx 离线模式，构建时不再需要数据库连接，只依赖 .sqlx 查询缓存
ENV SQLX_OFFLINE=true
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/link-service /app/link-service
CMD ["./link-service"]
