# msg-tool

msg-tool is a command-line tool for exporting, importing, packing, and unpacking script files.

## How to Compile

```bash
git clone https://github.com/lifegpc/msg-tool
cargo build --release  # Build with all features enabled
cargo build --release --no-default-features --features=circus  # Build with only specific features enabled. See supported types below.
```

## Exit Codes
By default, msg-tool will always return exit code 0 unless a exit signal is received (such as Ctrl+C).  
You can use the `--exit-code` / `-x` option to specify a non-zero exit code when some jobs failed.  
If all jobs failed, you can use the `--exit-code-all-failed` / `-X` option to specify a different exit code.

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
# Pack multiple files/folders into an archive
# If output is not specified, the archive file will be named with the first input's name with the appropriate extension.
# Use --dep-file xxxx.d to generate a dep file for other build systems. (such as ninja)
msg-tool pack-v2 -t <archive-type> -o <output> <input1> <input2> ...
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
- `m3t` / `m3ta` - A simple text format that supports both original/llm/translated messages.
- `yaml` - Same as `json`, but in YAML format.
- `po`/`pot` - Gettext PO/POT format.

## Supported Image Types
| Image Type | Feature Name |
|---|---|
| `png` | `image` (enabled automatically if any image script types are enabled) |
| `jpg` | `image-jpg` |
| `webp` | `image-webp` |
| `jxl` | `image-jxl` |

