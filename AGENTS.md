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

## Command line tools
Use UNIX tools to list directory contents, search text, and compare files. For example: `ls`, `find`, `grep`.
POWERHSELL COMMANDS ARE NOT ALLOWED.

## IO Extensions (`src/ext/io.rs`)
For endian-aware operation, default to little-endian, unless `_be` suffix is used. `_le` suffix are not used in the code.
- **`Peek` (for `Read + Seek`)** – adds non-destructive read helpers: generic `peek`/`peek_at`, typed endian-aware primitives (`peek_u8`…`peek_i128_be`), string readers (`peek_cstring`, `peek_fstring`, UTF-16 helpers), struct loaders via `StructUnpack` (`read_struct`, `read_struct_vec`), and assertions (`peek_and_equal`).
- **`CPeek` (thread-safe peek facade)** – mirrors `Peek` on shared references with exact/offset variants, typed primitives, string helpers, and matchers; implemented for `Mutex<T: Peek>`, in-memory readers, and writers.
- **`ReadExt` (for `Read`)** – provides endian-aware reads for integers/floats, C/fixed/UTF-16 string loaders using `Encoding`, vector reads (`read_exact_vec`), and equality assertions (`read_and_equal`).
- **`WriteExt` (for `Write`)** – exposes endian-aware writers for integers, signed values, floats, C-strings, plus `write_struct` delegating to `StructPack` with encoding awareness.
- **`WriteAt` (for `Write + Seek`)** – writes at absolute offsets with typed wrappers (`write_u8_at`…`write_i128_be_at`) and C-string support, restoring the original cursor after each call.
- **`SeekExt` (for `Seek`)** – offers `stream_length` (length without disturbing position) and `align` (rounds the cursor up to a power-of-two boundary).
- **`MemReader` / `MemReaderRef`** – byte-slice backed readers implementing `Read`, `Seek`, `Peek`, and `CPeek`; support cloning, EOF checks, borrowing via `to_ref`, and safe position management.
- **`MemWriter`** – growable in-memory writer implementing `Write`, `Seek`, and `CPeek`; supports constructing from/into `Vec<u8>` and borrowing via `to_ref`.
- **`StreamRegion<T: Seek>`** – wraps a stream segment with bounded `Read`/`Seek`, keeping track of local position and guarding seeks outside the declared range.
- **`BinaryPatcher<R, W, A, O>`** – coordinates incremental patching by copying sections, mapping offsets between old/new layouts, replacing ranges with provided writers, and patching values/addresses through user-supplied address↔offset closures.
