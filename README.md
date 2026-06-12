# API Relay Monitor

一个轻量的 Windows 桌面悬浮监控工具，用于实时查看 API 中转站的余额、消耗、请求次数和最近的使用日志。

基于 Tauri v2 + React + TypeScript + Rust，体积小、常驻系统托盘、可悬浮置顶。

## 功能

- 桌面悬浮窗，可拖动、可置顶
- 系统托盘菜单（显示 / 刷新 / 设置 / 退出）
- 当前余额 / 历史消耗 / 请求次数
- 最近请求日志（模型名、token 用量、消费金额）
- 消费曲线图（最近 20 次请求的消费趋势）
- Cookie / API Token 配置
- 可选 New-Api-User 请求头（New API / One API 系站点常用）
- 低余额系统通知
- Windows 开机自启动
- 历史记录本地持久化（最多 200 条）

## 截图

> 自行运行后查看，或在仓库 Issue / Discussion 中分享。

## 适配说明

项目内置一个**示例 Provider**（`src/renderer/services/provider/xiaoma.ts` + `src-tauri/src/commands.rs` 中的 `xiaoma_fetch`），按 New API / One API 兼容站点的常见字段实现。

不同 API 中转站的接口路径、认证头、字段名可能不一致。如果你的中转站不兼容，请按下文「修改 Provider」自行调整。

## 快速开始

需要：

- Node.js 18+
- Rust（`rustup`，stable toolchain）
- Windows 10/11（其他平台未测试）

```powershell
npm install
npm run dev
```

`npm run dev` 会启动 Vite 开发服务器（5174）和 Tauri 主进程，悬浮窗会出现在屏幕右上角。第一次启动 Provider 默认是 `mock`，显示假数据。

## 构建安装包

```powershell
npm run build
```

NSIS 安装包会输出到 `src-tauri/target/release/bundle/nsis/`。

## 配置

打开悬浮窗右上角设置图标，或托盘菜单 → 设置：

| 字段 | 说明 |
| --- | --- |
| Provider | `mock`（本地假数据）或示例适配器 |
| 中转站地址 | 例如 `https://example.com` |
| Cookie | 从浏览器复制，格式 `session=xxx` |
| API Token | 可选，Bearer Token |
| New-Api-User | 可选，仅 New API / One API 系站点需要 |
| 自动刷新间隔 | 默认 60 秒，0 表示不自动刷新 |
| 低余额阈值 | 默认 $5 |
| 系统通知 | 余额低于阈值时弹窗 |
| 窗口置顶 / 开机启动 | 按需开启 |

Cookie 格式示例：

```text
session=xxx; token=yyy
```

## 修改 Provider

如果默认示例适配器不能解析你的中转站接口，需要改两处：

1. **Rust 侧请求和字段映射** — [src-tauri/src/commands.rs](src-tauri/src/commands.rs) 中的 `xiaoma_fetch` 函数，主要包括：
   - 接口路径：`/api/user/self`、`/api/log/self`
   - 认证头：`Cookie` / `Authorization: Bearer` / `New-Api-User`
   - 余额字段：`quota` / `remain_quota` / `used_quota` / `request_count`
   - 日志字段：`prompt_tokens` / `completion_tokens` / `model_name` / `quota` 等
   - quota → USD 换算系数 `QUOTA_TO_USD`（默认 500_000）

2. **前端 Provider 包装** — [src/renderer/services/provider/xiaoma.ts](src/renderer/services/provider/xiaoma.ts)，通常不需要改。

文件名 `xiaoma` 仅是历史命名，可以保留也可以重命名为你的 Provider 名。

## 数据存储

历史记录、设置、Cookie 仅保存在本机：

- Windows: `%APPDATA%\com.apimonitor.app\`
  - `settings.json` — 配置（含 Cookie，仅本机）
  - `history.json` — 历史记录（最多 200 条）
  - `session.json` — 会话 Cookie

不会上传到任何服务器。

## 安全说明

- Cookie / API Token 等价于登录凭证，仅保存在本机，不要分享或提交到 Git。
- 项目 `.gitignore` 已排除 `settings.json` / `history.json` / `.env` 等本地配置文件，开源前请再次确认仓库中不包含真实凭据。

## 项目结构

```text
src/
  renderer/
    App.tsx                  # 路由：?page=floating / ?page=settings
    types.ts                 # 数据模型与默认设置
    components/
      FloatingWindow.tsx     # 悬浮窗
      SettingsWindow.tsx     # 设置页
      TrendChart.tsx         # Canvas 趋势折线图
    services/
      balanceService.ts      # 拉取余额、自动刷新
      historyStore.ts        # 历史记录（走 Tauri command）
      alertService.ts        # 低余额通知
      tauriAPI.ts            # Tauri command 包装
      provider/
        base.ts              # Provider 接口
        index.ts             # Provider 工厂
        mock.ts              # MockProvider（假数据）
        xiaoma.ts            # 示例 Provider
src-tauri/
  src/
    lib.rs                   # 入口、托盘、窗口
    commands.rs              # Tauri command（含 xiaoma_fetch）
    store.rs                 # 设置和历史持久化
    credential_store.rs      # Cookie 本地存储
  tauri.conf.json
  Cargo.toml
```

## 协议

[MIT](LICENSE)
