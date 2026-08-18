#!/usr/bin/env python3
from pathlib import Path
import io
import struct

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets" / "icon-master.jpg"

# Keep this generator deterministic so every platform uses the same approved whale artwork.

def master_icon() -> Image.Image:
    src = Image.open(SOURCE).convert("RGB")
    # The approved artwork is stored at 256px; upscale cleanly to the project's
    # canonical 512px icon, then restore transparent rounded corners.
    img = src.resize((512, 512), Image.Resampling.LANCZOS).convert("RGBA")
    mask = Image.new("L", (2048, 2048), 0)
    d = ImageDraw.Draw(mask)
    d.rounded_rectangle((0, 0, 2047, 2047), radius=436, fill=255)
    mask = mask.resize((512, 512), Image.Resampling.LANCZOS)
    img.putalpha(mask)
    return img


def png_bytes(image: Image.Image, size: int) -> bytes:
    out = io.BytesIO()
    resized = image.resize((size, size), Image.Resampling.LANCZOS)
    if size <= 128:
        resized = resized.filter(ImageFilter.UnsharpMask(radius=0.7, percent=55, threshold=2))
    resized.save(out, format="PNG", optimize=True)
    return out.getvalue()


def ico_bytes(image: Image.Image, sizes) -> bytes:
    out = io.BytesIO()
    image.save(out, format="ICO", sizes=sizes)
    return out.getvalue()


def icns_bytes(pngs: dict[int, bytes]) -> bytes:
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
    master = master_icon()
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
