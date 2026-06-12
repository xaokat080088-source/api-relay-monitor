// 生成一个简单的 16x16 PNG 图标（Node.js 原生，无需额外依赖）
// 运行: node scripts/gen-icon.js
const fs = require('fs')
const path = require('path')

// 最小合法 16x16 RGBA PNG（纯紫色圆形）
// 用 raw PNG chunk 手写，避免依赖 canvas / sharp
function u32be(n) {
  const b = Buffer.alloc(4)
  b.writeUInt32BE(n, 0)
  return b
}

function crc32(buf) {
  let c = 0xffffffff
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i]
    for (let j = 0; j < 8; j++) c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1)
  }
  return (c ^ 0xffffffff) >>> 0
}

function chunk(type, data) {
  const typeBuf = Buffer.from(type, 'ascii')
  const lenBuf = u32be(data.length)
  const crcBuf = u32be(crc32(Buffer.concat([typeBuf, data])))
  return Buffer.concat([lenBuf, typeBuf, data, crcBuf])
}

// deflate raw using zlib
const zlib = require('zlib')

const W = 32, H = 32
const rawRows = []
for (let y = 0; y < H; y++) {
  const row = Buffer.alloc(W * 4 + 1, 0)
  row[0] = 0 // filter type
  const cx = W / 2, cy = H / 2, r = W / 2 - 1
  for (let x = 0; x < W; x++) {
    const dx = x - cx + 0.5, dy = y - cy + 0.5
    const inside = dx * dx + dy * dy <= r * r
    const i = 1 + x * 4
    if (inside) {
      row[i] = 0x7c      // R
      row[i+1] = 0x6f    // G
      row[i+2] = 0xf7    // B
      row[i+3] = 0xff    // A
    }
  }
  rawRows.push(row)
}
const raw = Buffer.concat(rawRows)
const compressed = zlib.deflateSync(raw)

const sig = Buffer.from([137,80,78,71,13,10,26,10])
const ihdr = chunk('IHDR', Buffer.concat([
  u32be(W), u32be(H),
  Buffer.from([8, 2, 0, 0, 0]) // bit depth=8, colorType=2(RGB)... use 6 for RGBA
]))
// colorType=6 = RGBA
const ihdrData = Buffer.alloc(13)
ihdrData.writeUInt32BE(W, 0)
ihdrData.writeUInt32BE(H, 4)
ihdrData[8] = 8   // bit depth
ihdrData[9] = 6   // color type RGBA
ihdrData[10] = 0  // compression
ihdrData[11] = 0  // filter
ihdrData[12] = 0  // interlace

const png = Buffer.concat([
  sig,
  chunk('IHDR', ihdrData),
  chunk('IDAT', compressed),
  chunk('IEND', Buffer.alloc(0)),
])

const outDir = path.join(__dirname, '../assets')
fs.mkdirSync(outDir, { recursive: true })
fs.writeFileSync(path.join(outDir, 'icon.png'), png)
console.log('icon.png generated')
