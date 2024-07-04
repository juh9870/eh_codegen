use crate::codegen::structs::{Field, StructData};
use crate::codegen::switch::Variant;
use crate::schema::{SchemaDataType, SchemaItem};
use convert_case::{Case, Casing};

use itertools::Itertools;
use miette::{miette, Diagnostic, IntoDiagnostic, LabeledSpan, Report, SourceCode};
use miette::{Context, Result};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, HashMap};
use std::iter::once;
use thiserror::Error;

mod enums;
mod objects;
mod structs;
mod switch;

type TokensResult = Result<TokenStream>;

#[derive(Debug, Default)]
pub struct CodegenState {
    pub enums: HashMap<String, Vec<String>>,
    pub objects: HashMap<String, StructData>,
    pub extra_functions: BTreeMap<String, TokenStream>,
}

impl CodegenState {
    pub fn codegen(&mut self, item: SchemaItem) -> Result<Option<TokenStream>> {
        let tokens = match item {
            SchemaItem::Schema { .. } => {
                quote! {
                    pub use crate::helpers::*;
                    use diagnostic::prelude::*;
                }
            }
            SchemaItem::Data(data) => {
                let ident = format_ident!("{}", data.name);

                match data.ty {
                    SchemaDataType::Struct | SchemaDataType::Settings => {
                        let obj = self
                            .codegen_struct(
                                ident,
                                data.member.ok_or_else(|| {
                                    miette!("Got struct or settings without members")
                                })?,
                                data.switch,
                            )
                            .context("Failed to generate struct data")?;
                        let code = obj.code.clone();
                        if let Some(id) = &data.typeid {
                            self.objects.insert(id.clone(), obj);
                        }
                        code
                    }
                    SchemaDataType::Object => {
                        let obj = self
                            .codegen_object(ident, data.member.unwrap_or_default(), data.switch)
                            .context("Failed to generate object data")?;
                        let code = obj.code.clone();
                        if let Some(id) = &data.typeid {
                            self.objects.insert(id.clone(), obj);
                        }
                        code
                    }
                    SchemaDataType::Enum => self
                        .codegen_enum(
                            ident,
                            data.item.ok_or_else(|| miette!("Got enum without items"))?,
                        )
                        .context("Failed to generate enum data")?,
                    SchemaDataType::Expression => return Ok(None),
                }
            }
        };

        Ok(Some(tokens))
    }

    pub fn codegen_core_db_item(&mut self) -> TokensResult {
        let data = self
            .enums
            .get("ItemType")
            .ok_or_else(|| miette!("`ItemType` enum was not present in schema"))?;

        let mut variants = vec![];
        for variant in data {
            if variant == "Undefined" {
                continue;
            }
            let Some(data) = self.objects.remove(variant) else {
                eprintln!(
                    "Object or Setting with typeid `{}` was not present in schema",
                    variant
                );
                continue;
            };
            // let data = self.objects.remove(variant).ok_or_else(|| {
            //     miette!(
            //         "Object or Setting with typeid `{}` was not present in schema",
            //         variant
            //     )
            // })?;
            variants.push(Variant {
                ident: format_ident!("{variant}"),
                data,
            })
        }

        let id_fetchers = variants.iter().map(|Variant { ident, data }| {
            if let Some(id_access) = &data.id_access {
                quote! {
                    Self::#ident(x) => Some((#id_access).0),
                }
            } else {
                quote! {
                    Self::#ident(_) => None,
                }
            }
        });

        let lower_idents = variants
            .iter()
            .map(|v| {
                let ident_lower = format_ident!(
                    "{}",
                    v.ident
                        .to_string()
                        .from_case(Case::Pascal)
                        .to_case(Case::Snake)
                );
                (ident_lower, v)
            })
            .collect_vec();

        let contructor_macro_invocations =
            lower_idents
                .iter()
                .filter_map(|(ident_lower, Variant { ident, data })| {
                    if !data.has_default && data.ctor_params.is_none() {
                        return None;
                    }

                    let args = data.ctor_params.as_ref().map(|params| {
                        let args = params.iter().map(|Field { ident, ty, .. }| {
                            let ty_str = ty.to_string();
                            if ty_str.ends_with("Id") {
                                let ty = format_ident!("{}", &ty_str[..(ty_str.len() - 2)]);
                                quote!(#ident: (DatabaseItemId<#ty>))
                            } else {
                                quote!(#ident: (#ty))
                            }
                        });
                        quote!(#(#args),*)
                    });
                    Some(quote!(#ident_lower(#args) -> #ident))
                });

        let all_items_macro = lower_idents
            .iter()
            .map(|(lower_ident, Variant { ident, .. })| {
                quote! {
                    #lower_ident: #ident
                }
            });
        let all_settings_macro =
            lower_idents
                .iter()
                .filter_map(|(lower_ident, Variant { ident, data })| {
                    if data.id_access.is_some() {
                        return None;
                    }
                    Some(quote! {
                        #lower_ident: #ident
                    })
                });
        let all_collections_macro =
            lower_idents
                .iter()
                .filter_map(|(lower_ident, Variant { ident, data })| {
                    data.id_access.as_ref()?;
                    Some(quote! {
                        #lower_ident: #ident
                    })
                });

        let ident = format_ident!("Item");
        let code = self.codegen_custom_switch(
            ident.clone(),
            format_ident!("ItemType"),
            variants.as_slice(),
            false,
            [],
            "ItemType",
            false,
        )?;

        Ok(quote! {
            #code

            impl #ident {
                /// Fetches untyped ID of the inner item, or None if content is a setting
                pub fn id(&self) -> Option<i32> {
                    match self {
                        #(#id_fetchers)*
                    }
                }
            }

            #[macro_export]
            macro_rules! apply_constructors {
                ($macro_name:ident) => {
                    $macro_name! {
                        #(#contructor_macro_invocations),*
                    }
                }
            }

            #[macro_export]
            macro_rules! apply_all_items {
                ($macro_name:ident) => {
                    $macro_name! {
                        #(#all_items_macro),*
                    }
                }
            }

            #[macro_export]
            macro_rules! apply_all_settings {
                ($macro_name:ident) => {
                    $macro_name! {
                        #(#all_settings_macro),*
                    }
                }
            }

            #[macro_export]
            macro_rules! apply_all_collections {
                ($macro_name:ident) => {
                    $macro_name! {
                        #(#all_collections_macro),*
                    }
                }
            }
        })
    }

    pub fn codegen_extra_functions(&mut self) -> TokensResult {
        let values = self.extra_functions.values();
        Ok(quote! {
            #(#values)*
        })
    }

    pub fn format_tokens(tokens: Option<TokenStream>) -> Result<Option<String>> {
        match tokens {
            None => Ok(None),
            Some(tokens) => {
                let source = tokens.to_string();

                let text = prettyplease::unparse(
                    &syn::parse_file(&source)
                        .into_diagnostic()
                        .map_err(|e| SourceParseError(source, e))
                        .context("Generated code is not a valid Rust")?,
                );
                Ok(Some(text))
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("{}", .1)]
struct SourceParseError(String, Report);

impl Diagnostic for SourceParseError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.0)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(once(LabeledSpan::new(None, 0, self.0.len()))))
    }
}
