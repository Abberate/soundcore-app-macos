#!/usr/bin/env python3
"""Writes the Finder layout (.DS_Store) of the mounted installer volume: window size,
icon size, background picture and icon positions. Standard library only.

    dmg_layout.py <mount point> <app name>.app <read-me file name>

The background must already be at <mount point>/.background/background.tiff.
Icon centres match crates/soundcore-app/src/ui/dmg_bg.rs (window content 760 x 570 points).
"""

import base64
import os
import plistlib
import struct
import subprocess
import sys

WINDOW = (200, 120, 760, 570)  # x, y, width, height
ICON_SIZE = 112
BG_RGB = (0xF2 / 255, 0xF4 / 255, 0xF7 / 255)  # same light grey as the picture
HFS_EPOCH = 2082844800  # seconds between 1904-01-01 and 1970-01-01


# --- Legacy alias record (version 2), as Finder writes it ---------------------------------

def _tag(tag, data):
    out = struct.pack(">hh", tag, len(data)) + data
    return out + (b"\0" if len(data) & 1 else b"")


def _utf16_tag(tag, text):
    u = text.encode("utf-16-be")
    return struct.pack(">hhh", tag, len(u) + 2, len(u) // 2) + u


def alias_for(path, mount):
    st, parent = os.stat(path), os.path.dirname(path)
    vol_name = os.path.basename(mount)
    vol_date = int(os.stat(mount).st_birthtime + HFS_EPOCH)
    cr_date = int(st.st_birthtime + HFS_EPOCH)
    rel = os.path.relpath(path, mount)  # ".background/background.tiff"
    parts = rel.split("/")

    body = struct.pack(
        ">h28pI2shI64pII4s4shhI2s10s",
        0,  # kind: file
        vol_name.replace(":", "/").encode(),
        vol_date,
        b"H+",
        0,  # fixed disk
        os.stat(parent).st_ino,  # parent CNID
        parts[-1].encode(),
        st.st_ino,  # CNID
        cr_date,
        b"\0\0\0\0",
        b"\0\0\0\0",
        -1,
        -1,
        0,
        b"\0\0",
        b"\0" * 10,
    )
    body += _tag(0, os.path.basename(parent).encode())
    body += struct.pack(">hhQhhQ", 16, 8, vol_date * 65536, 17, 8, cr_date * 65536)
    cnids = [os.stat(os.path.join(mount, *parts[: i + 1])).st_ino for i in range(len(parts) - 1)]
    body += _tag(1, struct.pack(">%dI" % len(cnids), *cnids))
    body += _tag(2, (vol_name + ":" + ":\0".join(parts)).encode())
    body += _utf16_tag(14, parts[-1])
    body += _utf16_tag(15, vol_name)
    body += _tag(18, ("/" + rel).encode())
    body += _tag(19, mount.encode())
    body += struct.pack(">hh", -1, 0)
    head = struct.pack(">4shh", b"\0\0\0\0", 8 + len(body), 2)
    return head + body


def bookmark_for(path):
    """Modern bookmark data through Foundation (JavaScript for Automation, no app control)."""
    js = (
        "ObjC.import('Foundation');"
        "function run(a){var u=$.NSURL.fileURLWithPath(a[0]);"
        "var d=u.bookmarkDataWithOptionsIncludingResourceValuesForKeysRelativeToURLError(0,$(),$(),$());"
        "return d.base64EncodedStringWithOptions(0).js;}"
    )
    try:
        out = subprocess.run(
            ["osascript", "-l", "JavaScript", "-e", js, path],
            check=True, capture_output=True, text=True, timeout=20,
        ).stdout.strip()
        return base64.b64decode(out) if out else None
    except Exception as e:  # the alias alone is enough for Finder
        print("  (no bookmark: %s)" % e, file=sys.stderr)
        return None


# --- .DS_Store (buddy allocator + single-leaf B-tree) -------------------------------------

def record(name, code, kind, value):
    n = name.encode("utf-16-be")
    out = struct.pack(">I", len(n) // 2) + n + code.encode() + kind.encode()
    if kind == "blob":
        return out + struct.pack(">I", len(value)) + value
    if kind == "long":
        return out + struct.pack(">I", value)
    if kind == "type":
        return out + value.encode()
    raise ValueError(kind)


def ds_store(records):
    records.sort(key=lambda r: (r[0].lower(), r[1]))
    node = struct.pack(">II", 0, len(records)) + b"".join(record(*r) for r in records)
    if len(node) > 0x1000:
        raise SystemExit("layout too large for one B-tree page")

    space = bytearray(0x2000)  # allocator address space (file offset 4 onwards)
    # Blocks: 0 = bookkeeping (0x800, 2 KiB), 1 = DSDB header (0x20, 32 B), 2 = leaf (0x1000, 4 KiB).
    addresses = [0x800 | 11, 0x20 | 5, 0x1000 | 12]
    space[0:32] = b"Bud1" + struct.pack(">III", 0x800, 0x800, 0x800) + bytes.fromhex(
        "0000100c000000870000200b00000000"
    )
    space[0x20:0x34] = struct.pack(">IIIII", 2, 0, len(records), 1, 0x1000)
    space[0x1000 : 0x1000 + len(node)] = node

    book = struct.pack(">II", len(addresses), 0)
    book += struct.pack(">256I", *(addresses + [0] * (256 - len(addresses))))
    book += struct.pack(">I", 1) + bytes([4]) + b"DSDB" + struct.pack(">I", 1)
    free = {6: [0x40], 7: [0x80], 8: [0x100], 9: [0x200], 10: [0x400]}
    free.update({i: [1 << i] for i in range(13, 31)})
    for i in range(32):
        offsets = free.get(i, [])
        book += struct.pack(">I", len(offsets)) + b"".join(struct.pack(">I", o) for o in offsets)
    space[0x800 : 0x800 + len(book)] = book
    return struct.pack(">I", 1) + bytes(space)


def main():
    mount, app, readme = sys.argv[1:4]
    bg = os.path.join(mount, ".background", "background.tiff")
    if not os.path.exists(bg):
        raise SystemExit("missing " + bg)

    x, y, w, h = WINDOW
    bwsp = {
        "ContainerShowSidebar": False,
        "PreviewPaneVisibility": False,
        "ShowPathbar": False,
        "ShowSidebar": False,
        "ShowStatusBar": False,
        "ShowTabView": False,
        "ShowToolbar": False,
        "SidebarWidth": 0,
        "WindowBounds": "{{%d, %d}, {%d, %d}}" % (x, y, w, h),
    }
    icvp = {
        "arrangeBy": "none",
        "backgroundColorRed": BG_RGB[0],
        "backgroundColorGreen": BG_RGB[1],
        "backgroundColorBlue": BG_RGB[2],
        "backgroundImageAlias": alias_for(bg, mount),
        "backgroundType": 2,
        "gridOffsetX": 0.0,
        "gridOffsetY": 0.0,
        "gridSpacing": 100.0,
        "iconSize": float(ICON_SIZE),
        "labelOnBottom": True,
        "showIconPreview": True,
        "showItemInfo": False,
        "textSize": 12.0,
        "viewOptionsVersion": 1,
    }
    plist = lambda d: plistlib.dumps(d, fmt=plistlib.FMT_BINARY)  # noqa: E731
    iloc = lambda px, py: struct.pack(">IIII", px, py, 0xFFFFFFFF, 0xFFFF0000)  # noqa: E731

    records = [
        (".", "bwsp", "blob", plist(bwsp)),
        (".", "icvp", "blob", plist(icvp)),
        (".", "icvl", "type", "icnv"),
        (".", "vSrn", "long", 1),
        (app, "Iloc", "blob", iloc(210, 200)),
        ("Applications", "Iloc", "blob", iloc(550, 200)),
        (readme, "Iloc", "blob", iloc(650, 392)),
    ]
    bookmark = bookmark_for(bg)
    if bookmark:
        records.append((".", "pBBk", "blob", bookmark))

    with open(os.path.join(mount, ".DS_Store"), "wb") as f:
        f.write(ds_store(records))
    print("  layout written (%d records%s)" % (len(records), ", bookmark" if bookmark else ""))


if __name__ == "__main__":
    main()
