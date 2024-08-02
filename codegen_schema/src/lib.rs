use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use miette::{Context, IntoDiagnostic};
use walkdir::WalkDir;

use crate::schema::SchemaItem;

pub mod schema;

pub fn load_from_dir(dir: impl AsRef<Path>) -> miette::Result<Vec<(PathBuf, SchemaItem)>> {
    let mut files = vec![];
    for entry in WalkDir::new(dir.as_ref()).into_iter() {
        let entry = entry.into_diagnostic()?;
        if !entry.file_type().is_file() {
            continue;
        }
        if !entry
            .path()
            .extension()
            .is_some_and(|ext| ext.to_ascii_lowercase() == "xml")
        {
            continue;
        }

        process_file(entry.path(), &mut files)
            .with_context(|| format!("Failed to process file at `{}`", entry.path().display()))?;
    }

    files.sort_by(|a, b| {
        match (&a.1, &b.1) {
            (SchemaItem::Schema { .. }, SchemaItem::Data(..)) => Ordering::Less,
            (SchemaItem::Data(..), SchemaItem::Schema { .. }) => Ordering::Greater,
            (SchemaItem::Data(a), SchemaItem::Data(b)) => a.ty.cmp(&b.ty),
            (SchemaItem::Schema { .. }, SchemaItem::Schema { .. }) => Ordering::Equal,
        }
        .then_with(|| a.0.cmp(&b.0))
    });

    Ok(files)
}

fn process_file(path: &Path, files: &mut Vec<(PathBuf, SchemaItem)>) -> miette::Result<()> {
    let data = fs_err::read_to_string(path)
        .into_diagnostic()
        .context("Failed to read the file")?;

    let data = quick_xml::de::from_str::<SchemaItem>(&data)
        .into_diagnostic()
        .context("Failed to deserialize XML")?;

    files.push((path.to_path_buf(), data));

    Ok(())
}
