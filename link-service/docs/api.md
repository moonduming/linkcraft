# Link Service API 文档

该文档描述 `link-service` 暴露的 HTTP 接口。所有示例以 `https://api.example.com` 为基准，可根据 `AppConfig.addr` 调整。

## 认证与限流

- `POST /shorten`、`GET /links`、`POST /delete`、`GET /stats` 需要请求头 `Authorization: Bearer <JWT>`，JWT 会用 `cfg.jwt_secret` 校验，并在 Redis 中验证 `session:{jti}` 是否存在。
- 受保护接口在 `user_rate_limiter` 中按 `user_rate_limit`/`user_rate_limit_window` 做用户级限流。
- 公共重定向接口 `/s/{short_code}` 只做 IP 限流（`ip_rate_limit`/`ip_rate_limit_window`）。真实 IP 来自 `X-Forwarded-For`、`X-Real-IP` 或连接地址。

## 通用错误

所有接口失败时返回 JSON `{"error": "原因"}`，HTTP 状态码参见下方说明：

- `400 Bad Request`：参数格式/范围错误，自定义短码冲突，本地时间无法映射等
- `401 Unauthorized`：缺少或无效的 JWT
- `404 Not Found`：短码不存在、已过期或不属于当前用户
- `429 Too Many Requests`：触发用户或 IP 限流
- `500 Internal Server Error`：数据库、Redis 或后台任务内部错误

## 接口列表

### GET /

- 描述：健康检查，返回 `"Hello, World!"`。
- 认证：无。

### POST /shorten

- 描述：创建短链。
- 认证：需要。
- Request `application/json`（`ShortlinkCreateReq`）：
  ```json
  {
    "url": "https://long.example.com/path",
    "ttl": 86400,
    "short_code": "myalias"
  }
  ```
  - `url`：必须是合法 URL。
  - `ttl`：可选，秒数，必须在 `[shortlink_min_ttl, shortlink_max_ttl]`。
  - `short_code`：可选，自定义短码，若冲突返回 400。
- Response `200 OK`：
  ```json
  { "short_url": "https://api.example.com/s/abc123" }
  ```

### GET /s/{short_code}

- 描述：短链跳转，公共接口。
- Path：`short_code`。
- 行为：先查 Redis 命中则直接返回，miss 时回源 MySQL 并按剩余 TTL 判断是否缓存；总是异步记录点击与访问日志。
- Response：`302 Found`/`307 Temporary Redirect`（Axum `Redirect`），`Location` 指向长链。
- 常见错误：`404`（不存在或过期）、`429`（IP 限流）。

### GET /links

- 描述：分页查询当前用户的短链。
- 认证：需要。
- Query（`LinkQuery`）：
  | 参数 | 类型 | 说明 |
  | --- | --- | --- |
  | `short_code` | `String` | 模糊匹配（内部自动 `%keyword%`） |
  | `long_url` | `String` | 模糊匹配 |
  | `click_count` | `u64` | 精确匹配点击数 |
  | `date_from` / `date_to` | `NaiveDateTime` | 以客户端所在 `timezone` 的本地时间传入，服务端会转换为 UTC 过滤 |
  | `timezone` | `String` | IANA 时区，默认 `UTC`，校验失败返回 400 |
  | `limit` | `u64` | 1~100，默认 10 |
  | `offset` | `u64` | 默认 0 |
- Response `200 OK`：
  ```json
  {
    "links": [
      {
        "id": 1,
        "user_id": 42,
        "short_code": "abc123",
        "long_url": "https://long.example.com",
        "click_count": 37,
        "expire_at": "2024-05-01 12:00:00",
        "created_at": "2024-04-01 12:00:00"
      }
    ],
    "count": 17
  }
  ```
  `expire_at`/`created_at` 会用 `timezone` 转换后返回。

### POST /delete

- 描述：批量删除当前用户的短链，同时清理 Redis 缓存、点击计数以及访问日志。
- 认证：需要。
- Request `application/json`：`{"ids": [1, 2, 3]}`（长度 1~50）。
- Response：`204 No Content`（实现返回 `Ok(())`，可视情况映射为 200/204）。

### GET /stats

- 描述：按天统计短链访问量。
- 认证：需要。
- Query（`LinkStatsQuery`）：
  | 参数 | 类型 | 说明 |
  | --- | --- | --- |
  | `short_code` | `String` | 必填，目标短码 |
  | `days` | `u8` | 默认 30，必须 ≥1 且 ≤ `max_stats_days` |
  | `timezone` | `String` | 默认 `UTC`，用于把访问日志按本地日汇总 |
- Response `200 OK`：JSON 数组，元素形如 `{"0":"2024-04-01","1":37}`（Axum 默认序列化 `Vec<(String,i64)>`），按日期升序排列，缺口自动补 0。

## 后台任务

- Redis 队列 `BackgroundJob` 负责记录访问日志、同步点击量、写入缓存和删除过期短链。若待处理任务过多，会在日志中记录 `bg_jobs_tx try_send failed` 的警告。

## 版本

- 文档生成时间：2025-11-14（请根据需要更新）。
