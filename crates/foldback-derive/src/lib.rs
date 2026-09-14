// SPDX-License-Identifier: MIT OR Apache-2.0
//! `#[derive(FoldbackHash)]` — generates a
//! `foldback_core::hashable::FoldbackHash` impl (cookbook recipe 5).
//! Opt-in per field: `#[foldback(hash)]` marks a field to hash,
//! `#[foldback(skip)]` marks one as deliberately not hashed, and an
//! unmarked field is skipped too but its name is collected into the
//! generated `UNTRACKED_FIELDS` constant — visible rather than silently
//! assumed either way. See `foldback_core::hashable` for why opt-in
//! (not opt-out) was the deliberate choice here.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

enum FieldMarking {
    Hash,
    Skip,
    Unmarked,
}

fn field_marking(attrs: &[syn::Attribute]) -> Result<FieldMarking, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("foldback") {
            continue;
        }
        let ident: syn::Ident = attr.parse_args()?;
        return match ident.to_string().as_str() {
            "hash" => Ok(FieldMarking::Hash),
            "skip" => Ok(FieldMarking::Skip),
            other => Err(syn::Error::new_spanned(
                ident,
                format!("unknown #[foldback(...)] marking '{other}' — expected 'hash' or 'skip'"),
            )),
        };
    }
    Ok(FieldMarking::Unmarked)
}

#[proc_macro_derive(FoldbackHash, attributes(foldback))]
pub fn derive_foldback_hash(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(&input, "#[derive(FoldbackHash)] only supports structs")
            .to_compile_error()
            .into();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(
            &input,
            "#[derive(FoldbackHash)] requires named fields (not a tuple or unit struct)",
        )
        .to_compile_error()
        .into();
    };

    let mut hash_calls = Vec::new();
    let mut untracked_names = Vec::new();

    for field in &fields.named {
        let marking = match field_marking(&field.attrs) {
            Ok(m) => m,
            Err(e) => return e.to_compile_error().into(),
        };
        let field_ident = field.ident.as_ref().expect("Fields::Named guarantees this");
        let field_name = field_ident.to_string();

        match marking {
            FieldMarking::Hash => {
                hash_calls.push(quote! {
                    session.hash_field(
                        tick,
                        entity_id,
                        #field_name,
                        &::foldback_core::hashable::FieldBytes::field_bytes(&self.#field_ident),
                    )?;
                });
            }
            FieldMarking::Skip => {}
            FieldMarking::Unmarked => untracked_names.push(field_name),
        }
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::foldback_core::hashable::FoldbackHash for #name #ty_generics #where_clause {
            const UNTRACKED_FIELDS: &'static [&'static str] = &[#(#untracked_names),*];

            fn write_hashed_fields(
                &self,
                session: &mut ::foldback_core::session::Session,
                tick: u64,
                entity_id: u64,
            ) -> ::std::result::Result<(), ::foldback_core::Error> {
                #(#hash_calls)*
                Ok(())
            }
        }
    };

    expanded.into()
}
