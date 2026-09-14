// A minimal uncompressed ZIP writer. Drawables are small text files, so skipping
// compression keeps the plugin free of third-party code.

export interface ZipEntry {
  path: string;
  data: Uint8Array;
}

// 1980-01-01, the earliest valid ZIP date; a zero date field reads as invalid in some tools.
const DOS_DATE = (1 << 5) | 1;

const CRC_TABLE = Array.from({ length: 256 }, (_, byte) => {
  let crc = byte;
  for (let bit = 0; bit < 8; bit++) crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
  return crc >>> 0;
});

export function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of data) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

export function createZip(entries: ZipEntry[]): Uint8Array {
  const encoder = new TextEncoder();
  const locals: Uint8Array[] = [];
  const centrals: Uint8Array[] = [];
  let offset = 0;

  for (const entry of entries) {
    const path = encoder.encode(entry.path);
    const crc = crc32(entry.data);
    const size = entry.data.length;

    const local = new Uint8Array(30 + path.length + size);
    const header = new DataView(local.buffer);
    header.setUint32(0, 0x04034b50, true);
    header.setUint16(4, 20, true); // version needed
    header.setUint16(6, 0x0800, true); // UTF-8 file names
    header.setUint16(12, DOS_DATE, true);
    header.setUint32(14, crc, true);
    header.setUint32(18, size, true);
    header.setUint32(22, size, true);
    header.setUint16(26, path.length, true);
    local.set(path, 30);
    local.set(entry.data, 30 + path.length);

    const central = new Uint8Array(46 + path.length);
    const record = new DataView(central.buffer);
    record.setUint32(0, 0x02014b50, true);
    record.setUint16(4, 20, true); // version made by
    record.setUint16(6, 20, true); // version needed
    record.setUint16(8, 0x0800, true);
    record.setUint16(14, DOS_DATE, true);
    record.setUint32(16, crc, true);
    record.setUint32(20, size, true);
    record.setUint32(24, size, true);
    record.setUint16(28, path.length, true);
    record.setUint32(42, offset, true);
    central.set(path, 46);

    locals.push(local);
    centrals.push(central);
    offset += local.length;
  }

  const centralSize = centrals.reduce((total, central) => total + central.length, 0);
  const end = new Uint8Array(22);
  const trailer = new DataView(end.buffer);
  trailer.setUint32(0, 0x06054b50, true);
  trailer.setUint16(8, entries.length, true);
  trailer.setUint16(10, entries.length, true);
  trailer.setUint32(12, centralSize, true);
  trailer.setUint32(16, offset, true);

  const zip = new Uint8Array(offset + centralSize + end.length);
  let position = 0;
  for (const part of [...locals, ...centrals, end]) {
    zip.set(part, position);
    position += part.length;
  }
  return zip;
}
