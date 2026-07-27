# 设计方案：Pomotroid × macOS 勿扰模式（DND）联动

> 日期：2026-07-24
> 状态：已通过 Brainstorm 确认（待实施）
> 上游：`Splode/pomotroid` v1.7.1，MIT，Svelte 5 + Vite + Tauri 2

## 目标

在 Pomotroid 基础上新增功能：**番茄钟轮次切换时自动切换 macOS 勿扰模式（DND）**。

- Work 轮开始 → 开启 DND
- 休息轮（Short/Long Break）开始 → 关闭 DND
- 用户可在设置中开关此功能，并可自定义快捷指令名

## 关键决策（Brainstorm 确认）

| 决策点 | 结论 |
|--------|------|
| 切换目标 | macOS **勿扰模式 (DND)**，非 Work 专注模式 |
| 平台范围 | **仅 macOS 生效**；Windows/Linux 隐藏该设置项（UI 层 + controller no-op 双保险） |
| 快捷指令命名 | **可配置**：设置里两个输入框，默认 `"Pomotroid DND On"` / `"Pomotroid DND Off"` |
| 失败处理 | 发**系统通知**提醒失败，**不打断计时**；成功路径静默 |
| 测试策略 | **Rust 单元测试（mock CLI）+ 真机手动验收**；CI 不跑真实 `shortcuts` |

## 第 1 段：架构与模块

新增/改动文件：

```
src-tauri/src/
├── focus/
│   ├── mod.rs        ← 新建。FocusController，封装 shortcuts CLI 调用
│   └── error.rs      ← 新建。失败类型 + 通知映射
├── timer/mod.rs      ← 改动。在轮次切换 & 手动开始时触发
├── settings/
│   ├── defaults.rs   ← 改动。新增 3 个字段
│   └── mod.rs        ← 改动。字段读写
└── commands.rs       ← 改动（可选）。暴露"测试指令"命令供手测
```

`FocusController` 核心抽象（CLI 调用集中此处，便于单测注入）：

```rust
pub struct FocusController {
    enabled: bool,
    on_shortcut: String,   // 默认 "Pomotroid DND On"
    off_shortcut: String,  // 默认 "Pomotroid DND Off"
}
impl FocusController {
    pub fn new(enabled, on, off) -> Self
    pub fn set_dnd(&self, on: bool) -> Result<(), FocusError>  // true=开
}
```

平台判断：`std::env::consts::OS == "macos"`。非 macOS 时 controller 构造即标记 disabled，`set_dnd` 直接返回 `Ok(())` 当 no-op；UI 层也据此隐藏开关。

## 第 2 段：数据流与触发点

触发时机（2 个入口，逻辑只有一处真相 —— "当前轮次是否 Work"）：

```rust
// timer/mod.rs — listen_events 循环中
match event {
    TimerEvent::Complete(round) => {
        let next = sequence.next(round);
        focus.set_dnd(matches!(next, RoundType::Work))?;
    }
    TimerEvent::Started(round) => {
        focus.set_dnd(matches!(round, RoundType::Work))?;
    }
}
```

DND 状态绑定「当前轮次是否 Work」，无论自动流转还是手动 Start 都走同一判定。

**实施顺序（关键链路先验证）**：

1. **Phase 0 — 链路验证**：先单独确认 `shortcuts run "Pomotroid DND On"` 真能切换 DND + 读返回码。CLI 链路通了再往下。
2. **Phase 1**：实现 `FocusController` + 单测（mock `Command`）。
3. **Phase 2**：接进 `timer/mod.rs`（仅 macOS 触发）。
4. **Phase 3**：settings 字段 + 读写。
5. **Phase 4**：前端开关 UI。
6. **Phase 5**：手测全流程。

## 第 3 段：错误处理的设置 schema

Settings 新增字段（`settings/defaults.rs`）：

```rust
pub struct Settings {
    // ... 已有字段 ...
    pub focus_dnd_enabled: bool,    // 默认 false，向后兼容
    pub focus_dnd_on_name: String,  // 默认 "Pomotroid DND On"
    pub focus_dnd_off_name: String, // 默认 "Pomotroid DND Off"
}
```

错误类型（`focus/error.rs`）：

```rust
pub enum FocusError {
    Disabled,                    // 功能关，不报错
    NotMacOS,                   // 非 macOS，静默 no-op
    ShortcutNotFound(String),    // shortcuts run 返回非 0（指令不存在/名字错）
    CliFailed(std::io::Error),  // 连 CLI 都起不来
}
```

失败处理：`set_dnd` 返回 `Err` 时，timer 层 catch 住，走 Tauri notification 插件弹一条「专注模式切换失败，请检查快捷指令 <name> 是否存在」。成功路径完全静默。功能关 / 非 macOS → `set_dnd` 直接 `return Ok(())`，零副作用。

前端设置页形态：
- 一个总开关「自动切换勿扰模式」
- 开关下两个文本输入框（指令名），带 placeholder 显示默认名
- 非 macOS 时整组隐藏

## 第 4 段：测试与落地

单元测试（可进 CI）：

```rust
// focus/mod.rs 中把 CLI 调用抽成 trait，便于注入
trait Runner { fn run(&self, name: &str) -> std::io::Result<bool>; }
// 真实实现 = std::process::Command；测试实现 = 返回预设结果
// 验证：enabled=false→不调用；非macOS→不调用；
//       名字错误→返回 Err(ShortcutNotFound)；成功路径→调用一次
```

计时集成用 `#[cfg(test)]` 逻辑测试，mock 掉 `FocusController`，验证「Work 轮→开 / 其余→关」在 `Complete` 和 `Started` 两入口都正确。

手动验收清单（真机 macOS）：
1. 创建两个快捷指令（Set Focus → DND On/Off），名字与设置一致
2. 开总开关，开始 Work 轮 → 系统进入勿扰
3. 切到休息轮 → 勿扰关闭
4. 改错指令名 → 收到失败通知，计时不受影响
5. 关总开关 → 轮次切换不再触发任何 DND 变化

落地建议：按 Phase 0→5 顺序，每阶段结束先 `cargo check` / `cargo test` 通过再进下一阶段；Phase 0 必须先在真机确认 `shortcuts run` 真能切 DND。
