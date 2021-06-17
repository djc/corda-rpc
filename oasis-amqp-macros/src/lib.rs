extern crate proc_macro;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};

/// Implement AMQP 1.0-related functionality for structs and enums
///
/// For enums: this macro provides a custom implementation of serde::Deserialize. Only newtype
/// variants and unit variants are supported; all variants within an enum should be of the same
/// type.
///
/// For structs: this macro is used to implement the `oasis-amqp::Described` trait. It also
/// ensures `serde::Deserialize` is implemented for a type.
#[proc_macro_attribute]
pub fn amqp(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let (impls, attrs) = match syn::parse::<syn::Item>(item.clone()).unwrap() {
        syn::Item::Enum(item) => (enum_serde(item), None),
        syn::Item::Struct(item) => struct_serde(item, attr),
        _ => panic!("amqp attribute can only be applied to enum or struct"),
    };

    let mut new = attrs.unwrap_or_else(proc_macro::TokenStream::new);
    new.extend(item);
    new.extend(impls);
    new
}

fn enum_serde(def: syn::ItemEnum) -> proc_macro::TokenStream {
    let name = &def.ident;
    let (_, orig_ty_generics, _) = def.generics.split_for_impl();
    let mut generics = def.generics.clone();
    let mut lt_def = syn::LifetimeDef {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'de", Span::call_site()),
        colon_token: None,
        bounds: syn::punctuated::Punctuated::new(),
    };

    if def.generics.lifetimes().count() > 0 {
        lt_def.bounds = def
            .generics
            .lifetimes()
            .map(|def| def.lifetime.clone())
            .collect();
    }

    generics.params = Some(syn::GenericParam::Lifetime(lt_def))
        .into_iter()
        .chain(generics.params)
        .collect();

    let de_life = syn::Lifetime::new("'de", Span::call_site());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let screaming = translate(&def.ident.to_string());
    let scope = format_ident!("_IMPL_DESERIALIZER_FOR_{}", screaming);
    let name_str = syn::LitStr::new(&name.to_string(), Span::call_site());

    let mut field_variants = TokenStream::new();
    for i in 0..def.variants.len() {
        let name = format_ident!("F{}", i);
        field_variants.append_all(quote!(#name,));
    }

    match def.variants.first().unwrap().fields {
        syn::Fields::Unnamed(_) => {}
        _ => panic!("struct variants are not supported"),
    };

    let mut tag_u64 = TokenStream::new();
    let mut bytes_arms = TokenStream::new();
    let mut variants = TokenStream::new();
    let mut visitor_arms = TokenStream::new();

    let mut int_arms = TokenStream::new();
    for (i, var) in def.variants.iter().enumerate() {
        let fields = match &var.fields {
            syn::Fields::Unnamed(f) => f,
            _ => panic!("only unnamed fields allowed here"),
        };

        if fields.unnamed.len() != 1 {
            panic!("only 1 unnamed field is allowed");
        }

        let ty = match &fields.unnamed.first().unwrap().ty {
            syn::Type::Path(p) => p,
            p => panic!("only path types allowed: {}", p.into_token_stream()),
        };

        let variant = format_ident!("F{}", i);
        let mut ty_name = ty.clone();
        let mut segment = ty_name.path.segments.last_mut().unwrap();
        segment.arguments = syn::PathArguments::None;
        int_arms.append_all(quote!(#ty_name::CODE => std::result::Result::Ok(Field::#variant),));
        bytes_arms.append_all(quote!(#ty_name::NAME => std::result::Result::Ok(Field::#variant),));

        let variant_name = syn::LitStr::new(&var.ident.to_string(), Span::call_site());
        variants.append_all(quote!(#variant_name,));

        let var_ident = &var.ident;
        visitor_arms.append_all(quote!(
            (Field::#variant, __variant) => Result::map(
                serde::de::VariantAccess::newtype_variant::<#ty_name>(__variant),
                #name::#var_ident,
            ),
        ));
    }

    tag_u64.append_all(quote!(
        fn visit_u64<E>(
            self,
            value: u64,
        ) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match Some(value) {
                #int_arms
                _ => std::result::Result::Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Unsigned(value),
                    &"invalid descriptor ID",
                )),
            }
        }
    ));

    let res = quote!(
        const #scope: () = {
            use serde;
            use std::fmt;

            impl #impl_generics serde::Deserialize<#de_life> for #name #orig_ty_generics #where_clause {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: serde::Deserializer<#de_life>,
                {
                    enum Field { #field_variants }

                    struct FieldVisitor;

                    impl #impl_generics serde::de::Visitor<#de_life> for FieldVisitor {
                        type Value = Field;

                        fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                            fmt::Formatter::write_str(fmt, "variant identifier")
                        }

                        #tag_u64

                        fn visit_bytes<E>(
                            self,
                            value: &[u8],
                        ) -> std::result::Result<Self::Value, E>
                        where
                            E: serde::de::Error,
                        {
                            match Some(value) {
                                #bytes_arms
                                _ => {
                                    let value = std::string::String::from_utf8_lossy(value);
                                    std::result::Result::Err(serde::de::Error::unknown_variant(
                                        &value, VARIANTS,
                                    ))
                                }
                            }
                        }
                    }

                    impl<#de_life> serde::Deserialize<#de_life> for Field {
                        #[inline]
                        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                        where
                            D: serde::Deserializer<#de_life>,
                        {
                            serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor)
                        }
                    }

                    struct Visitor #ty_generics {
                        marker: std::marker::PhantomData<#name#orig_ty_generics>,
                        lifetime: std::marker::PhantomData<&#de_life ()>,
                    }

                    impl #impl_generics serde::de::Visitor<#de_life> for Visitor #ty_generics {
                        type Value = #name #orig_ty_generics;
                        fn expecting(
                            &self,
                            fmt: &mut fmt::Formatter,
                        ) -> fmt::Result {
                            fmt::Formatter::write_str(fmt, "enum #name_str")
                        }
                        fn visit_enum<__A>(
                            self,
                            __data: __A,
                        ) -> std::result::Result<Self::Value, __A::Error>
                        where
                            __A: serde::de::EnumAccess<#de_life>,
                        {
                            match match serde::de::EnumAccess::variant(__data) {
                                std::result::Result::Ok(__val) => __val,
                                std::result::Result::Err(__err) => {
                                    return std::result::Result::Err(__err);
                                }
                            } {
                                #visitor_arms
                            }

                        }

                    }

                    const VARIANTS: &[&'static str] = &[
                        #variants
                    ];

                    serde::Deserializer::deserialize_enum(
                        deserializer,
                        #name_str,
                        VARIANTS,
                        Visitor {
                            marker: std::marker::PhantomData::<#name#orig_ty_generics>,
                            lifetime: std::marker::PhantomData,
                        },
                    )
                }
            }
        };
    );

    res.into()
}

