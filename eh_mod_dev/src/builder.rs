use eh_schema::schema::DatabaseSettings;
use flate2::Compression;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct ModBuilderInfo {
    pub output_path: PathBuf,
    pub name: String,
    pub guid: String,
    pub version_major: i32,
    pub version_minor: i32,
}

impl ModBuilderInfo {
    pub fn from_settings(output_path: PathBuf, data: &DatabaseSettings) -> ModBuilderInfo {
        ModBuilderInfo {
            output_path,
            name: data.mod_name.clone(),
            guid: data.mod_id.clone(),
            version_major: data.database_version,
            version_minor: data.database_version_minor,
        }
    }
}

#[derive(Debug)]
pub struct ModBuilderData(Option<BTreeMap<PathBuf, Vec<u8>>>);

impl Default for ModBuilderData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModBuilderData {
    pub fn dummy() -> Self {
        Self(None)
    }

    pub fn new() -> Self {
        Self(Some(BTreeMap::new()))
    }

    pub fn add_file(&mut self, path: PathBuf, data: &[u8]) {
        self.0.as_mut().map(|m| m.insert(path, data.to_vec()));
    }

    pub fn build(self, info: &ModBuilderInfo) -> std::io::Result<()> {
        let Some(data) = self.0 else {
            return Ok(());
        };
        let mut w = std::fs::File::create(&info.output_path)?;
        build(&mut w, data, info)
    }
}

fn build(
    stream: &mut impl Write,
    data: BTreeMap<PathBuf, Vec<u8>>,
    info: &ModBuilderInfo,
) -> std::io::Result<()> {
    let mut raw_data: Vec<u8> = Default::default();
    serialize_data(&mut raw_data, data, info)?;

    encrypt(stream, raw_data)
}

fn compress(data: &[u8], compression: Compression) -> Vec<u8> {
    let mut flate2_data = vec![];
    let mut writer = flate2::write::ZlibEncoder::new(&mut flate2_data, compression);
    writer.write_all(data).unwrap();
    writer.flush_finish().unwrap();
    flate2_data
}

fn encrypt(stream: &mut impl Write, raw_data: Vec<u8>) -> std::io::Result<()> {
    serialize_header(stream)?;

    let mut data = compress(&raw_data, Compression::best());

    let size = data.len() as u32;

    let mut w = 0x12345678 ^ size;
    let mut z = 0x87654321 ^ size;
    let mut checksum: u8 = 0;

    for item in data.iter_mut() {
        checksum = checksum.wrapping_add(*item);

        *item ^= random(&mut w, &mut z) as u8
    }

    stream.write_all(&data)?;

    stream.write_all(&[checksum ^ random(&mut w, &mut z) as u8])?;

    Ok(())
}

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(u8)]
enum FileType {
    None = 0,
    Data = 1,
    Image = 2,
    Localization = 3,
    WaveAudio = 4,
    OggAudio = 5,
}

fn serialize_header(w: &mut impl Write) -> std::io::Result<()> {
    serialize_uint(w, 0xDA7ABA5E)
}

fn serialize_data(
    w: &mut impl Write,
    data: BTreeMap<PathBuf, Vec<u8>>,
    info: &ModBuilderInfo,
) -> std::io::Result<()> {
    const DB_VERSION: i32 = 1;

    serialize_int(w, DB_VERSION)?;
    serialize_string(w, &info.name)?;
    serialize_string(w, &info.guid)?;
    serialize_int(w, info.version_major)?;
    serialize_int(w, info.version_minor)?;

    for (path, bytes) in data {
        let Some(ext) = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
        else {
            warn!(path=%path.display(), "Skipping serializing file with no extension");
            continue;
        };

        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
            warn!(path=%path.display(), "Skipping serializing file with no file name");
            continue;
        };

        let Some(file_name_no_ext) = path.file_stem().and_then(|s| s.to_str()) else {
            warn!(path=%path.display(), "Skipping serializing file with no file name");
            continue;
        };

        match ext.as_str() {
            "json" => {
                serialize_type(w, FileType::Data)?;
                serialize_bytes(w, &bytes)?;
            }
            "png" | "jpg" | "jpeg" => {
                serialize_type(w, FileType::Image)?;
                serialize_string(w, file_name)?;
                serialize_bytes(w, &bytes)?;
            }
            "wav" => {
                serialize_type(w, FileType::WaveAudio)?;
                serialize_string(w, file_name_no_ext)?;
                serialize_bytes(w, &bytes)?;
            }
            "ogg" => {
                serialize_type(w, FileType::OggAudio)?;
                serialize_string(w, file_name_no_ext)?;
                serialize_bytes(w, &bytes)?;
            }
            "xml" => {
                serialize_type(w, FileType::Localization)?;
                serialize_string(w, file_name_no_ext)?;
                serialize_bytes(w, &bytes)?;
            }
            _ => {
                warn!(path=%path.display(), "Skipping serializing unknown file type")
            }
        }
    }

    serialize_type(w, FileType::None)?;

    Ok(())
}

fn serialize_type(w: &mut impl Write, data: FileType) -> std::io::Result<()> {
    w.write_all(&[data as u8])
}

fn serialize_int(w: &mut impl Write, data: i32) -> std::io::Result<()> {
    let bytes = data.to_le_bytes();
    w.write_all(&bytes)
}

fn serialize_uint(w: &mut impl Write, data: u32) -> std::io::Result<()> {
    let bytes = data.to_le_bytes();
    w.write_all(&bytes)
}

fn serialize_string(w: &mut impl Write, data: &str) -> std::io::Result<()> {
    serialize_bytes(w, data.as_bytes())
}

fn serialize_bytes(w: &mut impl Write, data: &[u8]) -> std::io::Result<()> {
    if data.is_empty() {
        return serialize_int(w, 0);
    }

    serialize_int(w, data.len() as i32)?;

    w.write_all(data)
}

fn random(w: &mut u32, z: &mut u32) -> u32 {
    *z = (36969u32.wrapping_mul((*z) & (u16::MAX as u32))) + (*z >> 16);
    *w = (18000u32.wrapping_mul((*w) & (u16::MAX as u32))) + (*w >> 16);
    (*z << 16).wrapping_add(*w)
}

#[cfg(test)]
mod tests {
    use super::encrypt;

    #[test]
    fn encode_bytes() {
        let mut buf = vec![];

        encrypt(
            &mut buf,
            vec![
                94, 186, 122, 218, 12, 36, 119, 53, 67, 251, 27, 41, 148, 224, 164, 255, 246,
            ],
        )
        .unwrap();

        assert_eq!(
            buf,
            vec![
                94, 186, 122, 218, 90, 65, 220, 255, 127, 204, 103, 190, 102, 235, 108, 122, 196,
                9, 55, 201, 205, 234
            ]
        );

        println!("Len: {}", buf.len());
        print!(
            "{}",
            buf.iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
