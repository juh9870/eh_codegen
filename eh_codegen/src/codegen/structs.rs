use std::collections::BTreeMap;
use std::str::FromStr;

use convert_case::{Case, Casing};
use itertools::Itertools;
use miette::{bail, miette, Context, IntoDiagnostic, Result};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use codegen_schema::schema::{SchemaStructMember, SchemaStructMemberType};

use crate::codegen::{CodegenState, TokensResult};

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub ty: TokenStream,
    pub default_value: Option<TokenStream>,
    pub serde_default: Option<Ident>,
    pub skip_serializing_if: Option<Ident>,
    pub field: SchemaStructMember,
}

impl Field {
    pub fn new(field: SchemaStructMember, struct_name: &Ident) -> Result<Self> {
        let name_snake = field.name.from_case(Case::Pascal).to_case(Case::Snake);
        let (ty, no_default) = rust_type(&field, struct_name)?;
        let ident = format_ident!("r#{}", name_snake);
        let default_value = (!no_default).then(|| default_value(&field)).transpose()?;
        let cleaned_value_name = default_value.as_ref().map(|v| {
            if v.to_string().replace(' ', "") == "Default::default()" {
                return "default".to_string();
            }
            v.to_string()
                .replace('.', "ඞdotඞ")
                .replace(':', "ඞcolonඞ")
                .replace(' ', "ඞspaceඞ")
                .replace('-', "ඞdashඞ")
                .replace('+', "ඞplusඞ")
                .replace('(', "ඞlparenඞ")
                .replace(')', "ඞrparenඞ")
                .replace('[', "ඞlbracketඞ")
                .replace(']', "ඞrbracketඞ")
                .replace('{', "ඞlbraceඞ")
                .replace('}', "ඞrbraceඞ")
                .replace('=', "ඞeqඞ")
                .replace('!', "ඞbangඞ")
                .replace('@', "ඞatඞ")
                .replace('#', "ඞhashඞ")
                .replace('$', "ඞdollarඞ")
                .replace('%', "ඞpercentඞ")
                .replace('^', "ඞcaretඞ")
                .replace('&', "ඞampඞ")
                .replace('*', "ඞstarඞ")
                .replace('?', "ඞquestionඞ")
                .replace('/', "ඞslashඞ")
                .replace('\\', "ඞbackslashඞ")
                .replace('|', "ඞpipeඞ")
                .replace('~', "ඞtildeඞ")
                .replace('`', "ඞbacktickඞ")
                .replace('"', "ඞquoteඞ")
                .replace('\'', "ඞsquoteඞ")
                .replace('<', "ඞltඞ")
                .replace('>', "ඞgtඞ")
                .replace(',', "ඞcommaඞ")
                .replace(';', "ඞsemicolonඞ")
                .replace('+', "ඞplusඞ")
        });
        let serde_default = cleaned_value_name
            .as_ref()
            .map(|val| format_ident!("default_{}", val));
        let skip_serializing_if = cleaned_value_name.map(|val| format_ident!("skip_if_{}", val));

        Ok(Field {
            ident,
            ty,
            field,
            default_value,
            serde_default,
            skip_serializing_if,
        })
    }

