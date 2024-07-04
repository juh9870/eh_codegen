use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::path::{DiagnosticPath, DiagnosticPathSegment};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Display;

#[derive(Debug)]
pub struct DiagnosticContext {
    pub diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    path: DiagnosticPath,
}

impl Default for DiagnosticContext {
    fn default() -> Self {
        DiagnosticContext {
            diagnostics: Default::default(),
            path: DiagnosticPath::empty(),
        }
    }
}

impl DiagnosticContext {
    pub fn enter(&mut self, ident: impl Display) -> DiagnosticContextRef<'_> {
        let entry = self.diagnostics.entry(ident.to_string()).or_default();
        DiagnosticContextRef {
            diagnostics: entry,
            path: &mut self.path,
            pop_on_exit: false,
        }
    }

    pub fn enter_new(&mut self, ident: impl Display) -> DiagnosticContextRef<'_> {
        if self.diagnostics.contains_key(&ident.to_string()) {
            panic!("Diagnostic context already exists for {}", ident);
        }

        self.enter(ident)
    }
}

pub struct DiagnosticContextRef<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    path: &'a mut DiagnosticPath,
    pop_on_exit: bool,
}

impl<'a> DiagnosticContextRef<'a> {
    pub fn emit(&mut self, diagnostic: DiagnosticKind) {
        self.diagnostics.push(Diagnostic {
            path: self.path.clone(),
            kind: diagnostic,
        })
    }

    pub fn enter(&mut self, segment: impl Into<DiagnosticPathSegment>) -> DiagnosticContextRef<'_> {
        self.path.push(segment);
        DiagnosticContextRef {
            diagnostics: self.diagnostics,
            path: self.path,
            pop_on_exit: true,
        }
    }

    pub fn enter_index(&mut self, index: usize) -> DiagnosticContextRef<'_> {
        self.enter(DiagnosticPathSegment::Index(index))
    }

    pub fn enter_field(&mut self, field: impl Into<Cow<'static, str>>) -> DiagnosticContextRef<'_> {
        self.enter(DiagnosticPathSegment::Field(field.into()))
    }

    pub fn enter_variant(
        &mut self,
        variant: impl Into<Cow<'static, str>>,
    ) -> DiagnosticContextRef<'_> {
        self.enter(DiagnosticPathSegment::Variant(variant.into()))
    }
}

impl<'a> Drop for DiagnosticContextRef<'a> {
    fn drop(&mut self) {
        if self.pop_on_exit {
            self.path.pop();
        }
    }
}
