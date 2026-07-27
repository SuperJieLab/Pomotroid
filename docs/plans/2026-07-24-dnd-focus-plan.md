# macOS 勿扰模式（DND）联动 — 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 番茄钟轮次切换时自动切换 macOS 勿扰模式（DND）：Work 轮开始 → 开 DND，休息轮开始 → 关 DND；功能可在设置开关，快捷指令名可配置；失败时发系统通知，不打断计时。

**Architecture:** 新建 `src-tauri/src/focus/mod.rs`，把 `shortcuts run <name>` 的调用封装成纯函数 `apply_dnd(is_work, on, off, runner, is_mac)`，通过 `Runner` trait 注入以便单测 mock。集成点选在 `timer/mod.rs` 的 `listen_events` 的 `TimerEvent::Started` 分支（覆盖首启/恢复/自动续轮，对恢复是幂等重设，无害）。设置新增 3 个字段，前端在 `SystemSection` 中以「仅 macOS 显示」的一组控件承载。

**Tech Stack:** Rust (Tauri 2) + Svelte 5 + Paraglide i18n + macOS `shortcuts` CLI。

> 设计定稿见 `docs/plans/2026-07-24-dnd-focus-design.md`，本计划是其可执行的任务拆解。
> 提交规则（用户要求）：每任务可 `git commit`，但**绝不自动 push**，等用户 review diff 后主动要求再提交。

---

## 关键事实（执行前必读）

- 现有 `src-tauri/src/shortcuts/mod.rs` 是**全局键盘快捷键**（tauri-plugin-global-shortcut），与本次功能无关，不要改它。新模块用 `focus/` 命名避免混淆。
- `settings_set(key, value, ...)` 是通用键写入，`settings_get` 返回完整 `Settings` 结构体序列化。新增字段只需在 Rust 侧补全 struct/defaults/load，前端即可读写，**无需白名单**。
- 平台判断用 `cfg!(target_os = "macos")`。非 macOS 上 `apply_dnd` 直接 `Ok(())` 当 no-op；前端用已有的 `isMac`（`$lib/utils/platform`）隐藏整组控件。
- 失败通知用现成 `crate::notifications::show(app, title, body)`（macOS 走 tauri-plugin-notification）。
- `TimerEvent::Started` 事件负载**不含 round_type**，需从 `sequence.current_round`（类型 `RoundType`）读取。

---

## Task 0：Phase 0 — 验证关键链路（先确认再写代码）

**不写代码，纯手动验证。** 在 macOS 真机终端执行：

```bash
# 前提：用户已在「快捷指令」App 建好两个指令
#   「Pomotroid DND On」  = 动作「设置 专注 > 打开」（勿扰）
#   「Pomotroid DND Off」 = 动作「设置 专注 > 关闭」
shortcuts run "Pomotroid DND On"; echo "exit=$?"
# 预期：系统进入勿扰，exit=0
shortcuts run "Pomotroid DND Off"; echo "exit=$?"
# 预期：系统退出勿扰，exit=0
shortcuts run "This Shortcut Does Not Exist"; echo "exit=$?"
# 预期：exit=非0（如 1），这是失败分支要捕获的信号
```

**Step:** 记录上述三次调用的退出码。
**Expected:** 前两次 `exit=0` 且真机 DND 状态翻转；第三次 `exit≠0`。
**若前两次失败：** 停止，先让用户在快捷指令里配对正确的「专注模式/勿扰」动作，再继续。此步不过，后面全白做。

**Commit:** 无（纯验证）。在记忆/对话里记录「链路已验证」即可。

---

## Task 1：Settings 新增 3 个字段

**Files:**
- Modify: `src-tauri/src/settings/defaults.rs`（`DEFAULTS` 数组）
- Modify: `src-tauri/src/settings/mod.rs`（`Settings` struct + `Default` impl + `load()`）

**Step 1: 写失败测试**

在 `settings/mod.rs` 的 `#[cfg(test)] mod tests` 内新增：

