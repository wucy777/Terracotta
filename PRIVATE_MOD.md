# 私人魔改完整改动清单

> 基于 `burningtnt/Terracotta` master 分支（commit 7436432）
> 所有改动的 commit 历史：df30951 → 7a2fa9f → 1ba08d2 → 6411159 → d3faa2f → 5ad5fc6 → 2d3985b

---

## 文件 1：`src/mc/scanning.rs` — 完全重写

### 原版行为
被动监听 UDP 多播 `224.0.2.60:4445`（IPv4）和 `FF75:230::60:4445`（IPv6），接收 Minecraft 客户端 LAN 世界广播包 `[MOTD]...[/MOTD][AD]port[/AD]`。只能扫到客户端开的局域网世界，扫不到独立服务端。

### 魔改行为
每 3 秒主动 TCP 连接 `127.0.0.1:25565`，用 Minecraft 1.7+ Server List Ping 协议验证。

### 具体修改

新增两个函数：

| 函数 | 位置 | 作用 |
|------|------|------|
| `fn write_varint(buf, value: u32)` | 第 13-22 行 | VarInt 编码（Minecraft 协议用） |
| `fn probe_mc_server(port: u16) -> bool` | 第 24-95 行 | TCP 连 127.0.0.1:port，发握手包+状态请求，收 JSON 检查有无 `"version"` 字段 |

修改 `impl MinecraftScanner`：

| 项目 | 原版 | 魔改 |
|------|------|------|
| `create` 签名 | `create(filter: fn(&str) -> bool)` | `create()` |
| 扫描方式 | UDP 多播监听所有网卡 | TCP 连接 `127.0.0.1:25565` |
| 线程函数 | `run(signal, output, filter) -> Result<()>` | `run(signal, output)` 无返回值 |
| 扫描间隔 | 每 500ms recv 超时轮询 | 每 3 秒一次完整探测 |
| 端口管理 | 5 秒超时自动移除旧端口 | 固定只检测 25565 |

导入变化：删除 `socket2`、`MaybeUninit`、`IpAddr`、`Ipv6Addr`、`Cow`、`FromStr` 等，新增 `Read`、`Write`、`TcpStream`。

---

## 文件 2：`src/controller/api.rs` — 7 处改动

### ① 删除无用导入（第 6 行）
```rust
// 删除:
use crate::MOTD;
```

### ② 新增常量（第 14-15 行）
```rust
const FIXED_ROOM_CODE: &str = "U/PN60-0000-0000-0000";
const ADMIN_PASSWORD: &str = "123456";
```
- `FIXED_ROOM_CODE`：硬编码房间号，房主房客都用同一个
- `ADMIN_PASSWORD`：开房管理员密码

### ③ `HostOk` 状态返回新增 `players` 字段（第 32 行、第 47 行）
```rust
// 原匹配:
AppState::HostOk { room, profiles, .. } => {
// 改匹配:
AppState::HostOk { room, profiles, players, .. } => {

// JSON 响应新增:
"players": players
```

### ④ `GuestOk` 状态返回新增 `difficulty` 字段（第 62 行、第 69-75 行）
```rust
// 原匹配:
AppState::GuestOk { server, profiles, .. } => {
// 改匹配:
AppState::GuestOk { server, profiles, difficulty, .. } => {

// JSON 响应新增:
"difficulty": "UNKNOWN|EASIEST|SIMPLE|MEDIUM|TOUGH"
```

### ⑤ `set_scanning` 函数签名变化（第 102 行）
```rust
// 原:
pub fn set_scanning(room: Option<String>, player: Option<String>, public_nodes: Vec<String>)

// 改:
pub fn set_scanning(_room: Option<String>, player: Option<String>, public_nodes: Vec<String>, password: Option<String>)
```

### ⑥ 新增密码校验（第 103-106 行）
```rust
if password.as_deref() != Some(ADMIN_PASSWORD) {
    logging!("Core", "Admin password mismatch, denied.");
    return;
}
```
密码不匹配直接 return，不执行任何操作。

### ⑦ 房间号逻辑改为硬编码（第 121 行）
```rust
// 原:
let room = room.and_then(|room| Room::from(&room)).unwrap_or_else(Room::create);

// 改:
let room = Room::from(FIXED_ROOM_CODE).unwrap_or_else(Room::create);
```