fn struct_serde(
    def: syn::ItemStruct,
    meta: proc_macro::TokenStream,
) -> (proc_macro::TokenStream, Option<proc_macro::TokenStream>) {
    if meta.is_empty() {
        panic!("no arguments found for attribute on struct type");
    }

    let list = syn::parse::<syn::MetaList>(meta).unwrap();
    if !list.path.is_ident("descriptor") {
        panic!("invalid attribute {:?}", list.path.get_ident().unwrap());
    }

    let (name, code) = if list.nested.len() == 2 {
        let name = if let Some(syn::NestedMeta::Lit(syn::Lit::Str(s))) = list.nested.first() {
            s.value()
        } else {
            panic!("could not extract descriptor name from attribute");
        };

        let id = if let Some(syn::NestedMeta::Lit(syn::Lit::Int(s))) = list.nested.last() {
            s.clone()
        } else {
            panic!("could not extract descriptor ID from attribute");
        };

        (Some(name), Some(id))
    } else {
        assert_eq!(list.nested.len(), 1);
        let pair =
            if let Some(syn::NestedMeta::Meta(syn::Meta::NameValue(pair))) = list.nested.first() {
                pair
            } else {
                panic!("could not extract descriptor name or code");
            };

        if pair.path.is_ident("name") {
            if let syn::Lit::Str(s) = &pair.lit {
                (Some(s.value()), None)
            } else {
                panic!("invalid type for descriptor name");
            }
        } else if pair.path.is_ident("code") {
            if let syn::Lit::Int(s) = &pair.lit {
                (None, Some(s.clone()))
            } else {
                panic!("invalid type for descriptor name");
            }
        } else {
            panic!(
                "invalid descriptor element {:?}",
                pair.path.get_ident().unwrap()
            );
        }
    };

    let ident = def.ident;
    let generics = def.generics;

    let renamed = format!(
        "{}|{}",
        name.clone().unwrap_or_else(|| "".into()),
        code.clone()
            .map_or("".into(), |i| i.base10_digits().to_string())
    );
    let none = quote!(None);
    let name = name.map_or(none.clone(), |s| {
        let lit = syn::LitByteStr::new(s.as_bytes(), Span::call_site());
        quote!(Some(#lit))
    });
    let code = code.map_or(none, |i| quote!(Some(#i)));

    let described = quote!(
        impl#generics Described for #ident#generics {
            const NAME: Option<&'static [u8]> = #name;
            const CODE: Option<u64> = #code;
        }
    );

    let rename = quote!(#[derive(Deserialize)] #[serde(rename = #renamed)]);
    (described.into(), Some(rename.into()))
}

fn translate(s: &str) -> String {
    let mut snake = String::new();
    for (i, ch) in s.char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_uppercase());
    }
    snake
}
