//! Minimal .zip reading and writing (stored + deflate), used to export the
//! whole notes folder and to import one from the Files app on iPhone/iPad.

fn crc(data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    h.finalize()
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Build a zip from (name inside the zip, bytes).
pub fn write(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let compressed = miniz_oxide::deflate::compress_to_vec(data, 6);
        let (method, body) = if compressed.len() < data.len() { (8u16, compressed) } else { (0u16, data.clone()) };
        let c = crc(data);
        let offset = out.len() as u32;
        let nb = name.as_bytes();
        // local header
        u32le(&mut out, 0x0403_4b50);
        u16le(&mut out, 20);
        u16le(&mut out, 0x0800); // UTF-8 names
        u16le(&mut out, method);
        u16le(&mut out, 0);
        u16le(&mut out, 0x21);
        u32le(&mut out, c);
        u32le(&mut out, body.len() as u32);
        u32le(&mut out, data.len() as u32);
        u16le(&mut out, nb.len() as u16);
        u16le(&mut out, 0);
        out.extend_from_slice(nb);
        out.extend_from_slice(&body);
        // central directory entry
        u32le(&mut central, 0x0201_4b50);
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        u16le(&mut central, 0x0800);
        u16le(&mut central, method);
        u16le(&mut central, 0);
        u16le(&mut central, 0x21);
        u32le(&mut central, c);
        u32le(&mut central, body.len() as u32);
        u32le(&mut central, data.len() as u32);
        u16le(&mut central, nb.len() as u16);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend_from_slice(nb);
    }
    let cd_offset = out.len() as u32;
    let cd_len = central.len() as u32;
    out.extend_from_slice(&central);
    u32le(&mut out, 0x0605_4b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, files.len() as u16);
    u16le(&mut out, files.len() as u16);
    u32le(&mut out, cd_len);
    u32le(&mut out, cd_offset);
    u16le(&mut out, 0);
    out
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Read every file in a zip. Folders and unsupported entries are skipped.
pub fn read(b: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    // Find the end-of-central-directory record.
    let min = b.len().saturating_sub(22 + 65535);
    let eocd = (min..b.len().saturating_sub(21)).rev().find(|&i| rd32(b, i) == Some(0x0605_4b50)).ok_or("not a zip file")?;
    let count = rd16(b, eocd + 10).ok_or("bad zip")? as usize;
    let mut p = rd32(b, eocd + 16).ok_or("bad zip")? as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        if rd32(b, p) != Some(0x0201_4b50) {
            return Err("bad zip directory".into());
        }
        let method = rd16(b, p + 10).ok_or("bad zip")?;
        let csize = rd32(b, p + 20).ok_or("bad zip")? as usize;
        let usize_ = rd32(b, p + 24).ok_or("bad zip")? as usize;
        let nlen = rd16(b, p + 28).ok_or("bad zip")? as usize;
        let elen = rd16(b, p + 30).ok_or("bad zip")? as usize;
        let clen = rd16(b, p + 32).ok_or("bad zip")? as usize;
        let local = rd32(b, p + 42).ok_or("bad zip")? as usize;
        let name = String::from_utf8_lossy(b.get(p + 46..p + 46 + nlen).ok_or("bad zip")?).to_string();
        p += 46 + nlen + elen + clen;
        if name.ends_with('/') {
            continue;
        }
        let lnl = rd16(b, local + 26).ok_or("bad zip")? as usize;
        let lel = rd16(b, local + 28).ok_or("bad zip")? as usize;
        let start = local + 30 + lnl + lel;
        let body = b.get(start..start + csize).ok_or("bad zip")?;
        let data = match method {
            0 => body.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(body, usize_.max(1) * 2 + 1024).map_err(|_| "could not unpack zip")?,
            _ => continue,
        };
        out.push((name, data));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let files = vec![
            ("My Notes/Page.md".to_string(), b"# Hello\n\nworld world world world world".to_vec()),
            ("My Notes/_assets/pic.bin".to_string(), (0..2000u32).map(|i| (i * 7 % 251) as u8).collect()),
            ("My Notes/Empty.md".to_string(), vec![]),
        ];
        let z = super::write(&files);
        let back = super::read(&z).unwrap();
        assert_eq!(back, files);
    }
}
