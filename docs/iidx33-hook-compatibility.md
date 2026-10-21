# IIDX 33 live entry mismatch — 2026-10-21

## Observed evidence

The reported installation is `\\2dx.lan.sh1mar.in\IIDX33`.
`modules/iidxbg.log` contains:

```text
Startup failed: Unsupported or malformed game DLL: native entry guard mismatch
```

The installed game file is 18,467,992 bytes, SHA-256
`c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`,
exactly the supported IIDX 33 / 20260812 profile. Installed iidxbg SHA-256:
`5f77e848231f801ae39027b85e232a08b3c9eb4e5374dafb016ba131e02212f8`.

The run log records the game loaded at `0x180000000` at 23:53:53 on 2026-10-20,
and SDK initialization for iidxbg at 23:54:01, after the other plugins loaded.
These include 2dx-gsm, fervidex, ifs_hook, playlister, 2dxtra, bilimani and
omnifix. This is a list of possible interacting components, not attribution
of the modification to any one plugin.

The old runtime checked SHA-256 and image size before entry bytes. The reported
failure therefore means a live code difference, not a malformed file. ASLR
was not involved in this run: the recorded load base equals the preferred base.
Comparing every `offset`/`dataEnabled`/`dataDisabled` range in the installed
`patches/LDJ-6a713632_b1cccc.json` with the profile's PE-converted guard ranges
found no overlap. The older 2026-09-27 minidump contains none of these guarded
entries and cannot identify the current difference.

## Chart-requester precedent

Commit `3157c1e89df30c42a161c56fce55698bab2f3522` introduced IIDX input
compatibility for a MinHook relay used by 2dxtra. Current implementation:
`crates/bilimani-iidx/src/versions/v33/b20260812/input_hook.rs`, with installation
in the adjacent `native.rs` and memory/module checks in `src/host/windows.rs`.

That implementation:

1. Accepts the original bytes unchanged.
2. Otherwise requires `E9 rel32`, with bytes 5–15 exactly matching the original.
3. Checks that the relay is a separate executable address.
4. Requires the 14-byte relay `FF 25 00 00 00 00 <u64 destination>`.
5. Requires a separate executable destination in an already loaded module
   outside bm2dx.dll.
6. Leaves the entry detour intact, wraps the vtable slot, and calls the original
   game entry so the plugin and its trampoline retain their place in the chain.

It also handles an independently verified Omnifix database accessor change
(`LEA` to `MOV`). That data-buffer compatibility rule is unrelated to this
results background feature and is not copied here.

## iidxbg implementation

`src/hook_chain.rs` applies the same constrained relay recognition to the native
entry guards in all exact-build profiles. It does not accept arbitrary jumps,
changed tail bytes, invalid relays, non-executable targets or unowned targets.
Runtime checks use `ReadProcessMemory`, `VirtualQuery` and existing-module lookup.
No discovered destination is executed during validation.

All native function calls still resolve to `bm2dx base + profile RVA`. No original
entry bytes are replaced and no discovered trampoline is called directly.
The three vtable slots must still point at their original profile targets before
installation; an already replaced vtable slot remains unsupported.

This is structural chain compatibility, not proof of an arbitrary plugin's
semantics or unchanged function signature. The Win64 function signatures,
object layouts and texture lifetime evidence remain those of the two build
analysis documents.

Logging now separates file identity from live-code validation. It records:

- Verified build/hash and loaded base.
- Accepted chains, with function name, RVA, relay, target and owning module.
- Every rejected entry with expected/actual bytes and chain rejection reason.
- Rejected vtable slots with expected/actual addresses.
- Bounded jump tracing for rejected entries where the form can be decoded.

Any rejection occurs before installing iidxbg's hooks. The misleading
"malformed game DLL" category is no longer used for live entry/vtable conflicts.

## Reproduction and remaining unknowns

Unit tests cover all profiles' original guards, recognized chains, changed
tail bytes, truncated/invalid relays, non-executable addresses, absent module
ownership and self-referential targets. `scripts/smoke-attach.py` maps a supplied
game DLL in a private test process without running its entry point/imports.
It tests unmodified attachment, synthetic MinHook attachment, and rejection
before vtable mutation when a byte after the jump is changed:

```powershell
python scripts/smoke-attach.py target/release/iidxbg.dll C:\path\bm2dx.dll --mode native
python scripts/smoke-attach.py target/release/iidxbg.dll C:\path\bm2dx.dll --mode chain
python scripts/smoke-attach.py target/release/iidxbg.dll C:\path\bm2dx.dll --mode bad-tail
```

No game function or graphics callback runs in these tests; no file is patched.
The synthetic target is an executable Windows export used only for ownership
validation. It is never called as a game function.

The original log contained no function RVA or bytes, so it could not identify
the changed entry or responsible plugin. The follow-up log below resolves those
unknowns. Reset/cleanup coverage and frame times still require validation.

Follow-up live log: the actual modified entry is `result_cleanup`, RVA
`0x92d970`, with relay `0x17ffe0086` leading to
`C:\IIDX33\modules\2dx-gsm.dll` at `0x7ffc218866a0`.
The updated DLL accepted this chain and logged successful attachment and a
1920x1080 upload. The user confirmed that an image appears, with a placement
problem investigated and subsequently confirmed fixed by the user in
[the IIDX 33 build analysis](iidx33-analysis.md).
This confirms this installation's cleanup-chain compatibility, not every plugin
or complete graphics lifecycle validation.
