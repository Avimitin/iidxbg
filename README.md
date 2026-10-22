# iidxbg

给 **beatmania IIDX (spice2x)** 用的背景替换插件。

把游戏里的结算插画换成你喜欢的图片；也可以进一步替换选曲画面、SUDDEN+、LIFT、目标分数图和 Premium/集中模式背景。图片可以来自本地文件夹，也可以从 Konachan / Yande.re 在线获取。

<img src="docs/config-gui.png" alt="iidxbg 配置界面" width="900">

*配置界面示意：用本地图片填充后的组件总览。*

## 这是什么

- 替换 IIDX 的 **结算画面插画**（默认功能）
- 可选替换 **选曲画面、SUDDEN+、LIFT、目标分数图** 等位置
- 支持本地图片、在线图源，以及本地/在线混合
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
   - 给想替换的组件选择本地图片文件夹；
   - 或者把图源切换为 Konachan / Yande.re；
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
| IIDX 34 / 010-0917 | 离线验证 |

要求：

- Windows x64
- spice2x，且支持 **Spice SDK v0.4 D3D9 callbacks**
- 只支持上表中的 x64 版本；其他版本不会加载

补充说明：

- **结算画面**：三个支持版本都可用。
- **选曲画面、SUDDEN+、LIFT、目标分数图**：需要单独开启，仍在继续测试。
- **Premium 区域 / 集中模式**：仅支持 IIDX 34 / 010-0917。
- IIDX 34 目前只有离线验证；实际游戏内的显示位置可能会被游戏本身裁剪或遮罩。

## 下载

最新版本：[GitHub Releases](https://github.com/Avimitin/iidxbg/releases)

当前版本：**0.2.2**

## License

MIT
