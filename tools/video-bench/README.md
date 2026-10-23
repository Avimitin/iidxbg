# video-bench

Scratch benchmark used by the [video research](../../docs/video-surfaces.md). It
is not part of the plugin and is never loaded by the game.

Two modes:

- `upload` creates a Windows D3D9Ex device with the same flags the game uses
  (`D3DCREATE_MULTITHREADED`, 1920x1080 backbuffer, `D3DPRESENT_INTERVAL_IMMEDIATE`),
  draws a full-screen quad from a dynamic `D3DPOOL_DEFAULT` texture every frame,
  and times `LockRect(D3DUSAGE_DYNAMIC)` / row copy / `UnlockRect` at 1920x1080,
  1280x720, 960x540 and 640x360. It compares a plain `memcpy` with streaming
  (non-temporal) stores, `D3DLOCK_NOSYSLOCK` and `D3DLOCK_DONOTWAIT`.
- `decode <file> [rgb|nv12] [w h] [native|scale] [0|1|2]` opens the file with a
  Media Foundation Source Reader, selects only the video stream, requests
  RGB32 or NV12 at the requested canvas, and times `ReadSample` plus the copy
  to an owned BGRA buffer. The last argument selects the reader attributes:
  `0` none, `1` `MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING`, `2`
  `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`.
- `dump <file> <out.bmp> [attrs] [w h]` decodes the first frame and writes a
  32-bit BMP so orientation and channel order can be inspected.

Example (the game machine has no Windows SDK in the default location, so pass
the standalone SDK layout):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1 -SdkRoot 'D:\Share\chart-requester\reference\sdk'
.\target\release\video-bench.exe upload
.\target\release\video-bench.exe decode 'D:\Share\iidxbg\target\sample-1080.mp4' rgb 1920 1080 scale 2
```

The 1080p60 H.264 fixture used in the recorded run is
`Big_Buck_Bunny_1080_10s_1MB.mp4` from test-videos.co.uk, 1,046,987 bytes,
SHA-256 `38d763af0161e28145e6300d9a5b3b1e4a387ed8f6c4ba7ac1a5f2948d8fac51`.
The fixture and the `target/` output are not committed.
