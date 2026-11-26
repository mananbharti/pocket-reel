# Reverse-engineering findings: "Video Convert Tool"

This document records what the original Windows tool does and with which exact
settings. It was produced by static analysis only. Nothing from the original
package (code, binaries, images, text) is reused in this project. Every format
detail below was re-implemented from scratch in [`crates/engine`](../crates/engine).

Legend used throughout:

- **Observed**: read directly from bytes, tables or disassembly of the shipped files.
- **Inferred**: a conclusion drawn from observed code, not seen executing.
- **Unknown**: not established.

## 1. What the package is

| Item | Value |
| --- | --- |
| Archive | `Video Convert Tool.zip` (61.3 MB, 17 entries) |
| Installer | `Setup.exe` (2008 bootstrapper) + `MSI.CAB` (402 files) + 13 per-language `Msi*.msi` |
| MSI ProductName | **Media Player Utilities 4.46** (ProductVersion 4.46, UpgradeCode `{329E5A1C-209C-434B-9501-D691DA8DAAE2}`) |
| MSI title | "MP3 Player" |
| Vendor (registry key) | `HKLM\Software\Actions` → Actions Semiconductor, the maker of the ATJ chips in cheap MP3/MP4 players |
| Build dates | `amvtransform.exe` 2012-10-26, `transdll.dll` 2012-10-26, `FFMpegDll.dll` 2012-10-30, `AVI_EncDLL.dll` 2011-08-02 |

`Setup.ini` only chains redistributables (VC6/VC2005 runtimes, Windows Media
Format 11, DirectX 9c). The MSI file table was decoded with a small table
reader (string pool + column-major tables) to map the scrambled CAB member
names (`_0074DCC0…`) back to install paths. The package installs five tools:

| Start-menu shortcut | Program | Purpose |
| --- | --- | --- |
| **AMV&AVI Video Converter** | `AMVConverter\amvtransform.exe` | **The video converter. This is the tool in question.** |
| AMV Player | `AMVPlayer\amvplayer.exe` | Desktop player for `.amv` files |
| Media Manager | `MediaManager\MediaManager.exe` | Syncs music to the player |
| Media Player Disk Manager | `RDiskUtility\RdiskUtility.exe` | Formats/partitions the player |
| Media Player Upgrade Tool | `RDiskUpdate\RdiskUpgrade.exe` | Firmware flasher (installs a WinUSB/ADFU driver) |

It also registers a shell context-menu handler (`*\shellex\ContextMenuHandlers\VIDEOTRANS`)
and a DirectShow source filter for `.amv`.

## 2. Converter architecture (Observed)

```
amvtransform.exe  (MFC UI, reads AmvTransform.ini + settings.ini)
   └─ transdll.dll   (conversion engine; exports CreateAmvInterface)
        ├─ decode:  DirectShow graph + "Wave Grabber" sample grabber,
        │           WMVCore.dll for WMV/ASF, MediaInfo.dll for probing,
        │           FFMpegDll.dll → FFmpeg 1.0-era libs (Lavc54.23.100 / Lavf54.6.100)
        ├─ AVI out: AVI_EncDLL.dll  (AviEncoderInit / AviEncoder / AviHeaderEncoder /
        │                            AviEncoderClose / audio_resample)
        └─ AMV out: AMV_EncDLL.dll
```

REA evidence for the export tables: `ev_4e194ac1…` (AVI_EncDLL), `ev_7b163b8a…`
(FFMpegDll), `ev_66b7ef79…` (transdll); import table of AVI_EncDLL:
`ev_ac96a5d5…` (KERNEL32 only, so the AVI encoder is fully self-contained:
its own JPEG encoder, its own IMA ADPCM encoder and its own AVI writer).

Input types accepted by the UI file filter: avi, mpeg/mpg/dat, wmv/wma/asf,
rm/ram/rmvb, mov/qt, vob, swf, amv, 3gp, f4v, flv, m4v, m4a, mkv, **mp4**, ts.

## 3. The MP4 → AVI conversion, exact settings

### 3.1 Resolution choices (Observed, `AmvTransform.ini` `[Option]`)

```
AVISIZE=96X64(14;22;22),128X96(14;22;22),128X128(14;22;22),160X120(14;22;22),
        160X128(14;22;22),176X128(14;22;22),176X144(14;22;22),192X160(14;22;22),
        208X120(14;22;22),208X160(14;22;22),208X176(14;22;22),320X240(14;22;22)
```

