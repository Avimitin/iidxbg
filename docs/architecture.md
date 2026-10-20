# Version support

Ship one `iidxbg.dll` for IIDX, with an independently verified profile for each
supported build. This is explicit support for known binaries, not automatic
compatibility with older or newer versions.

Configuration, local selection, Konachan/Yande.re, downloading, decoding, pending pixels
and result-visit selection are shared. The native boundary selects a profile by
the game file's exact SHA-256, checks the PE image size, then checks live native
entry bytes and original vtable targets before installing hooks. A changed entry
is accepted only for the constrained MinHook chain described in
[the IIDX 33 compatibility investigation](iidx33-hook-compatibility.md).
Calls continue through the original game entry so the existing hook stays active.

The remote provider shares listing, downloading, decoding and save logic across
the three supported domains. A typed domain selects the API endpoint, permitted
image/redirect hosts and provider namespace. Konachan's two domains share a
namespace; Yande.re has a separate one for labels and saved filenames.
`[remote]` settings accept `[konachan]` as an alias for existing configurations.

Each profile owns its addresses, scene layout and hook slots. IIDX 33 uses
scene +256 and cleanup slot 14; IIDX 34 uses +264 and slot 17. Runtime code does
not infer these from a version number or scatter version conditionals through
the image pipeline. Unknown builds fail attachment and leave the game untouched.
Profiles are embedded in the DLL; they are not user-editable runtime JSON.

The inspected builds share the Microsoft x64 callback signatures and the
64-byte AFP texture wrapper layout, so they use the same native adapter.
A future build with different signatures, ownership or rendering needs a
separate adapter with its own evidence, still behind the same image pipeline.
Do not try to express incompatible behavior through offsets alone.

Separate version DLLs become justified if versions require incompatible
architectures/dependencies or independent release lifecycles. Neither is true
of these two inspected x64 builds. Separate artifacts now would duplicate
packaging and invite loading the wrong plugin without improving isolation.

See [IIDX 33 evidence](iidx33-analysis.md) and
[IIDX 34 evidence](iidx34-analysis.md). Static compatibility does not establish
in-game stability. The user confirmed IIDX 33 image display and corrected
placement; broader lifecycle testing remains. IIDX 34 has only offline validation.
