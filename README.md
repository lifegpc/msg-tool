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

### Import data into script files
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

| Audio Type | Feature Name | Name | Export | Import | Create | Remarks |
|---|---|---|---|---|---|---|
| `bgi-audio`/`ethornell-audio` | `bgi-audio` | Buriko General Interpreter/Ethornell Audio File (Ogg/Vorbis) | ✔️ | ❌ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `bgi-img`/`ethornell-img` | `bgi-img` | Buriko General Interpreter/Ethornell Uncompressed Image File | ✔️ | ✔️ | ❌ | ❌ | ✔️ | Image files in `sysgrp.arc` |
| `bgi-cbg`/`ethornell-cbg` | `bgi-img` | Buriko General Interpreter/Ethornell Compressed Image File | ✔️ | ✔️  | ❌ | ❌ | ✔️  | V2 is not supported when importing/creating image |
### CatSystem2
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `cat-system` | `cat-system` | CatSystem2 Scene Script File (.cst) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
| `cat-system-cstl` | `cat-system` | CatSystem2 Scene I18N File (.cstl) | ✔️ | ✔️ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `cat-system-int` | `cat-system-arc` | CatSystem2 Archive File (.int) | ✔️ | ❌ | Encrypted archives are supported too. Use `--cat-system-int-encrypt-password` to specify password |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `cat-system-hg3` | `cat-system-img` | CatSystem2 HG3 Image File (.hg3) | ✔️ | ❌ | ✔️ | ❌ | ❌ | |
### Circus
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `circus` | `circus` | Circus Script File (.mes) | ✔️ | ✔️ | ❌ | ❌ | ❌ | Some scripts must use `--circus-mes-type` to specify game |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `circus-crm` | `circus-arc` | Circus Image Archive File (.crm) | ✔️ | ❌ | |
| `circus-dat` | `circus-arc` | Circus Archive File (.dat) | ✔️ | ❌ | |
| `circus-pck` | `circus-arc` | Circus Archive File (.pck/.dat) | ✔️ | ✔️ | |

| Audio Type | Feature Name | Name | Export | Import | Create | Remarks |
|---|---|---|---|---|---|---|
| `circus-pcm` | `circus-audio` | Circus Audio File (.pcm) | ✔️ | ❌ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `circus-crx` | `circus-img` | Circus Image File (.crx) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | |
| `circus-crxd` | `circus-img` | Circus Differential Image File (.crx) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### Escu:de
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `escude` | `escude` | Escu:de Script File (.bin) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
| `escude-list` | `escude` | Escu:de List File (.bin) | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `escude-arc` | `escude-arc` | Escu:de Archive File (.arc) | ✔️ | ✔️ | |
### ExHibit
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `ex-hibit` | `ex-hibit` | ExHibit Script File (.rld) | ✔️ | ✔️ | ✔️ | ✔️ | ❌ | |
### HexenHaus
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `hexen-haus` | `hexen-haus` | HexenHaus Script File (.bin) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
### Kirikiri
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `kirikiri`/`kr`/`kr-ks`/`kirikiri-ks` | `kirikiri` | Kirikiri Script File (.ks) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
| `kirikiri-scn`/`kr-scn` | `kirikiri` | Kirikiri Scene File (.scn) | ✔️ | ✔️ | ✔️ | ✔️ | ❌ | Patched script may be broken |
| `kirikiri-simple-crypt`/`kr-simple-crypt` | `kirikiri` | Kirikiri Simple Crypt Text File | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `kirikiri-mdf`/`kr-mdf` | `kirikiri` | Kirikiri Zlib-Compressed File | ❌ | ❌ | ✔️ | ❌ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `kirikiri-tlg`/`kr-tlg` | `kirikiri-img` | Kirikiri TLG Image File (.tlg) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
| `kirikiri-pimg`/`kr-pimg` | `kirikiri-img` | Kirikiri Multiple Image File (.pimg) | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `kirikiri-dref`/`kr-dref` | `kirikiri-img` | Kirikiri DPAK-referenced Image File (.dref) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### WillPlus
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `will-plus-ws2` | `will-plus` | WillPlus Script File (.ws2) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
### Yaneurao Itufuru
| Script Type | Feature Name | Name | Export | Import | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `yaneurao-itufuru`/`itufuru` | `yaneurao-itufuru` | Yaneurao Itufuru Script File | ✔️ | ✔️ | ❌ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `yaneurao-itufuru-arc`/`itufuru-arc` | `yaneurao-itufuru-arc` | Yaneurao Itufuru Archive File (.scd) | ✔️ | ✔️ | |
