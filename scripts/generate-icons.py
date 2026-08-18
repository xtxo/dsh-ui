#!/usr/bin/env python3
from pathlib import Path
import io
import struct

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SVG = ROOT / "assets" / "icon.svg"


def png_bytes(image: Image.Image, size: int) -> bytes:
    out = io.BytesIO()
    image.resize((size, size), Image.Resampling.LANCZOS).save(out, format="PNG", optimize=True)
    return out.getvalue()


def ico_bytes(image: Image.Image, sizes) -> bytes:
    out = io.BytesIO()
    image.save(out, format="ICO", sizes=sizes)
    return out.getvalue()


def icns_bytes(pngs: dict[int, bytes]) -> bytes:
    # PNG-compressed ICNS entries: 32, 128, 256 and 512 px.
    types = {32: b"icp5", 128: b"ic07", 256: b"ic08", 512: b"ic09"}
    payload = b""
    for size in (32, 128, 256, 512):
        data = pngs[size]
        payload += types[size] + struct.pack(">I", len(data) + 8) + data
    return b"icns" + struct.pack(">I", len(payload) + 8) + payload


def write(path: str, data: bytes) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    print(f"wrote {target.relative_to(ROOT)}")


def main() -> None:
    master_png = cairosvg.svg2png(url=str(SVG), output_width=1024, output_height=1024)
    master = Image.open(io.BytesIO(master_png)).convert("RGBA")

    pngs = {size: png_bytes(master, size) for size in (32, 128, 256, 512)}
    ico_multi = ico_bytes(master, [(32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    ico_32 = ico_bytes(master.resize((32, 32), Image.Resampling.LANCZOS), [(32, 32)])
    icns = icns_bytes(pngs)

    # README / website assets
    write("assets/icon.png", pngs[512])
    write("assets/icon_512.png", pngs[512])
    write("assets/icon_256.png", pngs[256])
    write("assets/icon_128.png", pngs[128])
    write("assets/icon_32.png", pngs[32])
    write("assets/icon.ico", ico_multi)
    write("assets/icon_256.ico", ico_multi)
    write("assets/icon_32.ico", ico_32)

    # Tauri canonical icon set
    write("src-tauri/icons/icon.png", pngs[512])
    write("src-tauri/icons/32x32.png", pngs[32])
    write("src-tauri/icons/128x128.png", pngs[128])
    write("src-tauri/icons/128x128@2x.png", pngs[256])
    write("src-tauri/icons/icon.ico", ico_multi)
    write("src-tauri/icons/icon.icns", icns)
    write("src-tauri/icons/deepseek.icns", icns)

    # Pake / Windows / tray icon set
    write("src-tauri/png/deepseek_harness_512.png", pngs[512])
    write("src-tauri/png/deepseek_harness_256.ico", ico_multi)
    write("src-tauri/png/deepseek_harness_32.ico", ico_32)
    write("src-tauri/png/icon_512.png", pngs[512])
    write("src-tauri/png/icon_256.ico", ico_multi)
    write("src-tauri/png/icon_32.ico", ico_32)


if __name__ == "__main__":
    main()