    pub fn struct_field(&self) -> TokenStream {
        let Self {
            ident,
            ty,
            serde_default,
            skip_serializing_if,
            field,
            ..
        } = self;

        let desc = field.description.as_ref().map(|s| quote!(#[doc = #s]));
        let serde_default = serde_default.as_ref().map(|s| s.to_string()).map(|s| {
            if s == "default_default" {
                quote! {#[serde(default)]}
            } else {
                quote!(#[serde(default=#s)])
            }
        });
        let skip_serializing_if = skip_serializing_if
            .as_ref()
            .map(|s| s.to_string())
            .and_then(|s| {
                if s == "skip_if_default" {
                    None
                } else {
                    Some(quote!(#[serde(skip_serializing_if=#s)]))
                }
            })
            .or_else(|| {
                if ty.to_string().starts_with("Option") {
                    Some(quote!(#[serde(skip_serializing_if = "Option::is_none")]))
                } else {
                    None
                }
            });

        let serde_with = match field.ty {
            SchemaStructMemberType::Vector => quote!(#[serde(with = "crate::helpers::glam_ser")]),
            _ => quote!(),
        };
        quote! {
            #desc
            #serde_default
            #skip_serializing_if
            #serde_with
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
            if value.to_string().contains("default ()") {
                quote! {
                    #ident: #value,
                }
            } else {
                quote! {
                    #ident: #value.into(),
                }
            }
        } else {
            quote! {
                #ident,
            }
        }
    }

    fn eq_code(&self) -> TokenStream {
        let Self { ident, field, .. } = self;
        match field.ty {
            SchemaStructMemberType::Float => {
                quote! {
                    ordered_float::OrderedFloat(self.#ident) == ordered_float::OrderedFloat(other.#ident)
                }
            }
            SchemaStructMemberType::Vector => {
                quote! {
                    ordered_float::OrderedFloat(self.#ident.x) == ordered_float::OrderedFloat(other.#ident.x)
                    && ordered_float::OrderedFloat(self.#ident.y) == ordered_float::OrderedFloat(other.#ident.y)
                }
            }
            _ => {
                quote! {&self.#ident == &other.#ident}
            }
        }
    }

    fn hash_code(&self) -> TokenStream {
        let Self { ident, field, .. } = self;
        match field.ty {
            SchemaStructMemberType::Float => {
                quote! {
                    ordered_float::OrderedFloat(self.#ident).hash(state);
                }
            }
            SchemaStructMemberType::Vector => {
                quote! {
                    ordered_float::OrderedFloat(self.#ident.x).hash(state);
                    ordered_float::OrderedFloat(self.#ident.y).hash(state);
                }
            }
            _ => {
                quote! {self.#ident.hash(state);}
            }
        }
    }

    fn need_custom_eq_hash(&self) -> bool {
        matches!(
            self.field.ty,
            SchemaStructMemberType::Float | SchemaStructMemberType::Vector
        )
    }

    fn validation(&self) -> TokensResult {
        let Self {
            ident, ty, field, ..
        } = self;

        let mut validation = vec![];
        let name_str = ident.to_string();
        let name_str = name_str.strip_prefix("r#").unwrap_or(&name_str);

        if !matches!(field.ty, SchemaStructMemberType::Expression) {
            if let Some(min) = &field.minvalue {
                validation.push(quote! {
                    if self.#ident < (#min as #ty) {
                        ctx.emit(DiagnosticKind::too_small(#min, self.#ident));
                    }
                })
            }
            if let Some(max) = &field.maxvalue {
                validation.push(quote! {
                    if self.#ident > (#max as #ty) {
                        ctx.emit(DiagnosticKind::too_large(#max, self.#ident));
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
                                ctx.emit(DiagnosticKind::obsolete_field());
                            }
                        })
                    }
                    opt => bail!("Encountered an unknown option: {}", opt),
                }
            }
        }

        match field.ty {
            SchemaStructMemberType::Struct => {
                validation.push(quote! {
                    self.#ident.validate(ctx);
                });
            }
            SchemaStructMemberType::StructList => {
                validation.push(quote! {
                    for (i, x) in self.#ident.iter().enumerate() {
                        let mut ctx = ctx.enter(i);
                        x.validate(ctx);
                    }
                });
            }
            SchemaStructMemberType::Object => {}
            SchemaStructMemberType::ObjectList => {}
            SchemaStructMemberType::Enum => {}
            SchemaStructMemberType::EnumFlags => {}
            SchemaStructMemberType::Expression => {}
            SchemaStructMemberType::Vector => {}
            SchemaStructMemberType::Float => {}
            SchemaStructMemberType::Int => {}
            SchemaStructMemberType::Color => {}
            SchemaStructMemberType::Bool => {}
            SchemaStructMemberType::String => {}
            SchemaStructMemberType::Image => {}
            SchemaStructMemberType::AudioClip => {}
            SchemaStructMemberType::Prefab => {}
            SchemaStructMemberType::Layout => validation.push(quote! {
                if (self.#ident.len() as f32).sqrt().floor().powi(2) != (self.#ident.len() as f32) {
                    ctx.emit(DiagnosticKind::layout_not_square(self.#ident.len()));
                }
            }),
        };

        if !validation.is_empty() {
            Ok(quote! {
                {
                    let mut ctx = ctx.enter(#name_str);
                    #(#validation)*
                }
            })
        } else {
            Ok(quote! {})
        }
    }

    pub fn add_extra_functions(&self, funcs: &mut BTreeMap<String, TokenStream>) {
        let ty = &self.ty;
        let Some(default) = &self.default_value else {
            return;
        };
        if let Some(name) = &self.serde_default {
            if name != "default_default" {
                funcs.entry(name.to_string()).or_insert_with(|| {
                    quote! {
                        #[allow(non_snake_case)]
                        pub fn #name() -> #ty {
                            #default.into()
                        }
                    }
                });
            }
        }
        if let Some(name) = &self.skip_serializing_if {
            if name != "skip_if_default" {
                let lhs = match self.field.ty {
                    SchemaStructMemberType::Int
                    | SchemaStructMemberType::Float
                    | SchemaStructMemberType::Bool => quote!(*x),
                    _ => quote!(x),
                };
                funcs.entry(name.to_string()).or_insert_with(|| {
                    quote! {
                        #[allow(non_snake_case)]
                        pub fn #name(x: &#ty) -> bool {
                            #lhs == #default
                        }
                    }
                });
            }
        }
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

        for f in &fields {
            f.add_extra_functions(&mut self.extra_functions)
        }

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

        let need_eq_hash_impls = fields.iter().any(|f| f.need_custom_eq_hash());
        let custom_eq_hash_impls = need_eq_hash_impls.then(|| {
            let eq_impl = fields.iter().enumerate().map(|(i, f)| {
                let eq = f.eq_code();
                if i == 0 {
                    eq
                } else {
                    quote! {&& #eq}
                }
            });

            let custom_eq_code = fields.is_empty().then(|| quote! {true});

            let hash_impl = fields.iter().map(|f| f.hash_code());
            quote! {
                impl std::cmp::Eq for #name {}

                impl std::cmp::PartialEq for #name {
                    fn eq(&self, other: &Self) -> bool {
                        #(#eq_impl)*
                        #custom_eq_code
                    }
                }

                impl std::hash::Hash for #name {
                    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                        #(#hash_impl)*
                    }
                }
            }
        });

        let eq_hash_derives = (!need_eq_hash_impls).then(|| {
            quote! {
                #[derive(Eq, PartialEq, Hash)]
            }
        });

        let name_str = name.to_string();

        let code = quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            #eq_hash_derives
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
                fn validate(&self, mut ctx: DiagnosticContextRef) {
                    #(#validations)*
                }

                fn type_name() -> &'static str {
                    #name_str
                }
            }

            #custom_eq_hash_impls

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
        return Ok(match field.ty {
            SchemaStructMemberType::Int => {
                quote! {0}
            }
            SchemaStructMemberType::Bool => {
                quote! {false}
            }
            SchemaStructMemberType::Float => {
                quote! {0.0}
            }
            SchemaStructMemberType::Color => {
                quote! {"#00000000"}
            }
            _ => quote!(Default::default()),
        });
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
        SchemaStructMemberType::String => quote!(#default),
        SchemaStructMemberType::Expression => quote!(#default),
        SchemaStructMemberType::Color => quote!(#default),
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
                quote!(std::collections::BTreeSet::<#id>)
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
