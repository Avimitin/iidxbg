# Implementation update (2026-10-23)

The earlier research below is retained as historical analysis. A first runtime
implementation now exists for **ordinary per-song result backgrounds** in
`src/video.rs` and `src/video/mf.rs`; other animation surfaces remain research.
Native evidence and exact input hash are in
[the results follow-up](iidx34-010-0917-analysis.md#results-follow-up-2026-10-23).

Set this **top-level** key in `iidxbg.toml` (before any `[section]`):

```toml
results_video = 'iidxbg/videos/background.mp4'
```

The configuration GUI exposes the same setting under **全局选项 → 结算视频**,
with a file chooser and clearable path field. Paths must be accessible from
the game machine. The image preview remains the fallback-image preview.

An empty string selects the existing image provider. The path is relative to
the TOML directory, or may be absolute. Changes are considered at scene request
boundaries, so the current result visit keeps its selected asset. The first
frame must finish decoding before the video can be published for another visit.
Use a new path when replacing a video; no active-file watcher is implemented.

The implementation uses Windows Media Foundation's Source Reader with advanced
video processing and only the first video stream selected. Playback is silent
and loops. H.264 MP4 is the tested format; other containers/codecs depend on
the machine's installed MF decoders. Output stretches to `[image]` width/height,
limited to 1920x1080; rotation metadata, HDR, aspect-preserving video crops,
audio, remote video URLs, and gameplay BGA replacement are not implemented.
Use an SDR video with the intended display aspect ratio.

Each decoder owns its COM/MF objects on a dedicated below-normal-priority MTA
thread. Only owned BGRA pixels cross into a three-frame mailbox. RGB32's unused
alpha byte is made opaque. `IMF2DBuffer2::Lock2DSize` supplies actual scanline
zero, pitch and allocation bounds; the media type's default stride is used
only for contiguous buffers without that interface. The processor can return
top-down storage despite a bottom-up default: a live test caught this mismatch.
Padded/signed strides and allocation bounds are checked, and compatible
initial media-type refinements are accepted. Incompatible dimensions/subtype
changes stop decoding. Initial failures fall back to the image provider;
mid-playback failures retain the last ready frame, with a new decoder attempted
at a subsequent scene request. The initialization wait is on the provider worker,
with a 20-second publication timeout, never on a game callback.

The graphics owner uses a nonblocking mailbox read, selects the newest due PTS,
and refreshes the existing dynamic D3D9 texture. It never opens a video or invokes
a decoder. Slow decoding holds the last frame. A new visit gets a new epoch,
seeks to the beginning on the worker, and rejects queued frames from old visits.
After EOF, subsequent PTS values include the previous loop's ending timestamp.
Graphics reset retains the selected CPU frame/visit clock and recreates only
native graphics resources. This is implemented behavior; actual reset timing
and frame-time tails still need live stress testing.

The local fixture remains outside source control:
`target/sample-1080.mp4`, 1,046,987 bytes, SHA-256
`38d763af0161e28145e6300d9a5b3b1e4a387ed8f6c4ba7ac1a5f2948d8fac51`.
The ignored integration test was explicitly run with this fixture: two 12-second
visits, each exceeding its 10-second duration, more than 300 delivered frames
per visit, passed. Mailbox tests cover stale epochs, overdue/future frames and
lock contention; row-copy tests cover negative stride, padding, alpha and short
buffers. `docs/validation.md` records the separate in-game test results.

The red-top/blue-bottom H.264 integration fixture additionally verifies actual
Media Foundation buffer orientation after scaling from 640x360 to 320x180.

Primary API references:
[Source Reader](https://learn.microsoft.com/en-us/windows/win32/medfound/processing-media-data-with-the-source-reader),
[2D buffer layout](https://learn.microsoft.com/en-us/windows/win32/api/mfobjects/nn-mfobjects-imf2dbuffer),
[SetCurrentPosition](https://learn.microsoft.com/en-us/windows/win32/api/mfreadwrite/nf-mfreadwrite-imfsourcereader-setcurrentposition).
Seeking uses GUID_NULL and VT_I8 zero (100-nanosecond units); readers may seek
to a preceding keyframe, so timestamps are normalized from the first sample.

---


# 自定义视频播放：研究与设计记录

> 状态：**研究 / 设计，未实现，未在游戏内验证。**
> 记录日期：2026-10-23。
> 目标构建：beatmania IIDX 34 / 010-0917，
> `C:\Users\BeatMania IIDX LM\Downloads\010-0917bm2dx.dll`，
> 18,200,216 字节，SHA-256
> `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`，
> 镜像基址 `0x180000000`。实机部署目录：
> `\\2dx.lan.sh1mar.in\IIDX34`（游戏机上是 `D:\IIDX34`）。

本文回答一个问题：**用现在这套“换图”机制，怎么在同样的位置播放自定义视频？**
结论是可行，但视频不是“每 30 秒准备一张图”能覆盖的：它需要一条独立的解码线程、
一个有界的帧环形缓冲、逐帧的纹理刷新和一套按墙钟时间推进的播放会话。现有 hook
点已经足够，**不需要新的逆向 RVA**；另外 Spice SDK v0.4 的 `DRAW` 事件（目前插件
完全忽略）可以在任意屏幕矩形上叠加视频。

---

## 结论摘要

1. **渲染侧复用现有机制**：每个 surface 的替换点都已经在逐帧回调里，视频帧只要
   写进**同一张 D3D9 纹理**即可，不需要重新 `set_texture`/重新注册 AFP 纹理，也
   不动原生 clip 的位置、缩放和层级。
2. **解码用 Windows 自带的 Media Foundation Source Reader**：不增加 DLL，不引入
   FFmpeg。实测开启 `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING` 后，
   1080p60 H.264 → 1080p RGB32 可跑到 **178 fps**（GPU 解码 + 缩放），CPU 只负责
   每帧约 1.1 ms 的拷出。
3. **纹理刷新很便宜**：独立的 D3D9Ex 基准里，1920x1080 动态纹理每帧
   `LockRect(DISCARD)` + 逐行拷贝约 **0.9 ms**（流式写入 0.55 ms）。
   60 fps 视频在 120 Hz 游戏里隔帧刷新，平均约 0.45 ms/帧。
4. **游戏内已有数据不能直接当逐帧成本**：当前日志里的 “Texture ready …
   upload 3.8–9.3 ms” 是**每次进结算画面的一次性** `Texture::upload`，包含
   `CreateTexture` + 拷贝 + **AFP `texture_wrap` 注册**；它不是逐帧刷新成本。
   逐帧刷新必须在实机单独测量（见 §8 P0）。
5. **任意位置叠加**：Spice SDK v0.4 的 `SPICE_SDK_D3D9_DRAW`（事件 `1`）在所有
   Spice overlay 之上、每帧调用，宿主已绑定 backbuffer 并恢复图形状态。插件现在
   只处理 0/2/3，忽略事件 1；补上它就是“自定义矩形视频 overlay”的最短路径
   （仅限主屏 1920x1080，副屏 1280x720 不走 DRAW）。
6. **格式现实**：MF 原生支持 MP4/H.264；HEVC 需要系统安装 HEVC Video Extensions，
   VP9/AV1 取决于系统编解码器；**MKV/WebM 默认不支持**。第一版建议只承诺
   MP4/H.264（必要时 HEVC），要 WebM/VP9/AV1 再考虑可选的 FFmpeg 后端。

---

## 1. 现有图片管线是怎么工作的

### 1.1 数据流

```text
场景请求 (hook) ─► REQUEST +1 ─► 图像 worker
                                  ├─ 重新读 TOML（失败保留上一份）
                                  ├─ Provider 选图/下载
                                  ├─ image crate 解码 + 缩放成 canvas BGRA
                                  └─ Publish: PENDING(结算) / LATEST bundle(其它 surface)
图形线程 (presentation thread)
  ├─ 进入 visit 时冻结一次选择（迟到图片等下一次）
  ├─ Texture::upload：CreateTexture(DYNAMIC/DEFAULT, A8R8G8B8)
  │                    LockRect(DISCARD) → 逐行拷贝 → UnlockRect
  │                    → AfpTextureEx 包装/注册 → 40 字节 descriptor
  └─ 按 surface 绑定：clip 的 set_texture / 自建 sprite / 自绘 quad
```

关键代码：

| 位置 | 作用 |
|---|---|
| `src/runtime.rs` `Texture::upload/refresh` | 唯一创建/刷新 D3D 纹理的地方 |
| `src/runtime.rs` `Graphics::update` | 结算 `illust`：原回调先跑，再绑定自定义纹理 |
| `src/runtime/surfaces.rs` `movie_draw` | 选曲背景 quad；同时驱动 premium 刷新 |
| `cover_visual` / `graph_art` | SUDDEN+/LIFT、目标分数图的逐帧 visual 回调 |
| `sprite_release` / `invalidate` | 生命周期和设备丢失 |
| `src/state.rs` `Selection` | “一次 visit 固定一张图”的语义 |

### 1.2 每个 surface 的逐帧入口

| Surface | 绘制方式 | 逐帧回调（可刷新纹理） | 备注 |
|---|---|---|---|
| 结算插画 | 原生 clip `illust`，AFP 纹理 | `background_update`（slot 2，每帧） | 每帧原回调先执行；自定义纹理只在 visit 首次绑定 |
| 选曲背景 | 插件自绘 D3D9 全屏 quad | `movie_draw`（每帧） | 状态块保存/恢复，视频只要换纹理 |
| SUDDEN+ / LIFT | 插件自有 AFP sprite | `cover_visual`（每帧） | 原生 HIDDEN/LIFT/裁剪/alpha 仍生效 |
| 目标分数图 | 插件自有 AFP sprite | `graph_art`（每帧绘制时） | 原生 marker 裁剪保留 |
| Premium 区域 | 插件自有 AFP sprite | `movie_draw` / `select_update` / 结算 hook | 会话级节点，pool 释放后重建；部署日志目前持续报 `premium area native transform unavailable`，视频落地前要先修这个读取失败 |
| 集中模式 | 原生 clip 重绑纹理 | 目前只在 show/`set_archive` 时重绑 | 视频需要额外逐帧 tick（可挂在 `movie_draw`） |
| **任意矩形** | **D3D9 quad** | **SDK `DRAW` 事件（当前未处理）** | 在主屏所有 UI 之上 |

所有图形操作都在 presentation thread 上；worker 只做 IO/解码。这条边界必须保留。

---

## 2. 视频和图片的差异

| 维度 | 图片 | 视频 |
|---|---|---|
| 供给 | 每次 visit 一张，迟到等下次 | 连续帧，必须提前解码 |
| 生命周期 | visit 冻结 | 每次 visit 一个播放会话（可循环） |
| 时钟 | 无 | 墙钟起点 + PTS；允许丢帧/重复帧 |
| GPU | 一次上传 + 一次注册 | 同一纹理逐帧 `refresh`，**不重复注册** |
| 失败 | 回退原图 | 回退原图或上一帧；解码线程可停 |
| 音频 | 无 | **不播放**（第一版明确不做） |

因此不能把视频塞进“每 30 秒 query 一次”的 `Providers::prepare` 流程。文件选择可以
继续走 Provider（随机、排除上一首），但**帧的生产**要换成独立的播放会话。
另外 `Provider::collect` 目前只枚举 `jpg/jpeg/png`，视频需要独立的文件枚举
（同一目录/递归/随机/排除上一个），并且**绝不能**把视频字节送进 `pixels::decode`。

---

## 3. 渲染位置

### 3.1 原生 surface（替换背景）

视频帧就是一块 BGRA 像素。只要 canvas 尺寸不变，`Texture::refresh` 会原地重写
同一张纹理；AFP clip/sprite 引用的 descriptor 不变，所以：

- **不要**每帧调用 `set_texture` / `set_archive` / `sprite_create`；
- **不要**每帧重设 origin/scale（除非原生回调自己改了）；
- 只做 `LockRect(DISCARD)` + 行拷贝 + `UnlockRect`。

这与现有 0.2.1 的“纹理复用”完全一致，只是把“每次 visit 刷一次”改成“每次换帧刷一次”。

### 3.2 Spice SDK v0.4 的 DRAW 事件（新发现）

插件当前 `graphics(event_id, ...)` 只处理：

- `0 = READY`：记录 presentation thread，置 `READY`；
- `2 = INVALIDATE` / `3 = DESTROY`：释放默认池资源、恢复原图。

SDK 头文件（`spice2x.github.io`，`spicesdk.h`，v0.4）定义了第四个事件：

```c
typedef enum SPICE_SDK_D3D9_EVENT {
    SPICE_SDK_D3D9_READY = 0,
    SPICE_SDK_D3D9_DRAW = 1,
    SPICE_SDK_D3D9_INVALIDATE = 2,
    SPICE_SDK_D3D9_DESTROY = 3,
} SPICE_SDK_D3D9_EVENT;
```

`DRAW` 的语义（头文件原文要点）：

> runs on top of all Spice overlays, even when the overlay is closed; the host
> binds the backbuffer, brackets the scene, and restores graphics state …
> DRAW/READY run on the presentation thread … frame is valid only during the call.

`SPICE_SDK_D3D9_FRAME` 提供 `device`（真实 `IDirect3DDevice9*`，不是 Spice wrapper）、
`window`、`width`、`height`。这正好是“在任意屏幕矩形画一个视频 quad”的入口：

- 每帧在游戏和 Spice overlay **之上**绘制；
- 宿主已经 BeginScene/EndScene 并恢复状态；
- 插件自己仍应用状态块（和 `draw_selection` 一样）做防御；
- **只覆盖主呈现目标（1920x1080）**，副屏 1280x720 不在 DRAW 范围内。

如果用户要的是“在画面某个位置叠一段视频”，这条路径不需要碰 AFP；如果目标是
“替换游戏背景”，仍走 §3.1。

---

## 4. 解码方案

### 4.1 方案比较

| 方案 | 格式覆盖 | 额外文件 | 线程/延迟 | 结论 |
|---|---|---|---|---|
| **Media Foundation Source Reader** | MP4/H.264 内置；HEVC/VP9/AV1 视系统扩展；MKV/WebM 不支持 | 无（系统 DLL） | 独立 MFT，可高级视频处理（GPU） | **首选** |
| FFmpeg（libav* DLL） | 几乎全部（MKV/WebM/VP9/AV1） | 约 30–50 MB + LGPL 合规 | 多线程解码成熟 | 二选一后端，后续再加 |
| `ffmpeg.exe` 子进程管道 | 全部 | ffmpeg.exe | 每帧 8 MB 管道拷贝 + 进程管理 | 不推荐 |
| 纯 Rust 解码器 | H.264/AV1/VP9 需各自 C 绑定 | 视实现 | 可控 | 不划算 |

第一版建议：**MF + MP4/H.264**。在配置里明确“只支持 MF 能解的容器/编码”，失败时
保留原图并写日志，不静默黑屏。

### 4.2 MF 的关键配置

```rust
// 每个进程一次
CoInitializeEx(None, COINIT_MULTITHREADED)?;
MFStartup(MF_VERSION, 0)?;

// 每个播放会话一个 Source Reader
let attrs = MFCreateAttributes(&mut attrs, 2)?;
attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?; // 首选：GPU 解码 + 缩放
// 若打开/SetCurrentMediaType 失败：重建 reader，改用
// MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING（软件回退，原生尺寸）。
let reader = MFCreateSourceReaderFromURL(path, &attrs)?;
reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)?;

// 输出：目标 canvas 大小的 RGB32（top-down）
let media = MFCreateMediaType()?;
media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
media.SetUINT64(&MF_MT_FRAME_SIZE, pack(canvas_w, canvas_h))?;
media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &media)?;

// 逐帧
reader.ReadSample(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0, .., &mut flags, &mut pts, &mut sample)?;
let buffer = sample.ConvertToContiguousBuffer()?;
buffer.Lock(&mut data, None, Some(&mut len))?;
// data 是 top-down BGRA（stride 可能为 0 → 用 width*4）
buffer.Unlock()?;
```

循环：`IMFSourceReader::SetCurrentPosition(&GUID_NULL, PROPVARIANT{ VT_I8, 0 })` 回到
0，再 `Flush(FIRST_VIDEO_STREAM)`；`windows` 0.62 已导出该签名。若 seek 偶发卡顿，
可以在 worker 上提前 seek，或直接重建 Source Reader（代价是一次文件打开）。

---

## 5. 实测数据

环境：i5-12490F（6C/12T）、RTX 2070 SUPER、Windows 10 IoT Enterprise LTSC
10.0.19044、D3D9Ex。游戏机日志报告的 CPU/GPU 型号与 OS build（19044）相同，
但基准是在本机跑的，不是游戏进程内。基准工具见 `tools/video-bench`，
复现命令见 §9。

### 5.1 D3D9Ex 动态纹理刷新（独立基准）

1920x1080 隐藏窗口、同游戏一致的 `D3DCREATE_MULTITHREADED`、每帧用该纹理画全屏
quad、`D3DPRESENT_INTERVAL_IMMEDIATE`：

| 尺寸 | LockRect | memcpy 拷贝 | UnlockRect | 流式写入拷贝 | +NOSYSLOCK |
|---|---:|---:|---:|---:|---:|
| 1920x1080 | 0.01 ms | **0.90 ms** | 0.01 ms | 0.56 ms | 0.43 ms |
| 1280x720 | 0.02 ms | 0.36 ms | 0.00 ms | 0.18 ms | 0.18 ms |
| 960x540 | 0.21 ms | 0.08 ms | 0.00 ms | 0.08 ms | 0.09 ms |
| 640x360 | 0.04 ms | 0.04 ms | 0.00 ms | 0.04 ms | 0.04 ms |

- `D3DLOCK_DONOTWAIT` 在“刚画完就锁”时 120/120 全部失败（`WASSTILLDRAWING`），
  说明刚提交的帧仍在 GPU 上；要用它必须配多张纹理轮转。
- `CreateTexture` 本身很快（1080p 约 50–90 µs）；游戏内 8 ms 级的耗时不在创建，
  更可能来自 AFP 注册/首次使用。

### 5.2 Media Foundation 解码（1 MB、1080p60 H.264、600 帧）

| 输出配置 | Reader 属性 | 实际输出 | ReadSample | 拷出 BGRA | 合计吞吐 |
|---|---|---|---:|---:|---:|
| RGB32 1920x1080 原生 | 普通 video processing | 1920x1080 | 23.06 ms | 0.78 ms | **40.7 fps** |
| RGB32 1920x1080 | **advanced** | 1920x1080 | 0.39 ms | 1.13 ms | **178 fps** |
| RGB32 1280x720 | advanced | 1280x720 | 0.70 ms | 0.95 ms | **200 fps** |
| RGB32 960x540 | advanced | 960x540 | 0.44 ms | 0.48 ms | **393 fps** |
| RGB32 640x360 | advanced | 640x360 | 1.36 ms | 0.26 ms | **498 fps** |

补充：

- 普通 video processing + RGB32 在原生尺寸可工作，但只有 ~41 fps，不够 60 fps；
- 请求 NV12 输出、或非原生尺寸的 RGB32（不带 advanced），都会
  `MF_E_INVALIDMEDIATYPE`（0xC00D36B4）；
- advanced 路径输出的 `MF_MT_DEFAULT_STRIDE` 可能是 0，按 `width * 4` 处理；
- 两条路径的首帧都 dump 成 1920x1080 BMP 检查过：**方向都正确（天空在上），
  内存顺序 BGRA 与 D3D9 `A8R8G8B8` 一致**，不需要翻转或交换通道；
- 600 帧 1080p 全部读+拷完约 3.4 s，远快于实时。

### 5.3 游戏内已有日志（部署中的构建）

部署的 `iidxbg.dll` SHA-256
`08891ae5b19e4caac30c9a2f4ba39586411f2f42b793655aeb73d3e3825c6948`（不是本仓库
`dist/iidxbg.dll` 的哈希，属于另一份本地构建）。日志
`\\2dx.lan.sh1mar.in\IIDX34\modules\iidxbg.log` 抓取时：

```text
Texture ready: 1920x1080; upload … us
n=100  min=3797  p50=8357  p90=8972  max=9277  mean=7823 (µs)
```

这 100 次全部来自结算 surface 的 `Graphics::update`（`Event::Applied` 只在
`src/runtime.rs` 的结算路径打印；选曲/premium 的上传不打印这条日志）。它包含
创建 + 拷贝 + AFP `AfpTextureEx` 注册，**不是逐帧刷新**：结算纹理在 cleanup 时被
丢弃，下次 visit 重新创建；选曲/premium 的纹理才会跨 visit 复用。0.2.1 文档也把
~8 ms 归因于 1080p 首次使用。对视频来说，正确的做法是让视频纹理跨 visit 存活、
只 `refresh`，从而把 8 ms 的注册成本移出播放路径（或提前预热）。

**待实机确认的未知量**：`refresh` 在游戏进程里（Spice 的
`WrappedIDirect3DDevice9`、全屏 120 Hz、AFP 正在采样该纹理）到底是多少毫秒。
独立基准的 0.9 ms 只是下限/参考。

---

## 6. 设计

### 6.1 会话与生命周期

每个 surface 一个 `VideoSession`：

```text
场景进入 / visit 开始
  ├─ worker 选择一个视频文件（随机、排除上一个；沿用 Provider 语义）
  ├─ 打开 Source Reader，请求 canvas 尺寸 RGB32
  ├─ 解码前 2–3 帧 → ring buffer（第一帧就绪后才允许替换，否则本 visit 用原图）
  └─ 图形线程第一次绘制时冻结 session（和现在的 visit 冻结一致）
逐帧
  ├─ target = now - start（或随机起点）
  ├─ 取 PTS ≤ target 的最新帧；没有新帧就继续用上一帧
  ├─ 有新帧 → Texture::refresh（不重绑、不重注册）
  └─ 落后超过 N 帧 → 丢弃旧帧（不排队）
场景退出 / cleanup / 设备丢失
  ├─ 停解码线程，等待其退出（有界）
  ├─ 恢复原图/清空 sprite（沿用现有代码）
  └─ 纹理：设备丢失时丢弃；正常退出可保留以复用注册
```

保持“一个 visit 一个选择”的现有语义：**视频文件在 visit 内不变**，循环播放；
换歌/换 visit 才换文件。

### 6.2 线程与缓冲

- 一个解码线程/会话，优先级低于普通（和现有 image worker 一致）。
- 环形缓冲 3 帧（1080p BGRA 8.29 MB/帧 → 约 25 MB；720p 约 11 MB）。
- 发布接口：`Mutex<Option<Arc<Frame>>>` + `try_lock`（和现有 `PENDING` 同风格），
  或三缓冲 + 原子指针；图形线程只做一次 `Arc` clone，不等待解码。
- 解码线程在缓冲满时 sleep/条件变量等待，不忙转。
- 同时最多 2 个 1080p 会话（`max_concurrent`），其余 surface 可降到 720p/540p。
- MF 初始化和 COM 放在 worker 线程；`MFStartup` 每进程一次，`MFShutdown` 在 DLL
  destroy 时（进程即将退出，可不做也行，但要在日志里说明）。

### 6.3 帧调度

- 时钟：session 第一次绘制时 `Instant::now()`；用墙钟，不依赖游戏 tick。
- 目标帧：`floor(elapsed * fps)`，再取 PTS ≤ 该时间的帧；或直接按源 PTS。
- `fps_limit` 默认 60；120 Hz 下只在目标帧变化时上传（60 fps → 隔帧）。
- 解码落后：丢帧追上；缓冲空：重复最后一帧，并计数（日志每秒最多一条）。
- 循环：提前 seek，避免最后一帧到第一帧之间出现长空档；或预解码下一轮首帧。

### 6.4 上传

- 复用 `Texture::refresh`；尺寸不变是常态。
- 可选优化（在实机量过再启用）：
  - `D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK`；
  - 非临时（streaming）写入，1080p 拷贝 0.90 → 0.55 ms；
  - 小 canvas（720p/540p）配 clip/sprite 的 scale 放大（需要验证原生 scale 语义，
    第一版不启用）。
- 绝不在图形回调里分配 8 MB、解码、读文件或等锁。
- 诊断日志必须限速：每帧只累加计数，每秒最多写一行（p50/p90/p99、丢帧/重复帧）。
  当前部署的 `iidxbg.log` 已经因为重复诊断行增长到 39 MB；视频路径若每帧写日志会
  迅速拖垮磁盘和帧时间。

### 6.5 配置草案

```toml
# 新增顶层 [video]；现有 [sources] / [surfaces] 语义不变。
[video]
enabled = true
directory = 'iidxbg/videos'     # 每个 surface 默认取这里的随机文件
fps_limit = 60                  # 1–240；0 = 跟随源
loop = true
start = "beginning"             # "beginning" | "random"
fit = "cover"                   # cover | contain；contain 对视频是黑边，不做模糊背景
max_concurrent = 2
prefer_hardware = true          # false 强制软件解码
max_file_mb = 512               # 打开前的大小上限
log_interval_seconds = 5        # 播放诊断聚合间隔，0 = 不写

[video.results]                 # 可选：按 surface 覆盖
directory = 'iidxbg/videos/results'
file = ''                       # 非空则固定这个文件

[video.song_select]
directory = 'iidxbg/videos/song-select'

# 任意矩形 overlay（走 SDK DRAW，主屏 1920x1080）
[video.overlay]
enabled = false
file = ''
x = 1500
y = 60
width = 360
height = 202
opacity = 1.0
```

校验规则（沿用现在 `deny_unknown_fields` 的严格风格）：fps 1–240、矩形在
0..=1920/0..=1080 内、宽高 ≥ 16、opacity 0–1、文件扩展名白名单
（`.mp4`、`.m4v`、`.wmv`、`.avi`，以及系统支持时的 `.mov`/`.hevc`），
路径相对 TOML 解析。

UI（`ui/`）用 `toml_edit` 做格式保持编辑，新增 `[video]` 不会破坏它；但控件要等
后续版本再加。

### 6.6 自定义位置 overlay（DRAW）

```rust
extern "C" fn graphics(event: u32, frame: *const Frame, _: *mut c_void) {
    match event {
        1 => draw_video_overlay(frame),   // 新增；presentation thread
        0 => { /* READY */ }
        2 | 3 => { /* INVALIDATE/DESTROY */ }
        _ => {}
    }
}

unsafe fn draw_video_overlay(frame: *const Frame) {
    let f = &*frame;
    let device = IDirect3DDevice9::from_raw_borrowed(&f.device);
    let save = device.CreateStateBlock(D3DSBT_ALL)?;
    // 顶点 = 配置矩形，UV 0..1，颜色 ARGB(opacity)
    // SetVertexShader(None) / SetPixelShader(None) / SetFVF(0x144) /
    // SetTexture(0, video_texture) / 采样器线性 / 关闭 Z/裁剪/雾/模板 /
    // 开 alpha blend / DrawPrimitiveUP(TRIANGLEFAN, 2, ...)
    save.Apply()?;
}
```

注意：

- `frame` 只在回调期间有效；不要保存 `device` 指针到回调外。
- 宿主会恢复状态，但插件仍应使用状态块（和 `draw_selection` 一致）。
- 位置/大小是屏幕像素；`frame.width/height` 可用于 clamp。
- 这是**前景**叠加，会盖住游戏 UI；要“当背景”请用 §3.1。
- 副屏（1280x720）不触发主目标的 DRAW；premium/集中模式仍走 AFP 路径。

---

## 7. 风险与未知

| 风险 | 影响 | 处理 |
|---|---|---|
| 游戏内逐帧 `refresh` 可能远高于 0.9 ms（Spice wrapper、120 Hz、AFP 采样） | 60 fps 掉帧 | P0 实机测量；必要时降 canvas/fps、NOSYSLOCK/streaming |
| MF advanced 路径依赖 GPU/D3D11，远程桌面/老驱动可能失败 | 只能软解（1080p ~41 fps） | 自动回退普通 video processing + 原生尺寸；再失败回原图 |
| HEVC/VP9/AV1 需要系统扩展 | 用户视频打不开 | 日志给出明确原因；文档只承诺 H.264 |
| Windows N 版没有 Media Feature Pack | MF 直接不可用 | 启动时探测 `MFStartup`/`MFCreateSourceReaderFromURL`，失败就关闭视频功能并写日志 |
| MKV/WebM 不被 MF 支持 | 常见片源不可用 | 第一版不承诺；后续可选 FFmpeg 后端 |
| 无音频 | 与视频不同步的预期 | 文档明确“静音播放”，不做音频注入 |
| 多 surface 同时播放 | GPU 解码 + 内存带宽叠加 | `max_concurrent`、每 surface fps、默认只开结算/选曲 |
| 结算纹理每次 visit 重建（8 ms 注册） | 进结算瞬间卡一下 | 视频纹理跨 visit 复用或提前预热 |
| 设备丢失/重建 | 黑帧或悬空 descriptor | 按现有 `invalidate` 顺序释放；READY 后从当前帧重建 |
| `illust` 原回调可能每帧重绑原图 | 绑定被覆盖 | 现有代码只在 visit 首次绑定且实机可用；视频若遇到再改为“每帧只在被覆盖时重绑” |
| 副屏不覆盖 | overlay 只出现在主屏 | 文档写明；需要副屏时走 AFP |
| 编码色域/范围 | 颜色发灰/过饱和 | MF RGB32 输出按系统转换；实测对比截图确认 |
| 网络共享路径/大文件 | 打开慢、卡顿 | 只接受本地路径；打开前检查大小；解码线程内打开 |
| 逐帧诊断日志 | 日志膨胀、掉帧 | 每秒最多一行聚合；`log_interval_seconds` 可关 |

明确不做：音频、HDR、字幕、变速、倒放、视频编辑、把视频写进游戏 IFS、修改游戏
文件。

---

## 8. 分阶段实现与验证

**P0（先量再写）**：在当前插件里加一个诊断构建，只对 `Texture::refresh` 计时
（LockRect / 拷贝 / Unlock 分开，每秒输出 p50/p90/p99），在结算和选曲各跑一次。
同时用 `tools/video-bench` 在游戏机上跑 advanced 解码，确认 GPU 路径可用。
产出：决定 1080p60 是否成立，还是 720p/30fps。

**P1（离线解码）**：新增 `src/video.rs`（MF Source Reader、ring buffer、帧调度），
加 `iidxbg-tool video <file> <out.png>` 抽帧预览；纯逻辑单测（PTS 选择、循环、饥饿、
fps 上限、缓冲边界）。此时不碰游戏 hook。

**P2（结算画面）**：`[video]` 配置、worker 选择文件、session 启停、结算
`background_hook` 逐帧 `refresh`。实机验证：clear/failed/full combo、SP 两侧、
DP、Premium Free、重复进出、设备 reset、退出游戏。

**P3（选曲 + overlay）**：`movie_draw` 刷新选曲纹理；`graphics` 事件 1 画 overlay
（位置/大小/透明度）。实机验证层叠关系、UI 可读性、120 Hz 帧时间。

**P4（其余 surface）**：premium/集中/SUDDEN+/LIFT/目标图；加 `max_concurrent`
和每 surface fps；量多路同时播放的帧时间。

**P5**：配置界面、文档、打包、默认关闭。

每阶段的验证清单：视频开/关各跑同一段；记录 `iidxbg.log` 的刷新耗时分布；确认
原图回退；确认没有在图形回调里做 IO；`cargo test`、`cargo clippy -- -D warnings`、
`cargo build --release --locked`、`cargo fmt --check` 全过；明确写出“是否实机验证”。

---

## 9. 复现

```powershell
# 1. 基准工具（本机没有默认位置的 Windows SDK，需要指向独立 SDK 布局）
cd tools/video-bench
powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1 `
  -SdkRoot 'D:\Share\chart-requester\reference\sdk'
.\target\release\video-bench.exe upload
.\target\release\video-bench.exe decode 'D:\Share\iidxbg\target\sample-1080.mp4' rgb 1920 1080 scale 2

# 2. 1080p60 H.264 样本（1,046,987 字节）
#    https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/1080/Big_Buck_Bunny_1080_10s_1MB.mp4
#    SHA-256 38d763af0161e28145e6300d9a5b3b1e4a387ed8f6c4ba7ac1a5f2948d8fac51

# 3. 游戏内证据
#    \\2dx.lan.sh1mar.in\IIDX34\modules\iidxbg.log
#    Select-String -Path iidxbg.log -Pattern 'Texture ready'
```

静态证据（游戏构建、SDK、hook）见：

- [IIDX 34 / 010-0917 分析](iidx34-010-0917-analysis.md)
- [背景 surface 实现](background-surfaces.md)
- [架构](architecture.md)
- Spice SDK 头文件：
  <https://github.com/spice2x/spice2x.github.io/blob/main/src/spice2x/sdk/include/spicesdk.h>
- MF Source Reader：
  <https://learn.microsoft.com/windows/win32/api/mfreadwrite/nn-mfreadwrite-imfsourcereader>
- D3D9 动态纹理/LockRect：
  <https://learn.microsoft.com/windows/win32/direct3d9/d3dlock>
- D3D9 多线程与丢失设备：
  <https://learn.microsoft.com/windows/win32/direct3d9/multithreading-issues>

### 本轮新增/修改的文件

- `docs/video-surfaces.md`（本文）。
- `docs/development.md`：加入本研究的入口。
- `tools/video-bench/`：独立基准（D3D9 上传 + MF 解码 + 首帧 dump），不随插件发布。
- `.gitignore`：忽略该工具的 `target/`。

验证：`cargo test --locked`（27 passed）、`cargo clippy --all-targets --locked -- -D warnings`、
`cargo build --release --locked`、`cargo fmt --all --check` 均通过；
`tools/video-bench` 也用 `-SdkRoot` 完成构建并实跑。

**未修改任何游戏文件、游戏 DLL 或 `bm2dx.dll`；未安装/覆盖游戏目录里的插件。**
本轮所有数字来自独立基准和已有日志，**没有进行新的实机测试**。
