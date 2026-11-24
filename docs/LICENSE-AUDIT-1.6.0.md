# Trasea 1.6.0 — Dependency & License Audit

- **Date:** 2026-11-24
- **Total workspace crates:** 339 (unchanged from 1.5.x; 0 new crates added in 1.6.0)
- **Network crates in `trasea-core`:** **0**
- **GPL / AGPL crates in workspace:** **0**
- **External AI calls:** Handled exclusively inside `src-tauri/src/ai_engine.rs` via OS `curl` and `std::process::Command` on explicit user action; WebView2 / WKWebView CSP and network isolation remain unchanged.
- **License notice file:** `THIRD_PARTY_LICENSES.md` up to date.
