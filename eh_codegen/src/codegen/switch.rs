use std::cell::OnceCell;
use std::collections::HashMap;

use convert_case::{Case, Casing};
use itertools::Itertools;
use miette::{bail, miette};
use miette::{Context, Result};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::codegen::structs::{Field, StructData};
use crate::codegen::{CodegenState, TokensResult};
use crate::schema::{SchemaStructMember, SchemaStructMemberType};

impl CodegenState {
    pub fn codegen_switch_struct(
        &mut self,
        ident: Ident,
        mut fields: Vec<SchemaStructMember>,
        switch: String,
    ) -> Result<StructData> {
        let switch_field_idx = fields
            .iter()
            .enumerate()
            .find(|f| f.1.name == switch)
            .map(|(i, _)| i)
            .ok_or_else(|| miette!("switch field points at a missing field"))?;

        let switch_field = fields.remove(switch_field_idx);

        if !matches!(switch_field.ty, SchemaStructMemberType::Enum) {
            bail!("switch field must be an enum");
        };
        let Some(enum_ty) = &switch_field.typeid else {
            bail!("switch field is missing a typeid")
        };
        let Some(enum_items) = self.enums.get(enum_ty) else {
            bail!("switch typeid points at the unknown enum `{}`", enum_ty)
        };
        let enum_items = enum_items.clone();
        let enum_ident = format_ident!("{enum_ty}");

        let mut variants: HashMap<String, Vec<SchemaStructMember>> = HashMap::default();

        for item in &enum_items {
            variants.insert(item.clone(), vec![]);
        }

        let mut neutrals = vec![];

        for field in &fields {
            match &field.case {
                None => {
                    for members in variants.values_mut() {
                        members.push(field.clone())
                    }
                    neutrals.push(field.clone());
                }
                Some(cases) => {
                    let cases = cases.split(',').map(|e| e.trim());
                    for case in cases {
                        let Some(members) = variants.get_mut(case) else {
                            bail!("Field {} contains unknown case `{}`", &field.name, case)
                        };

                        members.push(field.clone());
                    }
                }
            }
        }

        let _default_item = &variants[&enum_items[0]];

        let has_default = OnceCell::<bool>::new();

        let enum_variants: Vec<Variant> = enum_items
            .iter()
            .map(|variant| {
                let variant_ident = format_ident!("{}{}", ident, variant);
                let members = variants.remove(variant).unwrap_or_else(|| neutrals.clone());
                let data = self.codegen_struct(variant_ident.clone(), members, None)?;
                has_default.get_or_init(|| data.has_default);
                Result::<Variant>::Ok(Variant {
                    ident: format_ident!("{variant}"),
                    data,
                })
            })
            .try_collect()?;

        let has_default = has_default.into_inner().unwrap_or(false);

        if !variants.is_empty() {
            return Err(miette!(
                "Enum {enum_ty} does not include the following variant(s): {}",
                variants.keys().join(", ")
            ))
            .with_context(|| format!("Struct {ident} contains bas case statement"));
        }

        let switch_code = self.codegen_custom_switch(
            ident.clone(),
            enum_ident,
            enum_variants.as_slice(),
            has_default,
            neutrals,
            &switch_field.name,
            true,
        )?;

        Ok(StructData {
            ident,
            fields: vec![],
            id_access: None,
            code: switch_code,
            ctor_params: None,
            has_default,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn codegen_custom_switch(
        &mut self,
        switch_struct_ident: Ident,
        enum_ident: Ident,
        variants: &[Variant],
        has_default: bool,
        common_fields: impl IntoIterator<Item = SchemaStructMember>,
        tag_field: &str,
        generate_structs: bool,
    ) -> TokensResult {
        let enum_variants = variants.iter().map(|Variant { ident, data, .. }| {
            let content = &data.ident;
            quote! {
                #ident(#content),
            }
        });

        let default_impl = has_default.then(|| {
            let first_variant = &variants[0].ident;
            quote! {
                impl Default for #switch_struct_ident {
                    fn default() -> Self {
                        Self::#first_variant(Default::default())
                    }
                }
            }
        });

        let mut blocks: Vec<TokenStream> = vec![];

        let shared_enum = quote! {
            #[derive(Debug, Clone)]
            pub enum #switch_struct_ident {
                #(#enum_variants)*
            }

            #default_impl
        };

        blocks.push(shared_enum);

        let ident_str = switch_struct_ident.to_string();

        let shared_fields: Vec<Field> = common_fields
            .into_iter()
            .map(|f| Field::new(f, &switch_struct_ident))
            .try_collect()?;

        for Variant { ident, data, .. } in variants {
            let code = &data.code;
            let content = &data.ident;

            if generate_structs {
                blocks.push(code.clone())
            }

            blocks.push(quote! {
                impl From<#content> for #switch_struct_ident {
                    fn from(item: #content) -> Self {
                        Self::#ident(item)
                    }
                }

                impl #content {
                    pub fn wrap(self) -> #switch_struct_ident {
                        self.into()
                    }
                }
            });

