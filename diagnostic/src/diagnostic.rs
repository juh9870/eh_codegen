use crate::path::DiagnosticPath;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum DiagnosticKind {
    #[error("Obsolete field usage detected")]
    ObsoleteField,
    #[error("Value {} is too small, expected at least {}", .value, .min)]
    ValueTooSmall { min: f64, value: f64 },
    #[error("Value {} is too large, expected at most {}", .value, .max)]
    ValueTooLarge { max: f64, value: f64 },
    #[error("Expected a square layout, but got a layout with length {}", .length)]
    LayoutNotSquare { length: usize },
}

impl DiagnosticKind {
    pub fn too_small(min: impl Into<f64>, value: impl Into<f64>) -> Self {
        DiagnosticKind::ValueTooSmall {
            min: min.into(),
            value: value.into(),
        }
    }

    pub fn too_large(max: impl Into<f64>, value: impl Into<f64>) -> Self {
        DiagnosticKind::ValueTooLarge {
            max: max.into(),
            value: value.into(),
        }
    }

    pub fn obsolete_field() -> Self {
        DiagnosticKind::ObsoleteField
    }

    pub fn layout_not_square(length: impl Into<usize>) -> Self {
        DiagnosticKind::LayoutNotSquare {
            length: length.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        match self {
            DiagnosticKind::ObsoleteField => false,
            DiagnosticKind::ValueTooSmall { .. } => false,
            DiagnosticKind::ValueTooLarge { .. } => false,
            DiagnosticKind::LayoutNotSquare { .. } => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: DiagnosticPath,
    pub kind: DiagnosticKind,
}
