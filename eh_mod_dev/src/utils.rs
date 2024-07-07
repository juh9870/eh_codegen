use flate2::Compression;
use sha2::Digest;
use std::io::Write;

pub(crate) fn compress(data: &[u8], compression: Compression) -> Vec<u8> {
    let mut flate2_data = vec![];
    let mut writer = flate2::write::ZlibEncoder::new(&mut flate2_data, compression);
    writer.write_all(data).unwrap();
    writer.flush_finish().unwrap();
    flate2_data
}

pub(crate) fn decompress(data: &[u8]) -> Vec<u8> {
    let mut deflated = vec![];
    let mut writer = flate2::write::ZlibDecoder::new(&mut deflated);
    writer.write_all(data).unwrap();
    writer.flush().unwrap();
    drop(writer);
    deflated
}

pub(crate) fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