The comment above the key (GBK Chinese) reads "format: resolution(frame rate
low; medium; high)". So every AVI size offers 14 / 22 / 22 fps.
`settings.ini` defaults: `VWH=3` (fourth entry → **160×120**), `VFBS=high`
(→ 22), `Imagequality=0`, `Imagezoom=1`, `Overwrite=TRUE`.

### 3.2 Scaling (Observed, `FFMpegDll.dll`)

`sws_getContext(srcW, srcH, srcFmt, dstW, dstH, dstFmt, flags=4, …)`.
Flags is the global at `0x100170d8` = `4` = **`SWS_BICUBIC`**, never written.
Destination is exactly the chosen width × height, so the picture is
**stretched** (aspect ratio not preserved). Destination pixel format is
`3` = **BGR24** (or YUV420P when a caller flag is 1). The DirectShow decode
path was not traced; the same stretch behaviour is *Inferred* there.

### 3.3 Frame rate is derived from the audio block (Observed, `AviEncoderInit`)

```c
samples_per_block = (fps >= 18) ? 1017 : 1529;           // IMA ADPCM, mono
block_align       = (samples_per_block + 7) / 2;          // 512 or 768 bytes
us_per_frame      = samples_per_block * 1000000 / 22050;  // integer division
```

One ADPCM block is written per video frame, so the real frame rate is
`22050 / samples_per_block`:

| UI setting | samples/block | block_align | µs/frame | Real fps |
| --- | --- | --- | --- | --- |
| 14 ("low") | 1529 | 768 | 69342 | **14.4212** |
| 22 ("medium", "high") | 1017 | 512 | 46122 | **21.6816** |

### 3.4 Video stream (Observed, `AVI_EncDLL.dll` data at file offset `0xD178`)

Every frame is a complete baseline JPEG stored as chunk `00dc`:

- `SOI`, then `APP0` with identifier **`AVI1`** (16-byte segment, all zeros after the id)
- `DQT` table 0 and 1: **IJG quality-75** tables (standard Annex K tables × 0.5, rounded).
  No code writes to these bytes, so `Imagequality` does not affect AVI output
  (the UI string says "AMV image quality").
- `SOF0`: 8-bit, 3 components, Y sampling `2×2`, Cb/Cr `1×1` → **YCbCr 4:2:0**
- Four **standard Annex K Huffman tables** (DC/AC luma/chroma), repeated in every frame
- `SOS`, entropy-coded data, `EOI`
- RGB → YCbCr uses fixed-point lookup tables (exact coefficients *Unknown*; standard JFIF full-range BT.601 is *Inferred*)
- Chunk payload is zero-padded so the next chunk starts on a **4-byte boundary**; the chunk size field excludes this padding.

### 3.5 Audio stream (Observed)

Chunk `01wb`, written after each video chunk:

| Field | Value |
| --- | --- |
| wFormatTag | `0x0011` **IMA ADPCM** |
| Channels | **1** (mono) |
| Sample rate | **22050 Hz** (hard-coded by `transdll.dll`: `mov [esi+0x18], 0x5622`) |
| Bits/sample | 4 |
| nBlockAlign | 768 or 512 (see 3.3) |
| wSamplesPerBlock | 1529 or 1017 |
| nAvgBytesPerSec | `block_align * 22050 / samples_per_block` → 11075 or 11100 |

Input to the encoder is 16-bit PCM; `audio_resample` converts the source to
22050 Hz first.

### 3.6 AVI container (Observed, template at `0xD030`, patched by `AviEncoderInit`/`AviEncoderClose`)