```rust
#[test]
fn focus_dnd_defaults() {
    let conn = setup();
    seed_defaults(&conn).unwrap();
    let s = load(&conn).unwrap();
    assert!(!s.focus_dnd_enabled, "focus_dnd_enabled must default false");
    assert_eq!(s.focus_dnd_on_name, "Pomotroid DND On");
    assert_eq!(s.focus_dnd_off_name, "Pomotroid DND Off");
}

#[test]
fn focus_dnd_save_and_reload() {
    let conn = setup();
    seed_defaults(&conn).unwrap();
    save_setting(&conn, "focus_dnd_enabled", "true").unwrap();
    save_setting(&conn, "focus_dnd_on_name", "My On").unwrap();
    let s = load(&conn).unwrap();
    assert!(s.focus_dnd_enabled);
    assert_eq!(s.focus_dnd_on_name, "My On");
}
```

**Step 2: 运行测试确认失败**
Run: `cd src-tauri && cargo test settings::tests::focus_dnd`
Expected: 编译失败（`focus_dnd_enabled` 字段不存在）。

**Step 3: 实现**
- `defaults.rs` 的 `DEFAULTS` 数组追加三行（紧跟 `local_shortcut_fullscreen` 之后）：
  ```rust
  ("focus_dnd_enabled", "false"),
  ("focus_dnd_on_name", "Pomotroid DND On"),
  ("focus_dnd_off_name", "Pomotroid DND Off"),
  ```
- `settings/mod.rs` `Settings` struct 在 `local_shortcut_fullscreen: String,` 之后加：
  ```rust
  pub focus_dnd_enabled: bool,
  pub focus_dnd_on_name: String,
  pub focus_dnd_off_name: String,
  ```
- `Default` 实现对应加（放在 `local_shortcut_fullscreen: "F11".to_string(),` 之后）：
  ```rust
  focus_dnd_enabled: false,
  focus_dnd_on_name: "Pomotroid DND On".to_string(),
  focus_dnd_off_name: "Pomotroid DND Off".to_string(),
  ```
- `load()` 在 `local_shortcut_fullscreen: ...unwrap_or(...),` 之后加：
  ```rust
  focus_dnd_enabled: parse_bool(&map, "focus_dnd_enabled", d.focus_dnd_enabled),
  focus_dnd_on_name: map.get("focus_dnd_on_name").cloned().unwrap_or(d.focus_dnd_on_name),
  focus_dnd_off_name: map.get("focus_dnd_off_name").cloned().unwrap_or(d.focus_dnd_off_name),
  ```

**Step 4: 运行测试确认通过**
Run: `cd src-tauri && cargo test settings::tests::focus_dnd`
Expected: 两个 test PASS。

**Step 5: Commit**
```bash
git add src-tauri/src/settings/defaults.rs src-tauri/src/settings/mod.rs
git commit -m "feat(settings): add focus DND toggle + shortcut name fields"
```
（不 push）

---

## Task 2：focus 模块 + Runner trait（核心，可单测）

**Files:**
- Create: `src-tauri/src/focus/mod.rs`
- Modify: `src-tauri/src/lib.rs`（加 `pub mod focus;`）

**Step 1: 写失败测试**

在 `focus/mod.rs` 末尾 `#[cfg(test)] mod tests`：

```rust
/// Mock runner：记录调用，返回预设结果。
struct MockRunner {
    calls: std::cell::RefCell<Vec<String>>,
    fail: bool,
}
impl super::Runner for MockRunner {
    fn run(&self, name: &str) -> std::io::Result<()> {
        self.calls.borrow_mut().push(name.to_string());
        if self.fail { Err(std::io::Error::new(std::io::ErrorKind::NotFound, "boom")) }
        else { Ok(()) }
    }
}

#[test]
fn disabled_is_noop() {
    let r = MockRunner { calls: Default::default(), fail: false };
    let res = super::apply_dnd(true, "On", "Off", &r, true);
    assert!(res.is_ok());
    assert!(r.calls.borrow().is_empty(), "disabled must not call CLI");
}

#[test]
fn non_mac_is_noop() {
    let r = MockRunner { calls: Default::default(), fail: false };
    let res = super::apply_dnd(true, "On", "Off", &r, false); // is_mac=false
    assert!(res.is_ok());
    assert!(r.calls.borrow().is_empty(), "non-mac must not call CLI");
}

#[test]
fn work_enables_on_shortcut() {
    let r = MockRunner { calls: Default::default(), fail: false };
    super::apply_dnd(true, "On", "Off", &r, true).unwrap();
    assert_eq!(r.calls.borrow().as_slice(), &["On".to_string()]);
}

#[test]
fn break_disables_via_off_shortcut() {
    let r = MockRunner { calls: Default::default(), fail: false };
    super::apply_dnd(false, "On", "Off", &r, true).unwrap();
    assert_eq!(r.calls.borrow().as_slice(), &["Off".to_string()]);
}

#[test]
fn cli_failure_propagates_error() {
    let r = MockRunner { calls: Default::default(), fail: true };
    let res = super::apply_dnd(true, "On", "Off", &r, true);
    assert!(res.is_err());
}
```

