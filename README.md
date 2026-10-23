# iidxbg

给 **beatmania IIDX (spice2x)** 用的背景替换插件。

把游戏里的结算插画换成你喜欢的图片或视频；也可以进一步替换选曲画面、SUDDEN+、LIFT、目标分数图和 Premium/集中模式背景。媒体可以来自固定文件、随机文件夹，图片也可以从 Moebooru 兼容 API 获取。

<img src="docs/config-gui.png" alt="iidxbg 配置界面" width="900">

*SUDDEN+ 配置界面：独立选择媒体，或绑定 LIFT / 目标分数图。*

## 这是什么

- 替换 IIDX 的 **结算画面插画**（默认功能）
- 可选替换 **选曲画面、SUDDEN+、LIFT、目标分数图** 等位置
- 支持固定图片/视频、文件夹随机、Moebooru 兼容 API，以及本地/在线混合
- 结算、选曲、SUDDEN+、LIFT、目标分数图支持静音循环视频；在对应组件中选择「固定图片 / 视频」
- 配置界面只有一个组件总览页面，没有全局选项和侧栏；结算也有独立开关
- SUDDEN+ 可绑定 LIFT，共享选中的媒体、解码器和纹理；绑定目标关闭时仍可作为来源
- 附带 **iidxbg-config.exe** 配置界面，不需要手动编辑配置文件
- 每次有效修改立即保存，没有“保存”按钮

## 怎么用

1. 从 [GitHub Releases](https://github.com/Avimitin/iidxbg/releases) 下载最新 ZIP。
   ZIP 里只有三个文件：
   - `iidxbg.dll`
   - `iidxbg.toml`
   - `iidxbg-config.exe`

2. 把这三个文件解压到 **spice2x 游戏目录**。

3. 双击 `iidxbg-config.exe`：
   - 给想替换的组件选择固定图片/视频，或随机文件夹；
   - 或者把图源切换为 Moebooru API，选择内置站点或填写兼容接口；
   - 修改会自动保存到旁边的 `iidxbg.toml`。

4. 在 spice64 的启动参数里加上：

   ```text
   -k "你的游戏目录\iidxbg.dll"
   ```

   保留你原有的其他启动参数。

5. 进入游戏即可。没有配置到图片、或关闭某个组件时，会保留游戏原图。

> ZIP 不包含示例图片。第一次打开配置程序时，本地预览为空是正常的；选好文件夹或在线图源后就会出现图片。

## 支持哪些版本

| 游戏版本 | 状态 |
|---|---|
| IIDX 33 / 20260812 | 已实机确认 |
| IIDX 34 / 20260917 | 离线验证 |
| IIDX 34 / 010-0917 | 已实机确认 FAILED/F 结算、选曲、目标分数图、SUDDEN+、LIFT 视频；覆盖范围见验证记录 |

要求：

- Windows x64
- spice2x，且支持 **Spice SDK v0.4 D3D9 callbacks**
- 只支持上表中的 x64 版本；其他版本不会加载

补充说明：

- **结算画面**：三个支持版本都可用。
- **选曲画面、SUDDEN+、LIFT、目标分数图**：需要单独开启，仍在继续测试。
- **Premium 区域 / 集中模式**：仅支持 IIDX 34 / 010-0917。
- 各构建、模式的验证范围见 [验证记录](docs/validation.md)。实际显示可能受游戏裁剪或遮罩影响。

结算视频推荐 H.264 MP4，其他格式取决于游戏机器安装的 Windows 解码器。
文件路径可绝对指定，也可相对于 `iidxbg.toml`；修改在后续结算生效。
解码失败时使用图片来源。当前只替换单曲结算背景，不替换游玩中的 BGA。
分辨率和已知限制见 [视频说明](docs/video-surfaces.md)。

## 下载

最新版本：[GitHub Releases](https://github.com/Avimitin/iidxbg/releases)

当前版本：**0.2.2**

## License

MIT