---

## 文件 3：`src/controller/states.rs` — 2 个枚举字段 + Debug 输出

### ① `HostOk` 新增 `players`（第 28 行）
```rust
// HostOk 内新增:
players: Vec<serde_json::Value>,
```

### ② `GuestOk` 新增 `difficulty`（第 43 行）
```rust
// GuestOk 内新增:
difficulty: ConnectionDifficulty,
// 同时删除了 GuestOk 内 profiles 前面的空行
```

### ③ Debug 输出更新（第 69-70 行）
```rust
// 原:
"AppState::HostOk {{ code: {:?}, port: {}, easytier: .., profiles: {:?} }}"

// 改:
"AppState::HostOk {{ code: {:?}, port: {}, easytier: .., profiles: {:?}, players: .. }}"
```
```rust
// 原:
"AppState::GuestOk {{ code: {:?}, server_port: {}, easytier: .., profiles: {:?} }}"

// 改:
"AppState::GuestOk {{ code: {:?}, server_port: {}, easytier: .., profiles: {:?}, difficulty: .. }}"
```

---

## 文件 4：`src/controller/rooms/scaffolding/room.rs` — 4 处改动

### ① `start_host()` 初始化 `HostOk` 加 `players: vec![]`（第 168 行）
```rust
// state.set(AppState::HostOk { ... }) 中新增:
players: vec![],
```

### ② `start_host()` 监控循环 destructure 加 `players`（第 193 行）
```rust
// 原:
let AppState::HostOk { easytier, profiles, .. } = state.as_mut_ref() else {

// 改:
let AppState::HostOk { easytier, profiles, players: players_field, .. } = state.as_mut_ref() else {
```

### ③ `start_host()` 监控循环加 `get_players()` 调用（第 202-211 行）
```rust
if let Some(players) = easytier.get_players() {
    let players: Vec<serde_json::Value> = players.into_iter().map(|p| serde_json::json!({
        "hostname": p.hostname,
        "address": p.address.map(|a| a.to_string()),
        "nat": format!("{:?}", p.nat),
        "is_local": p.is_local,
    })).collect();
    *players_field = players;
    changed = true;
}
```
每 5 秒监控循环里调用一次 `get_players()`（复用已有的定时器，不新增线程），把每位玩家的虚拟 IP、NAT 类型、是否自己这台的标志存入 state。用 `changed` 标记统一触发 `increase_shared()`（避免所有权冲突）。

### ④ `start_guest()` 过渡时把 `difficulty` 带到 `GuestOk`（第 443 行、第 452 行）
```rust
// 原:
let AppState::GuestStarting { room, easytier, .. } = state else {

// 改:
let AppState::GuestStarting { room, easytier, difficulty } = state else {

// GuestOk 新增:
difficulty,
```

---

## 文件 5：`src/server/states.rs` — 1 处改动

### 路由参数新增 `password`（第 19-21 行）
```rust
// 原:
#[get("/scanning?<room>&<player>&<public_nodes>")]
fn set_state_scanning(room: Option<String>, player: Option<String>, public_nodes: Vec<String>) -> Status {

// 改:
#[get("/scanning?<room>&<player>&<public_nodes>&<password>")]
fn set_state_scanning(room: Option<String>, player: Option<String>, public_nodes: Vec<String>, password: Option<String>) -> Status {
```
只是多加了一个可选的 URL 查询参数，对现有调用无影响。

---

## 文件 6：`src/main.rs` — 1 处改动

### 删除已稳定的 feature（第 7 行）
```rust
// 删除整行:
#![feature(unsafe_cell_access)]
```
该 feature 在新版 nightly 中已默认启用，不删会编译 warning。

---

## 文件 7：`web/_.html` — 全部前端改动

### ① CSS 新增 `.top-header` 固定顶栏（第 44-52 行）
```css
.top-header {
    position: fixed; top: 0; left: 0; right: 0;
    padding: 20px 20px 10px; text-align: center;
    z-index: 30;
    background: linear-gradient(135deg, rgba(26,26,46,0.95), rgba(22,33,62,0.95));
}
```

