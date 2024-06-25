use std::str::FromStr;

use convert_case::{Case, Casing};
use itertools::Itertools;
use miette::{bail, miette, Context, IntoDiagnostic, Result};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::codegen::{CodegenState, TokensResult};
use crate::schema::{SchemaStructMember, SchemaStructMemberType};

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub ty: TokenStream,
    pub default_value: Option<TokenStream>,
    pub field: SchemaStructMember,
}

impl Field {
    pub fn new(field: SchemaStructMember, struct_name: &Ident) -> Result<Self> {
        let name_snake = field.name.from_case(Case::Pascal).to_case(Case::Snake);
        let (ty, no_default) = rust_type(&field, struct_name)?;
        let ident = format_ident!("r#{}", name_snake);
        let default_value = (!no_default).then(|| default_value(&field)).transpose()?;

        Ok(Field {
            ident,
            ty,
            field,
            default_value,
        })
    }

    pub fn struct_field(&self) -> TokenStream {
        let Self {
            ident, ty, field, ..
        } = self;

        let desc = field.description.as_ref().map(|s| quote!(#[doc = #s]));
        quote! {
            #desc
            pub #ident: #ty,
        }
    }

    pub fn builder_fn(&self) -> TokenStream {
        let Self { ident, ty, .. } = self;

        let i = ident.to_string().replace("r#", "");
        let builder_fn_ident = format_ident!("with_{}", i);
        let setter_fn_ident = format_ident!("set_{}", i);

        quote! {
            pub fn #builder_fn_ident(mut self, #ident: impl Into<#ty>) -> Self {
                self.#ident = #ident.into();
                self
            }
            pub fn #setter_fn_ident(&mut self, #ident: impl Into<#ty>) -> &mut Self {
                self.#ident = #ident.into();
                self
            }
        }
    }

    pub fn constructor_entry(&self) -> TokenStream {
        let Self {
            ident,
            default_value,
            ..
        } = self;
        if let Some(value) = default_value {
            quote! {
                #ident: #value,
            }
        } else {
            quote! {
                #ident,
            }
        }
    }

    fn validation(&self) -> TokensResult {
        let Self {
            ident, ty, field, ..
        } = self;

        let mut validation = vec![];
        let name_str = ident.to_string();

        if !matches!(field.ty, SchemaStructMemberType::Expression) {
            if let Some(min) = &field.minvalue {
                validation.push(quote! {
                    if self.#ident < (#min as #ty) {
                        tracing::warn!(field=#name_str, value=self.#ident, min=#min, "Field got truncated");
                        self.#ident = #min as #ty;
                    }
                })
            }
            if let Some(max) = &field.maxvalue {
                validation.push(quote! {
                    if self.#ident > (#max as #ty) {
                        tracing::warn!(field=#name_str, value=self.#ident, max=#max, "Field got truncated");
                        self.#ident = #max as #ty;
                    }
                })
            }
        }

        if let Some(options) = &field.options {
            let options = options.split(',').map(|e| e.trim());
            for opt in options {
                match opt {
                    "notnull" => {
                        // Handled elsewhere
                    }
                    "obsolete" => {
                        let default_val = &self
                            .default_value
                            .as_ref()
                            .ok_or_else(|| miette!("Obsolete notnull fields are not supported"))?;
                        let ty = &self.ty;
                        validation.push(quote! {
                            let dw: #ty = #default_val;
                            if self.#ident != dw {
                                tracing::error!(ield=#name_str, "Obsolete field usage detected, generated code may not work",);
                            }
                        })
                    }
                    opt => bail!("Encountered an unknown option: {}", opt),
                }
            }
        }

        Ok(quote! {
            #(#validation)*
        })
    }
}

#[derive(Debug)]
pub struct StructData {
    pub ident: Ident,
    #[allow(dead_code)]
    pub fields: Vec<Field>,
    pub id_access: Option<TokenStream>,
    pub code: TokenStream,
    pub ctor_params: Option<Vec<Field>>,
    pub has_default: bool,
}

