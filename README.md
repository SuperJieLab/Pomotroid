# Pomotroid Fork（本仓库为 pomotroid 的 fork）

> 上游原项目：[Splode/pomotroid](https://github.com/Splode/pomotroid)
> 原版完整文档（功能、安装、主题、WebSocket API、开发指南）请见上游 README。
> **本 README 仅记录本 fork 相对上游的改动。**

## 本 Fork 改动：macOS 快捷指令联动（DND）

在番茄钟的关键节点自动调用 macOS「快捷指令」App 里你**自建**的指令；默认用来切换系统勿扰模式（DND），但两个指令名完全可配置，因此同一机制也能驱动任何其它自动化（播放白噪音、控制智能灯、写日志等）。

### 行为

| 触发节点 | 默认动作 |
| --- | --- |
| 工作轮开始 | 运行「开启指令」（默认 `Pomotroid DND On`）→ 打开系统勿扰 |
| 休息轮（短/长）开始 | 运行「关闭指令」（默认 `Pomotroid DND Off`）→ 关闭系统勿扰 |
| 完整重置（离开工作会话） | 强制运行「关闭指令」 |
| 重新开始当前轮次 | 按当前（保留的）轮次重新套用：工作轮保持开启、休息轮保持关闭 |

### 配置（设置 → 系统 → 快捷指令联动，仅 macOS 显示）

- **总开关**：自动切换勿扰模式（默认关）
- **开启指令名** / **关闭指令名**：两个文本输入框，可改指向你自建的任意快捷指令

### 前置条件与失败处理

- 你需要先在「快捷指令」App 中**手动创建**对应名称的指令（动作：设置专注 → 打开/关闭）；本 app 不内置、也不自动生成指令。
- 指令名不存在或执行失败时，弹出**系统通知**提醒，**不打断计时**；成功路径完全静默。
- 非 macOS 平台：设置项隐藏，后端直接 no-op，无任何副作用。

### 实现位置（开发者）

- 后端 `src-tauri/src/focus/`（封装 `shortcuts run` 调用，用 `Runner` trait 便于单测注入）
- 计时集成 `src-tauri/src/timer/mod.rs` 的 `Started` / `Reset` 事件（锁序：先 drop settings 再锁 sequence）
- 设置字段 `focus_dnd_enabled` / `focus_dnd_on_name` / `focus_dnd_off_name`
- 前端 `src/lib/components/settings/sections/SystemSection.svelte`（仅 macOS 显示该分组）

> 验证：`cargo test` 95 passed；`npm run check` 0 errors。

## 构建（与原版一致）

```bash
npm install
npm run tauri dev     # 开发模式（热重载）
npm run tauri build   # 生产构建
```

完整的安装、主题、WebSocket API 与贡献指南见上游：
https://github.com/Splode/pomotroid

## 许可证

MIT © [Christopher Murphy](https://github.com/Splode) —— 本 fork 沿用原项目许可证。
