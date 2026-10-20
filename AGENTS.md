# iidxbg development

- This is a standalone Windows x64 spice2x plugin, output `iidxbg.dll`.
- Read docs/architecture.md and both docs/iidx33-analysis.md and
  docs/iidx34-analysis.md before changing native code.
- Record every new analysis with input hash, RVAs, ABI/lifetime evidence,
  reproduction steps and unknowns. Never infer compatibility from version labels.
- Keep provider IO and decoding off game/graphics callbacks. Publish owned pixels.
- Keep one image fixed per result visit. Late completion waits for another visit.
- Native/Direct3D objects belong to the graphics owner. Preserve original calls,
  reset handling, stock fallback and exact-build guards.
- Do not write game assets or game DLLs. Do not add game files to the repository.
- Verify with tests, clippy and release builds. State explicitly whether actual
  in-game testing was performed.

