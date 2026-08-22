# API Relay Monitor

一个轻量的 Windows 桌面悬浮监控工具，用于实时查看 API 中转站的余额、消耗、请求次数和最近的使用日志。

基于 Tauri v2 + React + TypeScript + Rust，体积小、常驻系统托盘、可悬浮置顶。

## 功能

- 桌面悬浮窗，可拖动、可置顶
- **拖到屏幕边缘自动收起成抽屉**，鼠标靠近边缘再滑出（带动画）
- **多中转站管理**：可配置多个中转站，悬浮窗点标题一键循环切换
- 系统托盘菜单（显示 / 刷新 / 设置 / 退出）
- 当前余额 / 历史消耗 / 请求次数
- 最近请求日志（模型名、token 用量、消费金额）
- 消费曲线图（最近 20 次请求的消费趋势）
- 每个中转站独立的历史记录与消费曲线（互不干扰）
- 支持两类适配器：
  - **New API / One API 系**（Cookie + 可选 New-Api-User）
  - **极智 API（jizhiapi.site）**（Bearer Token 认证）
- 低余额系统通知
- Windows 开机自启动
- 历史记录本地持久化（每站最多 200 条）

## 适配说明

项目内置多个 **Provider 适配器**：

| Provider | 文件 | 认证方式 | 接口 |
| --- | --- | --- | --- |
| `mock` | `provider/mock.ts` | 无 | 本地假数据 |
| `xiaoma`（New API / One API 系） | `provider/xiaoma.ts` + `xiaoma_fetch` | Cookie + New-Api-User | `/api/user/self`、`/api/log/self` |
| `jizhi`（极智 API） | `provider/jizhi.ts` + `jizhi_fetch` | Bearer Token | `/api/v1/auth/me`、`/api/v1/usage` |

不同中转站的接口路径、认证头、字段名可能不一致。如果你的中转站不兼容任一现有适配器，请按下文「修改 / 新增 Provider」自行调整。

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

打开悬浮窗右上角设置图标，或托盘菜单 → 设置。

### 中转站管理

设置页顶部「中转站管理」区可以添加多个中转站，每个中转站是一份独立配置：

- 点「➕ 添加中转站」新增一个配置
- 点配置按钮切换编辑对象；带 ✓ 的是当前正在监控的
- 「设为当前监控」把编辑中的配置设为激活（保存后生效）
- 「删除此中转站配置」移除配置（至少保留一个）
- 悬浮窗标题栏点名称或 ⇄ 图标，可在多个中转站间循环切换

### 每个中转站的字段

| 字段 | 说明 |
| --- | --- |
| 配置名称 | 自定义显示名 |
| 适配器类型 | `mock` / New API 系（Cookie）/  API（Token） |
| 中转站地址 | 例如 `https://xxx.best`、`https://xxx.site` |
| Cookie | New API 系：从浏览器复制，格式 `session=xxx`；极智可留空 |
| API Token | 极智：粘贴 `Authorization: Bearer` 后的 JWT（必填）；New API 系可选 |
| New-Api-User | 仅 New API / One API 系需要（极智不显示） |

### 全局设置

| 字段 | 说明 |
| --- | --- |
| 自动刷新间隔 | 默认 60 秒，0 表示不自动刷新 |
| 低余额阈值 | 默认 $5 |
| 系统通知 | 余额低于阈值时弹窗 |
| 窗口置顶 / 开机启动 | 按需开启 |

Cookie 格式示例（New API 系）：

```text
session=xxx; token=yyy
```

极智 API Token 获取：F12 → 网络 → 打开 `/api/v1/auth/me` 请求 → 请求标头 → 复制 `Authorization: Bearer` 后面那一长串 JWT。

## 修改 / 新增 Provider

如果现有适配器不能解析你的中转站接口，可以改现有的或新增一个：

**新增一个 Provider 的步骤：**

1. **Rust 侧** — 在 [src-tauri/src/commands.rs](src-tauri/src/commands.rs) 参照 `xiaoma_fetch` / `jizhi_fetch` 新写一个 `xxx_fetch` command，处理接口路径、认证头、字段映射；在 [src-tauri/src/lib.rs](src-tauri/src/lib.rs) 的 `invoke_handler` 注册。
2. **前端 Provider** — 在 [src/renderer/services/provider/](src/renderer/services/provider/) 新建 `xxx.ts`，参照 `jizhi.ts` 调用你的 command；在 `index.ts` 的 `getProvider` 注册；在 [types.ts](src/renderer/services/../types.ts) 的 `ProviderType` 加上类型。
3. **设置页** — 在 [SettingsWindow.tsx](src/renderer/components/SettingsWindow.tsx) 的适配器类型下拉里加一个 `<option>`。

**关键字段参考（New API 系）：**

- 接口路径：`/api/user/self`、`/api/log/self`
- 认证头：`Cookie` / `Authorization: Bearer` / `New-Api-User`
- 余额字段：`quota` / `remain_quota` / `used_quota` / `request_count`
- quota → USD 换算系数 `QUOTA_TO_USD`（默认 500_000）

## 数据存储

配置、历史记录仅保存在本机：

- Windows: `%APPDATA%\com.apimonitor.app\`
  - `settings.json` — 配置（含所有中转站的 Cookie / Token，仅本机）
  - `history_<profileId>.json` — 每个中转站独立的历史记录（各最多 200 条）

不会上传到任何服务器。

## 安全说明

- Cookie / API Token 等价于登录凭证，仅保存在本机，不要分享或提交到 Git。
- 项目 `.gitignore` 已排除 `settings.json` / `history.json` / `.env` 等本地配置文件，开源前请再次确认仓库中不包含真实凭据。

## 项目结构

```text
src/
  renderer/
    App.tsx                  # 路由：?page=floating / ?page=settings
    types.ts                 # 数据模型（StationProfile / AppSettings）与默认设置
    components/
      FloatingWindow.tsx     # 悬浮窗（含边缘吸附抽屉、中转站切换）
      SettingsWindow.tsx     # 设置页（含多中转站管理）
      TrendChart.tsx         # Canvas 趋势折线图
    services/
      balanceService.ts      # 拉取余额、自动刷新（按激活 profile）
      historyStore.ts        # 历史记录（按 profileId 分开存）
      alertService.ts        # 低余额通知
      tauriAPI.ts            # Tauri command 包装
      provider/
        base.ts              # Provider 接口
        index.ts             # Provider 工厂（按 providerType 选择）
        mock.ts              # MockProvider（假数据）
        xiaoma.ts            # New API / One API 系适配器
        jizhi.ts             # 极智 API 适配器
src-tauri/
  src/
    lib.rs                   # 入口、托盘、窗口
    commands.rs              # Tauri command（xiaoma_fetch / jizhi_fetch / 窗口 / 历史）
    store.rs                 # 多 profile 设置、分站历史持久化、旧格式迁移
    credential_store.rs      # Cookie 本地存储
  tauri.conf.json
  Cargo.toml
```

## 协议

[MIT](LICENSE)
