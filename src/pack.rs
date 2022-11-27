use std::io::Read;

use flate2::read::ZlibDecoder;
use scroll::Pread;
use scroll_derive::Pread;
use anyhow::{Result, anyhow};

#[derive(Debug, Pread, Clone)]
struct BilibiliPackHeader {
    pack_len: u32,
    header_len: u16,
    ver: u16,
    op: u32,
    seq: u32,
}

#[derive(Debug, Pread)]
struct PackHotCount {
    count: u32,
}

type BilibiliPackCtx<'a> = (BilibiliPackHeader, &'a [u8]);

fn pack(buffer: &[u8]) -> Result<BilibiliPackCtx> {
    let data = buffer.pread_with(0, scroll::BE)?;

    let buf = &buffer[16..];

    Ok((data, buf))
}

fn write_int(buffer: &[u8], start: usize, val: u32) -> Result<Vec<u8>> {
    let val_bytes = val.to_be_bytes();

    let mut buf = buffer.to_vec();

    for (i, c) in val_bytes.iter().enumerate() {
        buf[start + i] = *c;
    }

    Ok(buf)
}

pub fn encode(s: &str, op: u8) -> Result<Vec<u8>> {
    let data = s.as_bytes();
    let packet_len = 16 + data.len();
    let header = vec![0, 0, 0, 0, 0, 16, 0, 1, 0, 0, 0, op, 0, 0, 0, 1];

    let header = write_int(&header, 0, packet_len as u32)?;

    return Ok([&header, data].concat());
}

pub fn build_pack(buf: &[u8]) -> Result<Vec<String>> {
    let ctx = pack(buf)?;
    let msgs = decode(ctx)?;

    Ok(msgs)
}

fn get_hot_count(body: &[u8]) -> Result<String> {
    let count = body.pread_with::<PackHotCount>(0, scroll::BE)?.count;

    Ok(count.to_string())
}

fn zlib_decode(body: &[u8]) -> Result<(BilibiliPackHeader, Vec<u8>)> {
    // let mut body = body.to_vec();
    let mut buf = vec![];
    let mut z = ZlibDecoder::new(body);
    z.read_to_end(&mut buf)?;

    let ctx = pack(&buf)?;
    let header = ctx.0;
    let buf = ctx.1.to_vec();

    Ok((header, buf))
}

fn decode(ctx: BilibiliPackCtx) -> Result<Vec<String>> {
    let (mut header, body) = ctx;

    let mut buf = body.to_vec();

    loop {
        (header, buf) = match header.ver {
            2 => zlib_decode(body)?,
            3 => brotli_decode(body)?,
            0 | 1 => break,
            _ => break,
        }
    }

    let msgs = match header.ver {
        0 => split_msgs(buf, header)?,
        1 => vec![format!("{{\"count\": {}}}", get_hot_count(&buf)?)],
        x => return Err(anyhow!("Unsupport proto version! {}", x)),
    };

    Ok(msgs)
}

fn split_msgs(buf: Vec<u8>, header: BilibiliPackHeader) -> Result<Vec<String>> {
    let mut buf = buf;
    let mut header = header;
    let mut msgs = vec![];
    let mut offset = 0;
    let buf_len = buf.len();

    msgs.push(std::str::from_utf8(&buf[..(header.pack_len - 16) as usize])?.to_string());
    buf = buf[(header.pack_len - 16) as usize..].to_vec();
    offset += header.pack_len - 16;

    while offset != buf_len as u32 {
        let ctx = pack(&*buf)?;

        header = ctx.0;
        buf = ctx.1.to_vec();

        msgs.push(std::str::from_utf8(&buf[..(header.pack_len - 16) as usize])?.to_string());

        buf = buf[(header.pack_len - 16) as usize..].to_vec();

        offset += header.pack_len;
    }

    Ok(msgs)
}

fn brotli_decode(body: &[u8]) -> Result<(BilibiliPackHeader, Vec<u8>)> {
    let mut reader = brotli::Decompressor::new(body, 4096);

    let mut buf = Vec::new();

    reader.read_to_end(&mut buf)?;

    let ctx = pack(&buf)?;

    let header = ctx.0;
    let buf = ctx.1.to_vec();

    Ok((header, buf))
}
