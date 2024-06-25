use crate::codegen::structs::StructData;
use crate::codegen::CodegenState;
use crate::schema::{SchemaStructMember, SchemaStructMemberType};
use miette::Context;
use proc_macro2::Ident;
use quote::{format_ident, quote};

impl CodegenState {
    pub fn codegen_object(
        &mut self,
        name: Ident,
        mut fields: Vec<SchemaStructMember>,
        switch: Option<String>,
    ) -> miette::Result<StructData> {
        fields.insert(
            0,
            SchemaStructMember {
                name: "Id".to_string(),
                ty: SchemaStructMemberType::Object,
                minvalue: None,
                maxvalue: None,
                typeid: Some(name.to_string()),
                options: Some("notnull".to_string()),
                case: None,
                alias: None,
                default: None,
                arguments: None,
                description: None,
            },
        );

        let is_switch = switch.is_some();

        let mut data = self
            .codegen_struct(name.clone(), fields, switch)
            .context("Failed to generate object data")?;

        let id_name = format_ident!("{}Id", name);

        let id_field_getter = if is_switch {
            quote!(*x.id())
        } else {
            quote!(x.id)
        };

        let code = data.code;

        data.id_access = Some(id_field_getter.clone());

        data.code = quote! {
            pub type #id_name = DatabaseItemId::<#name>;
            #code

            impl DatabaseItemWithId for #name {
                fn id(&self) -> DatabaseItemId<Self> {
                    let x = self;
                    #id_field_getter
                }
            }
        };

        Ok(data)
    }
}
