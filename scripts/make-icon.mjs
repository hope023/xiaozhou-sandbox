import zlib from "node:zlib";
import fs from "node:fs";

const S = 1024;
const SS = 4;

function roundedRect(px, py, x, y, w, h, r) {
  const cx = Math.max(x + r, Math.min(px, x + w - r));
  const cy = Math.max(y + r, Math.min(py, y + h - r));
  return Math.hypot(px - cx, py - cy) <= r;
}

function insideLockBody(px, py) {
  return roundedRect(px, py, 316, 470, 392, 300, 56);
}

function insideShackle(px, py) {
  const cx = 512;
  const cy = 470;
  const outer = 168;
  const inner = 108;
  const d = Math.hypot(px - cx, py - cy);
  return py <= cy && d <= outer && d >= inner;
}

const raw = Buffer.alloc(S * S * 4);
for (let y = 0; y < S; y++) {
  for (let x = 0; x < S; x++) {
    let bg = 0;
    let fg = 0;
    for (let sy = 0; sy < SS; sy++) {
      for (let sx = 0; sx < SS; sx++) {
        const px = x + (sx + 0.5) / SS;
        const py = y + (sy + 0.5) / SS;
        if (roundedRect(px, py, 0, 0, S, S, 210)) bg++;
        if (insideLockBody(px, py) || insideShackle(px, py)) fg++;
      }
    }
    const total = SS * SS;
    const aBg = bg / total;
    const aFg = fg / total;
    const i = (y * S + x) * 4;
    // background #0F6E56, foreground #FFFFFF
    const r = 15 * (1 - aFg) + 255 * aFg;
    const g = 110 * (1 - aFg) + 255 * aFg;
    const b = 86 * (1 - aFg) + 255 * aFg;
    raw[i] = Math.round(r);
    raw[i + 1] = Math.round(g);
    raw[i + 2] = Math.round(b);
    raw[i + 3] = Math.round(255 * aBg);
  }
}

const stride = S * 4 + 1;
const scan = Buffer.alloc(stride * S);
for (let y = 0; y < S; y++) {
  scan[y * stride] = 0;
  raw.copy(scan, y * stride + 1, y * S * 4, (y + 1) * S * 4);
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body) >>> 0);
  return Buffer.concat([len, body, crc]);
}

let table = null;
function crc32(buf) {
  if (!table) {
    table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return c ^ -1;
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8;
ihdr[9] = 6;
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(scan, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

fs.mkdirSync("src-tauri", { recursive: true });
fs.writeFileSync("src-tauri/icon-source.png", png);
console.log("icon-source.png", png.length, "bytes");
