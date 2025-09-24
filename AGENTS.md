# Repository Guidelines

## Project Structure & Module Organization
The CLI lives in `src/main.rs` with argument parsing in `src/args.rs` and data types in `src/types.rs`. Submodules under `src/format`, `src/scripts`, `src/output_scripts`, and `src/utils` hold codec implementations and shared helpers - mirror that layout when adding new game engines or formats. The procedural macro crate is in `msg_tool_macro/`; keep its API stable with the matching version declared in `Cargo.toml`. Sample game assets used for manual verification live under `testscripts/`, while patched reference outputs go in `patched/` and scratch artifacts in `output/`.
All scripts should implement the `Script` and `ScriptBuilder` trait in `src/scripts/base.rs`. New script's type should be registered in `src/types.rs`. The corresponding script builder should be registered in `src/scripts/mod.rs`. If new flag are added, please register them in `src/args.rs` and `src/types.rs` (`ExtraConfig`).
Some useful utilities are in `src/utils/`. Some utilities should enabled via feature flags in `Cargo.toml`.

## Coding Style & Naming Conventions
Target the Rust 2024 edition with `rustfmt` defaults (4-space indentation, trailing commas). Modules and files stay in `snake_case`, public types in `PascalCase`, and flags/features use the hyphenated scheme already present (e.g., `bgi-arc`). Prefer explicit `use` blocks near call sites and annotate complex transforms with concise comments. Keep CLI option identifiers aligned with the conventions in `src/args.rs`.
DO NOT USE ANY CODE CAN CAUSE PANIC IN LIBRARY CODE.
panic only allowed in main.rs , args.rs and tests.

## Core Utilities (`src/utils/`)
- `counter.rs` - Thread-safe counters summarizing script outcomes; used to report OK/ignored/error/warning totals. Use `crate::COUNTER` to get global instance.
- `encoding.rs` - Shared encode/decode helpers with BOM detection, replacement handling, and optional Kirikiri wrappers for MDF and SimpleCrypt payloads.
- `files.rs` - Path utilities to collect inputs, filter by known script or archive extensions, stream stdin/stdout, and sanitize Windows file names.
- `struct_pack.rs` - Traits plus blanket implementations for binary pack/unpack backed by the `msg_tool_macro` crate; used when codecs read or write structured data.
- Feature-gated helpers such as `bit_stream.rs` or `threadpool.rs` stay under the same module; enable them via the matching `utils-*` features in `Cargo.toml`.