## Supported Script Types
### Artemis Engine
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `artemis` | `artemis` | Artemis Engine AST file (.ast) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `artemis-asb` | `artemis` | Artemis Engine ASB file (.asb/.iet) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | For `.iet` files, only custom export/import and create features are supported. |
| `artemis-txt` | `artemis` | Artemis Engine TXT (General) script | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `artemis-panmimisoft-txt` | `artemis-panmimisoft` | Artemis Engine TXT ([ぱんみみそふと](https://pannomimi.net/panmimisoft)) file (.txt) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `artemis-arc`/`pfs` | `artemis-arc` | Artemis Engine archive file (.pfs) | ✔️ | ✔️ | |
| `artemis-pf2`/`pf2` | `artemis-arc` | Artemis Engine Archive File (.pfs) (pf2) | ✔️ | ✔️ | |
### Buriko General Interpreter / Ethornell
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `bgi`/`ethornell` | `bgi` | Buriko General Interpreter/Ethornell Script | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ❌ | Some old games' scripts cannot be detected automatically |
| `bgi-bsi`/`ethornell-bsi` | `bgi` | Buriko General Interpreter/Ethornell BSI Script (._bsi) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `bgi-bp`/`ethornell-bp` | `bgi` | Buriko General Interpreter/Ethornell BP Script (._bp) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `bgi-dsc`/`ethornell-dsc` | `bgi-arc` | Buriko General Interpreter/Ethornell compressed file in archive | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |

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
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `cat-system` | `cat-system` | CatSystem2 Scene Script File (.cst) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `cat-system-cstl` | `cat-system` | CatSystem2 Scene I18N File (.cstl) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `cat-system-int` | `cat-system-arc` | CatSystem2 Archive File (.int) | ✔️ | ❌ | Encrypted archives are supported too. Use `--cat-system-int-encrypt-password` to specify password |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `cat-system-hg3` | `cat-system-img` | CatSystem2 HG3 Image File (.hg3) | ✔️ | ❌ | ✔️ | ❌ | ❌ | |
### Circus
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `circus` | `circus` | Circus Script File (.mes) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | Some scripts must use `--circus-mes-type` to specify game |

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
| `circus-crx` | `circus-img` | Circus Image File (.crx) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | V1 is not supported when importing/creating image |
| `circus-crxd` | `circus-img` | Circus Differential Image File (.crx) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### Emote
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `emote-psb`/`psb` | `emote-img` | Emote PSB File | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `emote-pimg` | `emote-img` | Emote Multiple Image File (.pimg) | ❌ | ❌ | ❌ | ❌ | ✔️ | ❌ | ❌ | `--emote-pimg-psd` is required. |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `emote-pimg`/`pimg` | `emote-img` | Emote Multiple Image File (.pimg) | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `emote-dref`/`dref` | `emote-img` | Emote DPAK-referenced Image File (.dref) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### Entis GLS engine
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `entis-gls` | `entis-gls` | Entis GLS engine XML Script (.srcxml) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `entis-gls-csx` | `entis-gls` | Entis GLS engine CSX Script (.csx) | ✔️ | ✔️ | ✔️ | ✔️ | ✔️ | ✔️ | ❌ | |
### Escu:de
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `escude` | `escude` | Escu:de Script File (.bin) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `escude-list` | `escude` | Escu:de List File (.bin) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `escude-arc` | `escude-arc` | Escu:de Archive File (.bin) | ✔️ | ✔️ | |
### ExHibit
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `ex-hibit` | `ex-hibit` | ExHibit Script File (.rld) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `ex-hibit-grp` | `ex-hibit-arc` | ExHibit GRP Archive File (.grp) | ✔️ | ❌ | |
### Favorite
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `favorite` | `favorite` | Favorite Hcb Script (.hcb) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ❌ | ❌ | |
### HexenHaus
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `hexen-haus` | `hexen-haus` | HexenHaus Script File (.bin) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `hexen-haus-arcc` | `hexen-haus-arc` | HexenHaus Arcc Archive File (.arc) | ✔️ | ❌ | |
| `hexen-haus-odio` | `hexen-haus-arc` | HexenHaus Audio Archive File (.bin) | ✔️ | ❌ | |
| `hexen-haus-wag` | `hexen-haus-arc` | HexenHaus Wag Archive File (.wag) | ✔️ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `hexen-haus-png` | `hexen-haus-img` | HexenHaus PNG Image File (.png) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### Kirikiri
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `kirikiri`/`kr`/`kr-ks`/`kirikiri-ks` | `kirikiri` | Kirikiri Script File (.ks) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `kirikiri-scn`/`kr-scn` | `kirikiri` | Kirikiri Scene File (.scn) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ❌ | |
| `kirikiri-simple-crypt`/`kr-simple-crypt` | `kirikiri` | Kirikiri Simple Crypt Text File | ❌ | ❌ | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `kirikiri-mdf`/`kr-mdf` | `kirikiri` | Kirikiri Zlib-Compressed File | ❌ | ❌ | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `kirikiri-tjs-ns0`/`kr-tjs-ns0` | `kirikiri` | Kirikiri TJS NS0 binary encoded script | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `kirikiri-tjs2`/`kr-tjs2` | `kirikiri` | Kirikiri compiled TJS2 script | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `kirikiri-xp3`/`kr-xp3`/`xp3` | `kirikiri-arc` | Kirikiri XP3 Archive File (.xp3) | ✔️ | ✔️ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `kirikiri-tlg`/`kr-tlg` | `kirikiri-img` | Kirikiri TLG Image File (.tlg) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | tlg6 is not supported when importing/creating image |
### Musica
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `musica` | `musica` | Musica Script File (.sc) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `musica-arc` | `musica-arc` | Musica Archive Resource File (.paz) | ✔️ | ✔️ | |
### QLIE
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `qlie` | `qlie` | Qlie Engine Scenario script (.s) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `qlie-abmp10` / `qlie-abmp11` / `qlie-abmp12` | `qlie-img` | Qlie Abmp10/11/12 image (.b) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `qlie-dpng` | `qlie-img` | Qlie tiled PNG image (.png) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ❌ | `--qlie-dpng-psd` is required. |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `qlie-pack` | `qlie-arc` | Qlie Pack Archive (.pack) | ✔️ | ✔️ | Currently only v3.1 are supported. `--backslash` are needed to correctly handle file paths when packing. |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `qlie-dpng` | `qlie-img` | Qlie tiled PNG image (.png) | ✔️ | ✔️ | ❌ | ❌ | ❌ | |
### Silky Engine
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `silky` | `silky` | Silky Engine Mes Script File (.mes) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
| `silky-map` | `silky` | Silky Engine Map File (.map) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
### Softpal
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `softpal` | `softpal` | Softpal Script File (.src) | ✔️ | ✔️ | ✔️ | ✔️ | ✔️ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `softpal-pac` | `softpal-arc` | Softpal Pac Archive File (.pac) | ✔️ | ❌ | |
| `softpal-pac-amuse` | `softpal-arc` | Softpal Amuse Pac Archive File (.pac) | ✔️ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `softpal-pgd-ge`/`pgd-ge`/`pgd` | `softpal-img` | Softpal PGD Ge Image File (.pgd) | ✔️ | ✔️ | ❌ | ❌ | ✔️ | |
| `softpal-pgd3`/`softpal-pgd2`/`pgd3`/`pgd2` | `softpal-img` | Softpal PGD Differential Image File (.pgd) | ✔️ | ❌ | ❌ | ❌ | ❌ | |
### WillPlus / AdvHD
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `will-plus-ws2`/`adv-hd-ws2` | `will-plus` | WillPlus/AdvHD Script File (.ws2) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |

| Image Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Create | Remarks |
|---|---|---|---|---|---|---|---|---|
| `will-plus-wip`/`adv-hd-wip` | `will-plus-img` | WillPlus/AdvHD WIP Image File (.wip) | ✔️ | ❌ | ✔️ | ❌ | ❌ |  |
### Yaneurao Itufuru
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `yaneurao-itufuru`/`itufuru` | `yaneurao-itufuru` | Yaneurao Itufuru Script File | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |

| Archive Type | Feature Name | Name | Unpack | Pack | Remarks |
|---|---|---|---|---|---|
| `yaneurao-itufuru-arc`/`itufuru-arc` | `yaneurao-itufuru-arc` | Yaneurao Itufuru Archive File (.scd) | ✔️ | ✔️ | |

### Yu-Ris
| Script Type | Feature Name | Name | Export | Import | Export Multiple | Import Multiple | Custom Export | Custom Import | Create | Remarks |
|---|---|---|---|---|---|---|---|---|---|---|
| `yuris-yscm` | `yuris` | Yu-Ris YSCM(opcodes metadata) file (.ybn) | ❌ | ❌ | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `yuris-yser` | `yuris` | Yu-Ris YSER(error message) file (.ybn) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `yuris-yscfg` | `yuris` | Yu-Ris YSCFG(config) file (.ybn) | ❌ | ❌ | ❌ | ❌ | ✔️ | ✔️ | ✔️ | |
| `yuris-ystb` | `yuris` | Yu-Ris YSTB(compiled script) file (.ybn) | ❌ | ❌ | ❌ | ❌ | ✔️ | ❌ | ❌ | |
| `yuris-txt` | `yuris` | Yu-Ris scenario text file (.txt) | ✔️ | ✔️ | ❌ | ❌ | ❌ | ❌ | ❌ | |