| Field | Value |
| --- | --- |
| `avih.dwMicroSecPerFrame` | 69342 / 46122 |
| `avih.dwMaxBytesPerSec` | `0x40000` (fixed) |
| `avih.dwFlags` | `0x00010010` (`AVIF_HASINDEX | AVIF_WASCAPTUREFILE`) |
| `avih.dwTotalFrames` | frame count (patched on close) |
| `avih.dwStreams` | 2 |
| `avih.dwSuggestedBufferSize` | `0x40000` |
| `avih.dwReserved[0]` | `22579200 / samples_per_block` = fps × 1024 (14767 / 22201) |
| `avih.dwReserved[1]` | 1 |
| `avih.dwReserved[3]` | duration packed as `0x00HHMMSS` (*Inferred*: computed from a seconds value by /3600, /60, %60) |
| `strh(vids)` | handler `MJPG`, scale = µs/frame, rate = 1 000 000, **length = 2** (template value, never updated) |
| `strf(vids)` | `BITMAPINFOHEADER` 40 bytes, 24 bpp, compression `MJPG`, sizeImage = w×h×3 |
| `strh(auds)` | scale = block_align, rate = avgBytesPerSec, sampleSize = block_align, **length = ((spb >> 2) + 4) × frames**, suggestedBufferSize = (spb >> 2) + 4 (386 / 258) |
| `strf(auds)` | 20-byte `WAVEFORMATEX` + `cbSize=2` + `wSamplesPerBlock` |
| `movi` | `00dc`, `01wb`, `00dc`, `01wb`, … one pair per frame |
| `idx1` | 2 entries per frame, flags `0x10` (keyframe) for both, offsets relative to `movi` |

No OpenDML (`odml`/`indx`) structures, so files are limited to 1 GB in practice.

### 3.7 Other behaviour (Observed from strings/config; not traced in code)

- Optional "Cut source video fragment" (start/end seconds), "Replace audio track", rotation
  (`Normal; UpDown; LeftRight; UpDown-LeftRight; Deasil 90; Anticlockwise 90`).
- "Copy video file to" the player and "Delete after copy".
- AMV mode uses `VIDEOSIZE` with 16 / 25 / 30 fps (20 at 320×240) and `MTVFILEEXTEND=amv`.
- Index entries are first written to a temp file `idx1.dat` in the program
  directory (falls back to `C:\` … `L:\`), then appended.

## 4. What this means for the new app

To be a drop-in replacement, the **"Classic player AVI"** preset in the new app
reproduces sections 3.1 to 3.6 exactly: the 12 sizes, 14.42 / 21.68 fps, bicubic
stretch, baseline 4:2:0 JPEG with IJG-75 tables, standard Huffman tables and
`AVI1` APP0, IMA ADPCM mono 22050 Hz with one block per frame, the same header
values including the reserved fields and the two `dwLength` quirks, 4-byte chunk
alignment and the `idx1` index. Old player firmware is the most likely reader
of the non-standard fields, so they are kept bit-for-bit.

Shortcomings of the original that the new app fixes: Windows-only, 32-bit
MFC UI, no queue progress/ETA, aspect ratio always distorted, 1 GB AVI limit,
no MP3 extraction, no modern formats, temp files written to `C:\`.

## 5. File identities

| File | SHA-256 |
| --- | --- |
| `Setup.exe` | `a7b971d9bfd17f0e924be9c4930e8fe1fa9cbd6f4d26b941aaeeb9ce6abb18d1` |
| `MSI.CAB` | `c47b6ed72aa1ac907fa2f1fe5a2d05c447db5f4cf0b421799ec9712f3b3ab848` |
| `Msien.msi` | `7099e2fb542ec0fc354ea1b1e66095ca2db0829ea02e393a54761e114f05da16` |
| `amvtransform.exe` | `4909b36d59efca4963a9835879cddb47c456ec06ca96f717ae0bd14436ab3cba` |
| `transdll.dll` | `34fed19316b9a812503d122bcdcec9bd393c7cba323ade64e1d09cc530ee3d00` |
| `AVI_EncDLL.dll` | `cc85a7741b462b1da67cc329d307df23c5cc994c05c62cbb16cbc54be9e707f1` |
| `FFMpegDll.dll` | `fd7bfa954dc5407bf487b5a701d05e5bdb90d789db650037b60341c7aef99497` |
| `AmvTransform.ini` | `b9d88e0bdcf2460d8cbf56d7048bbc2abb54c529afc7cf6e781095e38e4a59cc` |

## 6. Method and limits

- Tools: 7-Zip (CAB/MSI streams), a purpose-written MSI table decoder, REA 6.10.1
  (PE export/import inventory), GNU objdump (x86 disassembly), manual reading.
- No decompiler engine (Ghidra/IDA) was available, so code was read as assembly.
- Nothing was executed. Runtime behaviour (for example which decode path a given
  MP4 takes) is *Inferred* from code, not observed.
- Open questions: exact RGB→YCbCr coefficients; DirectShow-path scaling; the
  meaning of `ACOE=1.0` (likely an audio gain coefficient); the value stored in
  `avih.dwReserved[3]` when a fragment is cut.
