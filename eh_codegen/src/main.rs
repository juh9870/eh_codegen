use crate::codegen::CodegenState;
use crate::schema::SchemaItem;
use clap::Parser;
use miette::{Context, Diagnostic, IntoDiagnostic, Report};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use walkdir::WalkDir;

mod codegen;
mod schema;

/// Generates typescript definitions for items from Event Horizon schema
#[derive(Debug, Parser)]
struct Args {
    /// Path to the schema directory
    #[arg(short, long, env = "CODEGEN_SCHEMA_INPUT")]
    schema: PathBuf,
    /// Path to the output directory
    #[arg(short, long, env = "CODEGEN_OUTPUT")]
    output: PathBuf,
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(
    code(oops::my::bad),
    url(docsrs),
    help("try doing it better next time?")
)]
#[error("Code generation failed")]
struct MainErr(#[diagnostic_source] Report);

impl From<Report> for MainErr {
    fn from(value: Report) -> Self {
        Self(value)
    }
}

fn main() -> miette::Result<()> {
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::Layer::default())
        .with(EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    m_try(|| {
        let Args { schema, output } = Args::parse();

        let mut files = vec![];
        for entry in WalkDir::new(&schema).into_iter() {
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

            process_file(entry.path(), &mut files).with_context(|| {
                format!("Failed to process file at `{}`", entry.path().display())
            })?;
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

        let mut state = CodegenState::default();

        let mut code_builder = "\
            #![allow(clippy::large_enum_variant)]\n\
            #![allow(dead_code)]\n\
            #![allow(unreachable_patterns)]\n\n"
            .to_string();

        for (path, item) in files {
            let code = state
                .codegen(item)
                .and_then(CodegenState::format_tokens)
                .with_context(|| {
                    format!("Failed to generate code for file at `{}`", path.display())
                })?;
            code_builder += &format!("\n// {}\n", path.strip_prefix(&schema).unwrap().display());
            code_builder += &code.unwrap_or_default();
        }

        let db_item_code = state
            .codegen_core_db_item()
            .and_then(|c| CodegenState::format_tokens(Some(c)))
            .with_context(|| "Failed to generate core DB item type".to_string())?;
        code_builder += "\n// Core Database Item\n";
        code_builder += &db_item_code.unwrap_or_default();

        let extra_funcs_code = state
            .codegen_extra_functions()
            .and_then(|c| CodegenState::format_tokens(Some(c)))
            .with_context(|| "Failed to generate extra functions".to_string())?;
        code_builder += "\n// Helper functions\n";
        code_builder += &extra_funcs_code.unwrap_or_default();

        fs_err::write(output, code_builder)
            .into_diagnostic()
            .context("Failed to write a file")?;

        Ok(())
    })
    .context("Code generator failed")
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

/// Helper for wrapping a code block to help with contextualizing errors
/// Better editor support but slightly worse ergonomic than a macro
#[inline(always)]
pub(crate) fn m_try<T>(func: impl FnOnce() -> miette::Result<T>) -> miette::Result<T> {
    func()
}
