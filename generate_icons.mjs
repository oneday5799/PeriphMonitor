import sharp from "sharp";
import { mkdir, writeFile } from "fs/promises";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = join(__dirname, "src-tauri", "icons");

await mkdir(OUTPUT_DIR, { recursive: true });

// Bell SVG path data
const bellPath = 'M144 384a368 368 0 1 1 736 0v256a368 368 0 0 1-736 0V384zM512 112A272 272 0 0 0 240 384v256a272 272 0 0 0 544 0V384A272 272 0 0 0 512 112z';
const bellDotPath = 'M512 192a48 48 0 0 1 48 48v160a48 48 0 0 1-96 0v-160A48 48 0 0 1 512 192z';

// Tray icon: 32x32, black bell on transparent background
const traySvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="32" height="32">
  <path d="${bellPath}" fill="#14150f"/>
  <path d="${bellDotPath}" fill="#14150f"/>
</svg>`;

await sharp(Buffer.from(traySvg)).resize(32, 32).png().toFile(join(OUTPUT_DIR, "tray-icon.png"));
console.log("tray-icon.png saved");

// App icons: white bell on orange gradient background
const sizes = [
  [32, "32x32.png"],
  [128, "128x128.png"],
  [256, "128x128@2x.png"],
];

for (const [px, fn] of sizes) {
  const bgSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="${px}" height="${px}" viewBox="0 0 ${px} ${px}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#FF8033"/>
      <stop offset="100%" stop-color="#FF7500"/>
    </linearGradient>
  </defs>
  <rect width="${px}" height="${px}" fill="url(#bg)" rx="${Math.round(px * 0.18)}"/>
  <g transform="translate(${px * 0.1}, ${px * 0.1}) scale(${px * 0.8 / 1024})">
    <path d="${bellPath}" fill="#ffffff"/>
    <path d="${bellDotPath}" fill="#ffffff"/>
  </g>
</svg>`;
  await sharp(Buffer.from(bgSvg)).resize(px, px).png().toFile(join(OUTPUT_DIR, fn));
  console.log(`${fn} saved`);
}

// Generate ICO from 256x256
const img256 = await sharp(join(OUTPUT_DIR, "128x128@2x.png")).resize(256, 256).toBuffer();
const icoSizes = [256, 128, 64, 48, 32, 16];
const pngBuffers = [];
for (const s of icoSizes) {
  const buf = await sharp(img256).resize(s, s).png().toBuffer();
  pngBuffers.push({ size: s, buffer: buf });
}

const icoHeader = Buffer.alloc(6);
icoHeader.writeUInt16LE(0, 0);
icoHeader.writeUInt16LE(1, 2);
icoHeader.writeUInt16LE(icoSizes.length, 4);

let dataOffset = 6 + icoSizes.length * 16;
const entries = [];
for (const { size, buffer } of pngBuffers) {
  const entry = Buffer.alloc(16);
  entry.writeUInt8(size === 256 ? 0 : size, 0);
  entry.writeUInt8(size === 256 ? 0 : size, 1);
  entry.writeUInt8(0, 2);
  entry.writeUInt8(0, 3);
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(buffer.length, 8);
  entry.writeUInt32LE(dataOffset, 12);
  entries.push(entry);
  dataOffset += buffer.length;
}

const icoData = Buffer.concat([icoHeader, ...entries, ...pngBuffers.map(p => p.buffer)]);
await writeFile(join(OUTPUT_DIR, "icon.ico"), icoData);
console.log("icon.ico saved");

console.log("All icons generated!");
