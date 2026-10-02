# NES Emulator

这是一个用 Rust 编写的简易 NES（任天堂娱乐系统）模拟器，目标是提供一个可运行的教育型模拟器核心，支持 ROM 加载、画面渲染、音频输出和按键输入。

## 功能概览

- 兼容基本的 NES CPU 执行循环
- PPU 帧缓冲渲染
- APU 音频输出
- 读取 .nes ROM 文件
- 键盘 / XInput 游戏手柄输入
- 窗口化显示与缩放
- 支持导出一段帧为 PNG 图片
- 自带单元测试，用于验证核心逻辑

## 项目结构

```text
.
├── Cargo.toml
├── LICENSE
├── src/
│   ├── apu.rs
│   ├── audio.rs
│   ├── bus.rs
│   ├── cartridge.rs
│   ├── controller.rs
│   ├── cpu.rs
│   ├── emulator.rs
│   ├── input.rs
│   ├── main.rs
│   └── ppu.rs
└── target/
```

关键模块说明：

- `src/main.rs`：程序入口，负责命令行参数处理、窗口创建和主循环。
- `src/emulator.rs`：模拟器核心对象，封装 CPU 与总线执行逻辑。
- `src/cpu.rs`：CPU 指令执行。
- `src/ppu.rs`：图像处理与帧缓冲。
- `src/apu.rs`、`src/audio.rs`：音频生成与输出。
- `src/cartridge.rs`：ROM 解析。
- `src/bus.rs`：存储总线与设备挂载。
- `src/input.rs`、`src/controller.rs`：输入映射。

## 环境要求

- Rust 稳定版工具链
- 支持 `edition = "2024"` 的 Cargo 环境
- 需要可用的显示窗口能力（窗口化输出）
- 可选：游戏手柄 / XInput 支持

## 构建

在项目根目录执行：

```bash
cargo build --release
```

## 运行

启动模拟器并加载 ROM：

```bash
cargo run --release  path/to/rom.nes
```

可选提供窗口缩放参数：

```bash
cargo run --release path/to/rom.nes 2
```

当前实现中，缩放参数可取 `1`、`2`、`3`、`4`，分别对应大约 `1x`、`2x`、`4x`、`8x` 的窗口缩放。

## 生成 PNG 画面输出

该项目支持直接导出一段帧作为图片：

```bash
cargo run --release -- --dump path/to/rom.nes 120 output.png
```

示例含义：

- `--dump`：进入导出模式
- `path/to/rom.nes`：ROM 文件路径
- `120`：模拟的帧数
- `output.png`：输出 PNG 文件路径

## 控制说明

### 键盘控制

- `方向键`：移动
- `Z`：A
- `X`：B
- `A` / `S`：Turbo A / Turbo B
- `Enter`：Start
- `Shift`：Select
- `Esc`：退出

### 游戏手柄控制

- `D-Pad`：移动
- `A / B`：A / B
- `X / Y`：Turbo A / Turbo B
- `Start / Select`：开始 / 选择

## 测试

运行测试：

```bash
cargo test
```

项目中已包含一些基础单元测试，用于验证 CPU 执行与帧循环行为。

## 许可协议

本项目使用 MIT License。详细内容见 [LICENSE](LICENSE)。

## 说明

这是一个学习与实验性质的 NES 模拟器，适合用于理解：

- CPU 指令执行
- PPU 渲染流程
- APU 音频生成
- 存储映射与总线架构
- 手柄和键盘输入处理

如果你想继续扩展这个项目，可以从以下方向入手：

1. 增加更多 Mapper 支持
2. 完善更完整的 PPU 逻辑
3. 增加更稳定的音频同步
4. 添加调试器和反汇编能力
5. 支持更多 ROM 类型与兼容性优化