            let builder_name = format_ident!(
                "{}",
                ident
                    .to_string()
                    .from_case(Case::Pascal)
                    .to_case(Case::Snake)
            );
            if let Some(params) = &data.ctor_params {
                let args = params
                    .iter()
                    .map(|Field { ident, ty, .. }| quote!(#ident: #ty,));
                let call_args = params.iter().map(|Field { ident, .. }| quote!(#ident,));
                blocks.push(quote! {
                    impl #switch_struct_ident {
                        pub fn #builder_name(#(#args)*) -> #content {
                            #content::new(#(#call_args)*)
                        }
                    }
                })
            } else if data.has_default {
                blocks.push(quote! {
                    impl #switch_struct_ident {
                        pub fn #builder_name() -> #content {
                            #content::new()
                        }
                    }
                })
            }
        }

        let matcher = |body: TokenStream, is_mut: bool| {
            let x_pat = if is_mut { quote!(ref mut x) } else { quote!(x) };
            let matches = variants.iter().map(|v| {
                let name = &v.ident;
                quote! {
                    Self::#name(#x_pat) => {#body}
                }
            });

            quote! {
                match self {
                    #(#matches)*
                }
            }
        };

        let serde_ser_matcher = variants.iter().map(|v| {
            let name = &v.ident;
            quote! {
                Self::#name(x) => AdjTagged { t: #enum_ident::#name, c: x }.serialize(serializer),
            }
        });
        let serde_deser_matcher = variants.iter().map(|v| {
            let name = &v.ident;
            quote! {
                #enum_ident::#name => Self::#name(serde_json::from_value(data).map_err(serde::de::Error::custom)?),
            }
        });

        let serde_impl = quote! {
            impl serde::Serialize for #switch_struct_ident {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer
                {
                    #[derive(serde::Serialize)]
                    #[serde(rename = #ident_str)]
                    struct AdjTagged<T> {
                        #[serde(rename = #tag_field)]
                        t: #enum_ident,
                        #[serde(flatten)]
                        c: T,
                    }

                    match self {
                        #(#serde_ser_matcher)*
                    }
                }
            }

            impl<'de> serde::Deserialize<'de> for #switch_struct_ident {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::de::Deserializer<'de> {
                    let data = serde_json::Value::deserialize(deserializer)?;
                    let variant_ty: #enum_ident = if let Some(variant) = data.get(#tag_field) {
                        serde_json::from_value(variant.clone()).map_err(serde::de::Error::custom)?
                        // return Err(serde::de::Error::missing_field(#tag_field));
                    } else {
                        Default::default()
                    };

                    let value = match variant_ty {
                        #(#serde_deser_matcher)*
                        _ => {
                            return Err(serde::de::Error::unknown_variant((variant_ty as i32).to_string().as_str(), &[]))
                        }
                    };
                    Ok(value)
                }
            }
        };

        blocks.push(serde_impl);

        let (deref_box_matchers, deref_matchers, deref_mut_matchers): (Vec<_>, Vec<_>, Vec<_>) =
            variants
                .iter()
                .map(|v| {
                    let name = &v.ident;
                    (
                        quote! {
                            Self::#name(x) => Box::new(x) as Box<dyn std::any::Any>,
                        },
                        quote! {
                            Self::#name(x) => x as &dyn std::any::Any,
                        },
                        quote! {
                            Self::#name(x) => x as &mut dyn std::any::Any,
                        },
                    )
                })
                .multiunzip();

        let deref_impl = quote! {
            impl #switch_struct_ident {
                pub fn into_inner_any(self) -> Box<dyn std::any::Any> {
                    match self {
                        #(#deref_box_matchers)*
                    }
                }
                pub fn as_inner_any_ref(&self) -> &dyn std::any::Any {
                    match self {
                        #(#deref_matchers)*
                    }
                }
                pub fn as_inner_any_mut(&mut self) -> &mut dyn std::any::Any {
                    match self {
                        #(#deref_mut_matchers)*
                    }
                }
            }
        };

        blocks.push(deref_impl);

        for Field {
            ident: field_name,
            ty,
            ..
        } in shared_fields
        {
            let field_name_mut = format_ident!("{}_mut", field_name);
            let field_name_setter = format_ident!("set_{}", field_name);
            let field_name_with = format_ident!("with_{}", field_name);
            let access = matcher(quote!(&x.#field_name), false);
            let access_mut = matcher(quote!(&mut x.#field_name), false);
            let setter = matcher(quote!(x.#field_name = value.into()), false);
            let with_setter = matcher(quote!(x.#field_name = value.into()), true);
            blocks.push(quote! {
                impl #switch_struct_ident {
                    pub fn #field_name(&self) -> &#ty {
                        #access
                    }

                    pub fn #field_name_mut(&mut self) -> &mut #ty {
                        #access_mut
                    }

                    pub fn #field_name_setter(&mut self, value: impl Into<#ty>) -> &mut Self {
                        #setter
                        self
                    }

                    pub fn #field_name_with(mut self, value: impl Into<#ty>) -> Self {
                        #with_setter
                        self
                    }
                }
            });
        }

        let validations = matcher(quote!(x.validate()), false);

        blocks.push(quote! {
            impl DatabaseItem for #switch_struct_ident {
                fn validate(&mut self) {
                    #validations
                }

                fn type_name() -> &'static str {
                    #ident_str
                }
            }
        });

        let type_names = variants.iter().map(|Variant { ident, data }| {
            let ty = &data.ident;
            quote!(Self::#ident(_) => #ty::type_name(),)
        });
        blocks.push(quote! {
            impl #switch_struct_ident {
                pub fn inner_type_name(&self) -> &'static str {
                    match self {
                        #(#type_names)*
                    }
                }
            }
        });

        Ok(quote! {
            #(#blocks)*
        })
    }
}

#[derive(Debug)]
pub struct Variant {
    pub ident: Ident,
    pub data: StructData,
}