**Step 2: 运行确认失败**
Run: `cd src-tauri && cargo test focus::`
Expected: 编译失败（`focus` 模块不存在）。

**Step 3: 实现 `focus/mod.rs`**

```rust
//! macOS 勿扰模式（DND）联动。
//! 通过 `shortcuts run <name>` 调用用户在「快捷指令」App 里建好的指令。
//! 非 macOS 或功能关闭时直接 no-op。

/// 抽象 CLI 调用，便于单测注入 mock。
pub trait Runner {
    fn run(&self, name: &str) -> std::io::Result<()>;
}

/// 生产用 runner：真正执行 `shortcuts run <name>`。
pub struct OsRunner;
impl Runner for OsRunner {
    fn run(&self, name: &str) -> std::io::Result<()> {
        std::process::Command::new("shortcuts")
            .arg("run")
            .arg(name)
            .status()
            .and_then(|s| {
                if s.success() { Ok(()) }
                else { Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("shortcut '{name}' exited non-zero"),
                )) }
            })
    }
}

/// 根据当前是否 Work 轮，切换 DND。
/// - `is_mac=false` 或 `enabled=false` → 直接 Ok，不碰 CLI。
/// - `is_work=true`  → 运行 `on_name`
/// - `is_work=false` → 运行 `off_name`
pub fn apply_dnd(
    is_work: bool,
    on_name: &str,
    off_name: &str,
    runner: &dyn Runner,
    is_mac: bool,
) -> Result<(), std::io::Error> {
    if !is_mac {
        return Ok(());
    }
    let name = if is_work { on_name } else { off_name };
    runner.run(name)
}
```

**Step 4: 运行确认通过**
Run: `cd src-tauri && cargo test focus::`
Expected: 全部 PASS。

**Step 5: Commit**
```bash
git add src-tauri/src/focus/mod.rs src-tauri/src/lib.rs
git commit -m "feat(focus): add apply_dnd with injectable Runner for testing"
```
（不 push）

---

## Task 3：接入计时事件循环

**Files:**
- Modify: `src-tauri/src/timer/mod.rs`（`listen_events` 的 `TimerEvent::Started` 分支）

**Step 1: 写失败测试**

把「决定 + 调用」抽成可单测的纯函数，放在 `focus/mod.rs`（便于测），再放入 timer。在 `focus/mod.rs` 增加：

```rust
/// 从 round_type 判定是否应开启 DND（仅 Work 轮开启）。
pub fn should_enable(round: crate::timer::sequence::RoundType) -> bool {
    matches!(round, crate::timer::sequence::RoundType::Work)
}
```

`focus/mod.rs` 测试里加：
```rust
#[test]
fn only_work_enables() {
    use crate::timer::sequence::RoundType;
    assert!(super::should_enable(RoundType::Work));
    assert!(!super::should_enable(RoundType::ShortBreak));
    assert!(!super::should_enable(RoundType::LongBreak));
}
```

**Step 2: 运行确认失败**
Run: `cd src-tauri && cargo test focus::should_enable`
Expected: 编译失败（函数未定义）。