impl CodegenState {
    pub fn codegen_struct(
        &mut self,
        name: Ident,
        mut fields: Vec<SchemaStructMember>,
        switch: Option<String>,
    ) -> Result<StructData> {
        if let Some(switch) = switch {
            return self.codegen_switch_struct(name, fields, switch);
        }
        fields.dedup_by(|a, b| a.name == b.name);

        if fields.iter().enumerate().any(|(i1, f1)| {
            fields
                .iter()
                .enumerate()
                .any(|(i2, f2)| f1.name == f2.name && i1 != i2)
        }) {
            bail!("Struct {name} contains duplicate fields");
        }

        let fields: Vec<Field> = fields
            .into_iter()
            .map(|f| Field::new(f, &name))
            .try_collect()?;

        let struct_fields = fields.iter().map(|f| f.struct_field());
        let builder_fns = fields.iter().map(|f| f.builder_fn());

        let (_with_defaults, contructed) = fields
            .iter()
            .partition::<Vec<_>, _>(|f| f.default_value.is_some());

        let field_construction = fields.iter().map(|f| f.constructor_entry());
        let constructor_arguments = contructed
            .iter()
            .map(|Field { ident, ty, .. }| quote!(#ident: #ty,));

        let validations: Vec<_> = fields.iter().map(|f| f.validation()).try_collect()?;

        let default_impl = contructed.is_empty().then(|| {
            quote! {
                impl Default for #name {
                    fn default() -> Self {
                        Self::new()
                    }
                }
            }
        });

        let name_str = name.to_string();

        let code = quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            #[serde(rename_all = "PascalCase")]
            pub struct #name {
                #(#struct_fields)*
            }

            impl #name {
                pub fn new(#(#constructor_arguments)*) -> Self {
                    Self {
                        #(#field_construction)*
                    }
                }

                #(#builder_fns)*
            }

            impl DatabaseItem for #name {
                fn validate(&mut self) {
                    #(#validations)*
                }

                fn type_name() -> &'static str {
                    #name_str
                }
            }

            #default_impl
        };
        Ok(StructData {
            ident: name,
            ctor_params: (!contructed.is_empty())
                .then(|| contructed.into_iter().cloned().collect()),
            fields,
            id_access: None,
            code,
            has_default: default_impl.is_some(),
        })
    }
}

fn default_value(field: &SchemaStructMember) -> TokensResult {
    let Some(default) = &field.default else {
        return Ok(quote!(Default::default()));
    };

    Ok(match field.ty {
        SchemaStructMemberType::Int => {
            let value = i32::from_str(default)
                .into_diagnostic()
                .context("Encountered an issue during parsing default int value")?;
            quote!(#value)
        }
        SchemaStructMemberType::Bool => {
            let value = bool::from_str(default)
                .into_diagnostic()
                .context("Encountered an issue during parsing default bool value")?;
            quote!(#value)
        }
        SchemaStructMemberType::Float => {
            let value = f32::from_str(default)
                .into_diagnostic()
                .context("Encountered an issue during parsing default float value")?;
            quote!(#value)
        }
        _ => quote!(#default.to_string()),
    })
}

fn rust_type(field: &SchemaStructMember, struct_name: &Ident) -> Result<(TokenStream, bool)> {
    let type_id = || {
        field
            .typeid
            .as_ref()
            .map(|id| format_ident!("{}", id))
            .ok_or_else(|| miette!("Missing typeid field"))
    };
    let object_id = || {
        field
            .typeid
            .as_ref()
            .map(|id| format_ident!("{}Id", id))
            .ok_or_else(|| miette!("Missing typeid field"))
    };
    Ok((
        match field.ty {
            SchemaStructMemberType::Struct => {
                let id = type_id()?;

                if struct_name.to_string().contains(&id.to_string()) {
                    quote!(Box::<#id>)
                } else {
                    quote!(#id)
                }
            }
            SchemaStructMemberType::StructList => {
                let id = type_id()?;
                quote!(Vec<#id>)
            }
            SchemaStructMemberType::Object => {
                let id = object_id()?;
                if field
                    .options
                    .as_ref()
                    .is_some_and(|opts| opts.contains("notnull"))
                {
                    return Ok((quote!(#id), true));
                } else {
                    quote!(Option<#id>)
                }
            }
            SchemaStructMemberType::ObjectList => {
                let id = object_id()?;
                quote!(Vec<#id>)
            }
            SchemaStructMemberType::Enum => {
                let id = type_id()?;
                quote!(#id)
            }
            SchemaStructMemberType::EnumFlags => {
                let id = type_id()?;
                quote!(std::collections::HashSet::<#id>)
            }
            SchemaStructMemberType::Expression => {
                // MAYBE?: something smarter for expressions?
                quote!(String)
            }
            SchemaStructMemberType::Vector => {
                quote!(glam::f32::Vec2)
            }
            SchemaStructMemberType::Float => {
                quote!(f32)
            }
            SchemaStructMemberType::Int => {
                quote!(i32)
            }
            SchemaStructMemberType::Color => {
                quote!(String)
            }
            SchemaStructMemberType::Bool => {
                quote!(bool)
            }
            SchemaStructMemberType::String => {
                quote!(String)
            }
            SchemaStructMemberType::Image => {
                quote!(String)
            }
            SchemaStructMemberType::AudioClip => {
                quote!(String)
            }
            SchemaStructMemberType::Prefab => {
                quote!(String)
            }
            SchemaStructMemberType::Layout => {
                quote!(String)
            }
        },
        false,
    ))
}
