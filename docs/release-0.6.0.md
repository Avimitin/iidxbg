# 0.6.0

Adds a multi-user application flow with a searchable, compact account switcher,
multiple cards per user, and per-user Bilibili bindings. Display modules follow
the logged-in user; unknown cards and guests use stock user modules. Bilibili
sessions and media caches are isolated by user and configuration.

Game asset replacements are shared across all users. The title-bar **游戏素材**
button opens with one click, with title/login preview cards above the catalog.
The page works without users and retains its shared scope when switching users,
closing an editor or reloading configuration. Existing per-user artwork entries
are migrated to shared replacements; conflicting entries are rejected.

The UI has permanent, light module captions, transparent buttons, a compact
title bar and a bundled Windows icon. Editors open on **设置**; **媒体来源** is
shown for **单独设置**. User/card management and Bilibili binding share one page.

Validation: 87 core tests and 36 UI tests passed, with 7 and 1 optional tests
ignored respectively. Clippy with warnings denied, formatting and release builds
passed. Desktop checks covered navigation, user switching, shared saves and
button states. The user reported the deployed build tested OK on 2026-10-26;
the exact scenarios were not specified. No additional agent-run in-game testing
was performed. Exact-build native guards and graphics lifetime rules remain in
place; see [user sessions](user-sessions.md) for native evidence and remaining
live-test limits.