**Step 3: 实现**
- 在 `focus/mod.rs` 加入上面的 `should_enable`。
- `timer/mod.rs` 顶部 `use` 增加：`use crate::focus::{self, OsRunner};` 和 `use crate::notifications;`（已有 `use crate::settings::Settings;` 等）。
- 在 `TimerEvent::Started { total_secs } => {` 分支内，在 `shared.lock().unwrap().is_running = true;` 之后插入：
  ```rust
  // --- DND 联动：按当前轮次类型切换 macOS 勿扰模式 ---
  {
      let s = settings.lock().unwrap();
      if s.focus_dnd_enabled {
          let on = s.focus_dnd_on_name.clone();
          let off = s.focus_dnd_off_name.clone();
          let is_work = focus::should_enable(sequence.lock().unwrap().current_round);
          drop(s);
          if let Err(e) = focus::apply_dnd(
              is_work, &on, &off, &OsRunner,
              cfg!(target_os = "macos"),
          ) {
              log::warn!("[focus] DND toggle failed: {e}");
              notifications::show(
                  &app,
                  "专注模式切换失败",
                  &format!("请检查快捷指令「{}」是否存在", if is_work { &on } else { &off }),
              );
          }
      }
  }
  ```
  > 注意：`Started` 在「恢复暂停」时也会触发，此时 `current_round` 不变 → 对 DND 是幂等重设（已开再开无害）。自动续轮由 `Complete` 发送 `TimerCommand::Start` 引发 `Started`，因此单一 hook 已覆盖所有轮次切换，无需在 `Complete` 再加。

**Step 4: 运行确认通过 + 类型检查**
Run: `cd src-tauri && cargo test focus:: && cargo check`
Expected: test PASS；`cargo check` 无错（确认 timer 借用/作用域正确，特别是 `drop(s)` 后再用 `sequence`）。

**Step 5: Commit**
```bash
git add src-tauri/src/focus/mod.rs src-tauri/src/timer/mod.rs
git commit -m "feat(timer): toggle macOS DND on round start via focus module"
```
（不 push）

---

## Task 4：前端类型 + store 默认值

**Files:**
- Modify: `src/lib/types.ts`（`Settings` 接口）
- Modify: `src/lib/stores/settings.ts`（`defaults`）

**Step 1: 改类型（无独立测试，靠 `npm run check` 兜底）**
`types.ts` 的 `Settings` 接口在 `local_shortcut_fullscreen: string;` 后加：
```ts
  focus_dnd_enabled: boolean;
  focus_dnd_on_name: string;
  focus_dnd_off_name: string;
```
`settings.ts` 的 `defaults` 对象在 `local_shortcut_fullscreen: 'F11',` 后加：
```ts
  focus_dnd_enabled: false,
  focus_dnd_on_name: 'Pomotroid DND On',
  focus_dnd_off_name: 'Pomotroid DND Off',
```

**Step 2: 类型检查**
Run: `npm run check`
Expected: 无类型错误（Rust 侧已序列化同名字段，前后端 struct 现已对齐）。

**Step 3: Commit**
```bash
git add src/lib/types.ts src/lib/stores/settings.ts
git commit -m "feat(frontend): add focus DND settings to types + store defaults"
```
（不 push）

---

## Task 5：前端 UI（SystemSection，仅 macOS）

**Files:**
- Modify: `src/lib/components/settings/sections/SystemSection.svelte`
- 复用: `SettingsToggle.svelte`、`$lib/ipc` 的 `setSetting`

**Step 1: 加控件（套用已有 websocket 范式）**
在 `<script>` 里复用已有 `toggle()` 函数（它已支持任意 dbKey）。新增两个本地绑定 + blur 保存：
```ts
let dndOnName = $state($settings.focus_dnd_on_name);
let dndOffName = $state($settings.focus_dnd_off_name);
$effect(() => { dndOnName = $settings.focus_dnd_on_name; dndOffName = $settings.focus_dnd_off_name; });

async function saveName(key: string, val: string) {
  const updated = await setSetting(key, val);
  settings.set(updated);
}
```
在模板 `system_group_integrations` 分组内、`websocket` toggle **之前**插入（注意整组用 `{#if isMac}` 包裹，非 macOS 完全不显示）：
```svelte
{#if isMac}
  <div class="group-heading">专注模式联动</div>
  <SettingsToggle
    label={"自动切换勿扰模式"}
    description={"工作开始时开启勿扰，休息时关闭"}
    checked={$settings.focus_dnd_enabled}
    onclick={() => toggle('focus_dnd_enabled', $settings.focus_dnd_enabled)}
  />
  {#if $settings.focus_dnd_enabled}
    <div class="row">
      <span class="label">开启指令名</span>
      <input class="port-input" type="text" bind:value={dndOnName} onblur={() => saveName('focus_dnd_on_name', dndOnName)} />
    </div>
    <div class="row">
      <span class="label">关闭指令名</span>
      <input class="port-input" type="text" bind:value={dndOffName} onblur={() => saveName('focus_dnd_off_name', dndOffName)} />
    </div>
    <p class="note">
      需要在「快捷指令」App 中创建两个指令：动作均为「设置 → 专注 → 打开/关闭」。
      默认名 <code>Pomotroid DND On</code> / <code>Pomotroid DND Off</code>。
    </p>
  {/if}
{/if}
```

