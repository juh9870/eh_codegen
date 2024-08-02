use std::str::FromStr;

use itertools::Itertools;
use miette::bail;
use proc_macro2::Ident;
use quote::{format_ident, quote};

use codegen_schema::schema::SchemaEnumItem;

use crate::codegen::{CodegenState, TokensResult};
use crate::m_try;

impl CodegenState {
    pub fn codegen_enum(&mut self, name: Ident, items: Vec<SchemaEnumItem>) -> TokensResult {
        let mut is_char = false;
        self.enums.insert(
            name.to_string(),
            items.iter().map(|i| i.name.clone()).collect(),
        );
        let variants: Vec<_> = items
            .iter()
            .map(|SchemaEnumItem { name, value: raw_value, description, .. }| {
                m_try(|| {
                    let ident = format_ident!("{}", name);
                    let value = match raw_value {
                        None => {
                            quote!()
                        }
                        Some(value) => match i32::from_str(value) {
                            Ok(num) => quote! { = #num },
                            Err(_) => {
                                if !value.starts_with('\'') || value.len() != 3 {
                                    bail!("Enum value must be an integer or a character in 'c' form, but got `{}`", value)
                                }

                                is_char = true;

                                let char_code = value
                                    .chars().nth(1)
                                    .expect("Length should be 3 here")
                                    as u32;

                                quote! {
                                = #char_code
                            }
                            }
                        },
                    };
                    let char_comment = raw_value.as_ref().filter(|s| s.starts_with('\'')).map(|value| quote!(#[doc = #value]));
                    let desc_comment = description.as_ref().map(|value| quote!(#[doc = #value]));
                    Ok(quote! {
                        #desc_comment
                        #char_comment
                        #ident #value,
                    })
                })
            })
            .try_collect()?;

        let mut derive_reprs = false;
        let impls = if is_char {
            let named_items: Vec<_> = items
                .iter()
                .filter_map(|i| {
                    let name = i.value.as_ref().map(|s| s[1..2].to_string())?;
                    Some((name, i))
                })
                .collect();
            let valid_names = named_items.iter().map(|i| &i.0);
            let deser_matches = named_items.iter().map(|(name, item)| {
                let ident = format_ident!("{}", item.name);
                quote! {
                    #name => Ok(Self::#ident),
                }
            });
            quote! {
                impl serde::Serialize for #name {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: serde::Serializer,
                    {
                        self.to_string().serialize(serializer)
                    }
                }

                impl <'de> serde::Deserialize<'de> for #name {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::de::Deserializer<'de> {
                        let name = String::deserialize(deserializer)?;
                        match name.as_str() {
                            #(#deser_matches)*
                            _ => Err(serde::de::Error::unknown_variant(&name, &[#(#valid_names,)*]))
                        }
                    }
                }

                impl std::fmt::Display for #name {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        let code = *self as u32;
                        if code == 0 {
                            write!(f, "")
                        } else {
                            write!(f, "{}", char::from_u32(code).unwrap())
                        }
                    }
                }
            }
        } else {
            derive_reprs = true;
            quote! {}
        };

        let repr = if is_char { quote!(u32) } else { quote!(i32) };

        let name_str = name.to_string();

        let derive_reprs = derive_reprs
            .then(|| quote! {#[derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr)]});

        Ok(quote! {
            #[repr(#repr)]
            #[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
            #derive_reprs
            pub enum #name {
                #[default]
                #(#variants)*
            }

            impl DatabaseItem for #name {
                fn validate(&self, _ctx: DiagnosticContextRef) {}

                fn type_name() -> &'static str {
                    #name_str
                }
            }

            #impls
        })
    }
}