### ② 修改 `.container` 布局（第 54-64 行）
```css
/* 原 */
.container { position: fixed; top: 0; bottom: 80px; }

/* 改 */
.container { position: fixed; top: 150px; bottom: 170px; z-index: 20;
             overflow-y: auto; overflow-x: hidden; scrollbar-gutter: stable; }
```

### ③ 删除原 `header` 样式（第 66-68 行删除）
```css
/* 删除 */
header { margin-bottom: 40px; }
```

### ④ 修改 `footer` 样式（第 506-513 行）
```css
/* 原 */
footer { margin-top: 40px; position: fixed; bottom: 20px; }

/* 改 */
footer { position: fixed; bottom: 0; padding: 14px 20px; }
```

### ⑤ 新增 `.close-button` 样式（第 383-402 行）
```css
.close-button {
    background: linear-gradient(135deg, #c0392b, #e74c3c);
    color: white; border: none; padding: 10px 28px;
    font-size: 0.95rem; border-radius: 50px; cursor: pointer;
    margin: 6px 0;
}
.close-button:hover { ... }
```

### ⑥ 新增 `.players-list` 表格样式（第 404-440 行）
```css
.players-list { width: 100%; margin: 16px 0; text-align: left; }
.players-list table { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
.players-list th, .players-list td { padding: 8px 12px; border-bottom: 1px solid rgba(255,255,255,0.1); }
.players-list th { opacity: 0.6; font-weight: 400; font-size: 0.8rem; }
.players-list td { font-weight: 300; }
```

### ⑦ 滚动条样式改为 `.container` 作用域（第 489-503 行）
```css
/* 原（全局） */
*::-webkit-scrollbar { width: 10px; }
*::-webkit-scrollbar-thumb { background-color: #b5958c; }

/* 改（仅容器） */
.container::-webkit-scrollbar { width: 4px; }
.container::-webkit-scrollbar-track { background: transparent; }
.container::-webkit-scrollbar-thumb {
    background-color: rgba(255, 255, 255, 0.35);
    border-radius: 20px;
}
.container::-webkit-scrollbar-thumb:hover {
    background-color: rgba(255, 255, 255, 0.6);
}
```

### ⑧ HTML 结构调整：顶栏移出容器（第 618-627 行）
```html
<!-- 原 -->
<div class="container">
    <header>
        <h1>Terracotta | 陶瓦联机</h1>
        <div class="subtitle">基于 EasyTier 的 Minecraft 联机助手</div>
    </header>
    <div class="view-container">...

<!-- 改 -->
<div class="top-header">
    <h1>Terracotta | 陶瓦联机</h1>
    <div class="subtitle">基于 EasyTier 的 Minecraft 联机助手</div>
    <div style="...">关闭浏览器或关闭当前标签页面，程序仍然在后台运行...</div>
</div>
<div class="container">
    <div class="view-container">...
```

### ⑨ 房主/房客描述文字修改（第 629-636 行）
```html
<!-- 房主原 -->
<p class="tile-description">创建房间并生成邀请码，与好友一起畅玩</p>
<!-- 房主改 -->
<p class="tile-description">房主仅限服务器管理员操作，多人同时开启房主可能导致联机异常或断开</p>

<!-- 房客原 -->
<p class="tile-description">输入房主提供的邀请码加入游戏世界</p>
<!-- 房客改 -->
<p class="tile-description">自动加入房间，与好友一起畅玩</p>
```

### ⑩ 扫描提示文字修改（第 657 行）
```html
<!-- 原 -->
<div class="loading-text">请进入单人存档，按下 ESC 键，选择对局域网开放，点击创建局域网世界。</div>
<!-- 改 -->
<div class="loading-text">正在扫描本地 Minecraft 服务端（127.0.0.1:25565），请确保服务端正在运行。</div>
```

### ⑪ `host-result-view` 新增玩家列表表格（第 691-704 行）
```html
<!-- 在 invite-code 下方新增 -->
<p class="result-description">正在等待玩家加入</p>
<div class="players-list" id="players-list-container" style="display:none;">
    <table>
        <thead><tr><th>虚拟 IP</th><th>NAT 类型</th></tr></thead>
        <tbody id="players-list-body"></tbody>
    </table>
</div>
```

### ⑫ Footer 新增完全关闭按钮（第 779-783 行）
```html
<!-- 在 QQ 群链接和版本号之间新增 -->
<p><button class="close-button" id="close-app-button">完全关闭 Terracotta 及后台进程</button></p>
```