**Step 2: 类型/构建检查**
Run: `npm run check`
Expected: 无错误。

**Step 3: Commit**
```bash
git add src/lib/components/settings/sections/SystemSection.svelte
git commit -m "feat(ui): focus DND toggle + shortcut name inputs (macOS only)"
```
（不 push）

---

## Task 6：i18n 文案

**Files:**
- Modify: Paraglide 文案源（英文 + 中文）。定位方式：仓库用 inlang，`src/lib/paraglide/messages.js` 是从 `project.inlang` 编译生成；新增 key 需加到 inlang message catalog（英文源 + zh 翻译），再 `npm run paraglide:compile` 重新生成。
- 参考：`SystemSection` 已用 `m.system_toggle_websocket()` 等，新 key 命名建议：`focus_group`、`focus_toggle`、`focus_toggle_desc`、`focus_on_name`、`focus_off_name`、`focus_note`。

**Step 1: 加 key + 翻译**
- 英文：`focus_toggle` = "Auto-toggle Do Not Disturb" 等
- 中文：`focus_toggle` = "自动切换勿扰模式" 等

**Step 2: 重新编译 + 替换硬编码文案**
Run: `npm run paraglide:compile`
把 Task 5 里写死的 `label`/`description`/`note` 文本替换为 `m.focus_toggle()` 等调用。

**Step 3: 检查**
Run: `npm run check`
Expected: 无错误，文案走 i18n。

**Step 4: Commit**
```bash
git add -A
git commit -m "feat(i18n): add focus DND strings (en + zh)"
```
（不 push）

---

## Task 7：整体验收

**Step 1: 后端单测全绿**
Run: `cd src-tauri && cargo test`
Expected: 全部 PASS（含 settings / focus 新用例）。

**Step 2: 前端检查**
Run: `npm run check`
Expected: 无类型/语法错误。

**Step 3: 真机手动验收（macOS）**
1. 创建两个快捷指令（Set Focus → DND On/Off），名字与设置一致。
2. 开「自动切换勿扰模式」开关，开始一个 Work 轮 → 系统进入勿扰。
3. 切到休息轮 → 勿扰关闭。
4. 把一个指令名改错 → 收到「专注模式切换失败」通知，计时不受影响。
5. 关总开关 → 轮次切换不再触发任何 DND 变化。

**Step 4: 提交汇总（如前面任务已分别 commit，此步可跳过）**
若选择不逐任务提交，则在此统一提交；否则确认各任务 commit 已在本地。
> 任意情况都**不要 push**，等用户 review diff 后主动说「提交」再推。

---

## 风险与备注

- **捷径依赖**：功能强依赖用户自建快捷指令，这是 Apple 限制所致，非技术妥协。已在 UI note 与失败通知里引导。
- **幂等假设**：`Started` 在恢复暂停时重设 DND；依赖「Set Focus → 打开」对已开状态是 no-op。若某 macOS 版本非幂等，需在 Task 3 增加「状态去重」（先读 `INFocusStatusCenter` 再决定），但当前不实现（YAGNI）。
- **i18n 文件位置**：若 `project.inlang` 实际结构与此计划假定不同，以仓库内 inlang 配置为准，调用 `npm run paraglide:compile` 后确认 `src/lib/paraglide/messages.js` 出现新 key。
