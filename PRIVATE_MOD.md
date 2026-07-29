# 私人魔改清单（给下次 AI 用）

## 一、魔改目标

| 需求 | 实现方式 |
|------|---------|
| 只扫本地 25565 端口服务端 | UDP 多播 → TCP 主动探测，Minecraft 1.7+ 协议 |
| 固定房间号 | 代码硬编码，房主/房客都用同一个 |
| 管理员密码开房 | 弹窗输入，后端校验，朋友不能开服 |
| 连接状态展示 | 复用已有 `get_players()`，零新增开销 |
| 客人一键加入 | 点击"我想当房客"直接自动加入 |

---

## 二、改动的文件（共 7 个）

### 1. `src/mc/scanning.rs` — 完全重写

删除原多播相关导入，改为 TCP 主动探测。

**新增函数：**
- `write_varint(buf: &mut Vec<u8>, mut value: u32)` — VarInt 编码
- `probe_mc_server(port: u16) -> bool` — TCP 连接 `127.0.0.1:port`，发握手包+状态请求，收 JSON 检查 `"version"`

**改结构体：**
- `create()` 去掉 `filter: fn(&str) -> bool` 参数
- `run()` 改为每 3 秒探测一次 `25565`，结果存到 `output`

**与官方冲突风险：低。** 直接覆盖整个文件。

---

### 2. `src/controller/api.rs` — 3 处改动

**(A) 顶部加常量：**

```rust
const FIXED_ROOM_CODE: &str = "U/PN60-0000-0000-0000";
const ADMIN_PASSWORD: &str = "123456";
```

**(B) `pub fn set_scanning(...)` 函数签名：**

```rust
// 原: pub fn set_scanning(room: Option<String>, player: Option<String>, public_nodes: Vec<String>)
// 改: pub fn set_scanning(_room: Option<String>, player: Option<String>, public_nodes: Vec<String>, password: Option<String>)

// 函数体开头加:
if password.as_deref() != Some(ADMIN_PASSWORD) { return; }

// 房间号逻辑改:
let room = Room::from(FIXED_ROOM_CODE).unwrap_or_else(Room::create);
```

**(C)** `HostOk` API 返回加 `"players"` 字段，`GuestOk` API 返回加 `"difficulty"` 字段

**(D)** 删掉 `use crate::MOTD;`

**与官方冲突风险：中。** 需检查 `set_scanning` 签名是否变化。

---

### 3. `src/controller/states.rs` — 2 个字段

```rust
// HostOk 加:
players: Vec<serde_json::Value>,

// GuestOk 加:
difficulty: ConnectionDifficulty,
```

**与官方冲突风险：低。** 用 `..` 语法不会被破坏。

---

### 4. `src/controller/rooms/scaffolding/room.rs` — 4 处

**(A)** `start_host()` 初始 state 加 `players: vec![]`

**(B)** `start_host()` 监控循环 destructure 加 `players: players_field`

**(C)** `start_host()` 监控循环加 `get_players()` 调用（用 `changed` 标记，不单独调 `increase_shared` 以避免所有权冲突）

**(D)** `start_guest()` 过渡 `GuestStarting→GuestOk` 时把 `difficulty` 带过去

**与官方冲突风险：低。** 追加代码，不破坏原逻辑。

---

### 5. `src/server/states.rs` — 1 处

```rust
#[get("/scanning?<room>&<player>&<public_nodes>&<password>")]
```

**与官方冲突风险：低。** 只加了一个 `Option` 参数。

---

### 6. `src/main.rs` — 1 处

去掉 `#![feature(unsafe_cell_access)]`（新版 nightly 已稳定，不删会 warning）。

---

### 7. `web/_.html` — 3 处

**(A) 房主按钮：** 弹密码 → 扫描开服
**(B) 客人按钮：** 直接自动加入
**(C) 说明文字：** 房主改为"房主仅限服务器管理员操作，多人同时开启房主可能导致联机异常或断开"；房客改为"自动加入房间"

**与官方冲突风险：高。** 官方可能频繁改前端。每次拉新版本后手动替换 `host-tile` 和 `guest-tile` 的点击处理函数及对应说明文字。

---

## 三、不改的文件

| 文件 | 原因 |
|------|------|
| `src/mc/fakeserver.rs` | 无关 |
| `src/easytier/*` | EasyTier 本身已支持 IPv6/P2P |
| `src/scaffolding/*` | 无关 |
| `src/server/statics.rs` | 无关 |

---

## 四、关于房间号固定

当前 `U/PN60-0000-0000-0000` 已校验为合法码。如需更换，要求数值为 **7 的倍数**，用以下 Python 生成：

```python
CHARS = "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ"

def value_to_code(value):
    code = ""
    for i in range(16):
        v = CHARS[value % 34]
        value //= 34
        if i in (4, 8, 12):
            code += "-"
        code += v
    return "U/" + code

seed = int(input("输入数字: "))
seed = seed - seed % 7
print(value_to_code(seed))
```

---

## 五、更新后重做魔改的步骤

```powershell
git pull
# 对着上面 7 个文件逐一手动改
git add .
git commit -m "private mod: reapply after upstream update"
git push
```
