# msg-tool

msg-tool is a command-line tool for exporting, importing, packing, and unpacking script files.

## How to Compile

```bash
git clone https://github.com/lifegpc/msg-tool
cargo build --release  # Build with all features enabled
cargo build --release --no-default-features --features=circus  # Build with only specific features enabled. See supported types below.
```

## Basic Usage
### Extract messages from script files
```bash
msg-tool export <input> [output]
```
Some script files cannot be detected automatically. You can specify the type of script file with the `--script-type` / `-t` option.
```bash
msg-tool export -t <script-type> <input> [output]
```
If the script file is an image file, you can specify the output type of the image file with the `--image-type` / `-i` option.
```bash
msg-tool export -i webp <input> [output]
```
If the script file is an archive file, it will be unpacked and will try to extract messages/images/audio from the unpacked files. If you don't want to extract, please use the `unpack` command.

If the input is a directory, all script files in the directory will be processed. (The `-r` / `--recursive` option is needed if you want to process files in subdirectories.)

### Import data to script files
```bash
msg-tool import <input> <output> <patched>
```


### Pack files into an archive
```bash
msg-tool pack <input> -t <archive-type> [output]
```

### Unpack an archive file
```bash
msg-tool unpack <input> [output]
```
Some archive files cannot be detected automatically. You can specify the type of archive file with the `--script-type` / `-t` option.

### Create a new script file
```bash
msg-tool create -t <script-type> <input> <output>
```

## Supported Output Script Types
- `json` - [GalTransl](https://github.com/GalTransl/GalTransl)'s JSON format
- `m3t` - A simple text format that supports both original/llm/translated messages.

## Supported Image Types
| Image Type | Feature Name |
|---|---|
| `png` | `image` (enabled automatically if any image script types are enabled) |
| `jpg` | `image-jpg` |
| `webp` | `image-webp` |

## Supported Script Types
### Artemis Engine
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `artemis` | `artemis` | Artemis Engine AST file (.ast) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
| `artemis-asb` | `artemis` | Artemis Engine ASB file (.asb) | ✔️ | ✔️ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `artemis-arc`/`pfs` | `artemis-arc` | Artemis Engine archive file (.pfs) | ✔️ | ✔️ | `pf2` is not supported now |
### Buriko General Interpreter / Ethornell
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `bgi`/`ethornell` | `bgi` | Buriko General Interpreter/Ethornell Script | ✔️ | ✔️ | ❌ | ❌ | ❌ | Some old games' scripts cannot be detected automatically |
| `bgi-bsi`/`ethornell-bsi` | `bgi` | Buriko General Interpreter/Ethornell BSI Script (._bsi) | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `bgi-bp`/`ethornell-bp` | `bgi` | Buriko General Interpreter/Ethornell BP Script (._bp) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
| `bgi-dsc`/`ethornell-dsc` | `bgi-arc` | Buriko General Interpreter/Ethornell compressed file in archive | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `bgi-arc-v1`/`ethornell-arc-v1` | `bgi-arc` | Buriko General Interpreter/Ethornell Archive File Version 1 (.arc) | ✔️ | ✔️ | |
| `bgi-arc`/`bgi-arc-v2`/`ethornell-arc`/`ethornell-arc-v2` | `bgi-arc` | Buriko General Interpreter/Ethornell Archive File Version 2 (.arc) | ✔️ | ✔️ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `bgi-img`/`ethornell-img` | `bgi-img` | Buriko General Interpreter/Ethornell Uncompressed Image File | ✔️ | ✔️ | ❌ | ❌ | ✔️ | Image files in `sysgrp.arc` |
| `bgi-cbg`/`ethornell-cbg` | `bgi-img` | Buriko General Interpreter/Ethornell Compressed Image File | ✔️ | ❌  | ❌ | ❌ | ❌  | |