### ⑬ JavaScript：房主按钮弹密码（第 934-943 行）
```javascript
// 原:
document.getElementById("host-tile").addEventListener('click', () => {
    showView('host-scanning-view');
    fetch("/state/scanning");
});

// 改:
document.getElementById("host-tile").addEventListener('click', () => {
    let pwd = prompt("请输入管理员密码开服");
    if (pwd) {
        showView('host-scanning-view');
        fetch("/state/scanning?password=" + encodeURIComponent(pwd));
    }
});
```

### ⑭ JavaScript：客人按钮一键加入（第 945-955 行）
```javascript
// 原:
document.getElementById('guest-tile').addEventListener('click', () => {
    showView('guest-input-view');
    document.getElementById("invite-code-input").value = "";
    // ...
});

// 改:
document.getElementById('guest-tile').addEventListener('click', () => {
    fetch("/state/guesting?room=U/PN60-0000-0000-0000").then(r => {
        if (r.status == 200) {
            showView('guest-loading-view');
        } else {
            showView("room-error-view");
            document.getElementById("room-error-icon").innerText = "❌";
            document.getElementById("room-error-title").innerText = "加入房间失败";
            document.getElementById("room-error-desc").innerText = "房间已关闭或网络不稳定";
        }
    });
});
```

### ⑮ JavaScript：状态轮询中更新玩家列表（第 865-879 行）
```javascript
// 在 host-ok 状态处理中新增:
if (r.players && r.players.length > 0) {
    let html = "";
    for (let p of r.players) {
        let addr = p.address || "中继节点";
        let name = p.is_local ? "（房主）" : "";
        let nat = p.nat && p.nat != "Unknown" ? p.nat : "-";
        html += "<tr><td>" + addr + name + "</td><td>" + nat + "</td></tr>";
    }
    document.getElementById("players-list-body").innerHTML = html;
    document.getElementById("players-list-container").style.display = "";
    let count = r.players.filter(p => !p.is_local).length;
    document.querySelector("#host-result-view .result-description").innerText =
        count > 0 ? count + " 位玩家已连接" : "正在等待玩家加入";
}
```

### ⑯ JavaScript：完全关闭按钮事件（第 966-976 行）
```javascript
document.getElementById("close-app-button").addEventListener("click", () => {
    if (confirm("确定要完全关闭 Terracotta 吗？")) {
        fetch("/panic?peaceful=true").finally(() => {
            setTimeout(() => {
                try { window.close(); } catch(e) {}
                document.body.innerHTML = '<div style="...">Terracotta 已关闭，请关闭此浏览器标签页。</div>';
            }, 500);
        });
    }
});
```

---

## 新增文件 1：`.github/workflows/build.yaml`

与上游 `burningtnt/Terracotta` 完全一致，无任何修改。包含全平台编译流程（Windows/Linux/Android/macOS/FreeBSD），由 `wty2019wty` 维护。

---

## 新增文件 2：`PRIVATE_MOD.md`

本文件，魔改说明文档。

---

## 功能总结

| 功能 | 实现方式 | 涉及文件 |
|------|---------|---------|
| 扫描本地 25565 服务端 | TCP 主动探测 + MC 1.7+ 协议 | `src/mc/scanning.rs` |
| 固定房间号 | 代码硬编码 `FIXED_ROOM_CODE` | `src/controller/api.rs` |
| 管理员密码开房 | 弹窗输入 + 后端校验 | `api.rs` + `web/_.html` |
| 客人一键加入 | 自动调用 guesting API | `web/_.html` |
| 房主查看成员连接信息 | 复用 `get_players()` | `room.rs` + `states.rs` + `api.rs` |
| 完全关闭按钮 | 调 `/panic?peaceful=true` | `web/_.html` |
| 布局固定 | 顶栏/底栏固定，中间滚动 | `web/_.html` CSS |
| 滚动条优化 | 4px 滚动条，仅限容器区域 | `web/_.html` CSS |

## 未改动的文件

`src/easytier/*`、`src/scaffolding/*`、`src/mc/fakeserver.rs`、`src/mc/mod.rs`、`build.rs`、`Cargo.toml`、`src/server/statics.rs` 等全部与上游一致。
