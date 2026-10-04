use proc_macro::TokenStream;
use quote::quote;
use syn::{
    DeriveInput, GenericArgument, Ident, Path, PathArguments, Result, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

struct IntoAttr {
    target_type: Path,
}

impl Parse for IntoAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let target_type: Path = input.parse()?;
        Ok(IntoAttr { target_type })
    }
}

pub fn derive_into(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute to get the target type name
    let attr = parse_macro_input!(attr as IntoAttr);
    let target_type = attr.target_type;

    // Parse the annotated item
    let input = parse_macro_input!(item as DeriveInput);
    let source_name = &input.ident;

    // Generate implementation based on type
    let implementation = match &input.data {
        syn::Data::Struct(data_struct) => {
            generate_struct_impl(source_name, target_type, data_struct)
        }
        syn::Data::Enum(data_enum) => generate_enum_impl(source_name, target_type, data_enum),
        _ => {
            return syn::Error::new_spanned(
                input.ident,
                "derive_into can only be used with structs or enums",
            )
            .to_compile_error()
            .into();
        }
    };

    // Combine the original item with the implementation
    let expanded = quote! {
        #input

        #implementation
    };

    expanded.into()
}

/// Check if a type is an Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && type_path.path.segments.len() == 1
    {
        return type_path.path.segments[0].ident == "Option";
    }
    false
}

/// Check if a type is a Vec<T>
fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && type_path.path.segments.len() == 1
    {
        return type_path.path.segments[0].ident == "Vec";
    }
    false
}

fn is_option_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && type_path.path.segments.len() == 1
        && type_path.path.segments[0].ident == "Option"
        && let PathArguments::AngleBracketed(angle_bracketed_args) =
            &type_path.path.segments[0].arguments
        && angle_bracketed_args.args.len() == 1
        && let GenericArgument::Type(inner_type) = &angle_bracketed_args.args[0]
        && is_vec_type(inner_type)
    {
        return true;
    }

    false
}

fn generate_struct_impl(
    source_name: &Ident,
    target_type: Path,
    data_struct: &syn::DataStruct,
) -> proc_macro2::TokenStream {
    match &data_struct.fields {
        syn::Fields::Named(fields) => {
            let field_conversions = fields.named.iter().map(|field| {
                let field_name = &field.ident;

                // Handle different container types
                if is_option_vec_type(&field.ty) {
                    quote! { #field_name: source.#field_name.map(|v| v.into_iter().map(Into::into).collect()) }
                } else if is_option_type(&field.ty) {
                    quote! { #field_name: source.#field_name.map(Into::into) }
                } else if is_vec_type(&field.ty) {
                    quote! { #field_name: source.#field_name.into_iter().map(Into::into).collect() }
                } else {
                    quote! { #field_name: source.#field_name.into() }
                }
            });

            quote! {
                impl From<#source_name> for #target_type {
                    fn from(source: #source_name) -> Self {
                        Self {
                            #(#field_conversions),*
                        }
                    }
                }
            }
        }
        syn::Fields::Unnamed(fields) => {
            let field_indices = 0..fields.unnamed.len();
            let field_conversions = field_indices.map(|i| {
                let index = syn::Index::from(i);
                let field = &fields.unnamed[i];

                if is_option_type(&field.ty) {
                    quote! { source.#index.map(Into::into) }
                } else if is_vec_type(&field.ty) {
                    quote! { source.#index.into_iter().map(Into::into).collect() }
                } else {
                    quote! { source.#index.into() }
                }
            });

            quote! {
                impl From<#source_name> for #target_type {
                    fn from(source: #source_name) -> Self {
                        Self(
                            #(#field_conversions),*
                        )
                    }
                }
            }
        }
        syn::Fields::Unit => {
            quote! {
                impl From<#source_name> for #target_type {
                    fn from(_source: #source_name) -> Self {
                        Self
                    }
                }
            }
        }
    }
}

fn generate_enum_impl(
    source_name: &Ident,
    target_type: Path,
    data_enum: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    let variant_conversions = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;

        match &variant.fields {
            syn::Fields::Named(fields) => {
                let field_names: Vec<_> = fields.named.iter().map(|f| &f.ident).collect();
                let field_conversions = fields.named.iter().map(|field| {
                    let field_name = &field.ident;

                    // Handle different container types
                    if is_option_vec_type(&field.ty) {
                        quote! { #field_name: #field_name.map(|v| v.into_iter().map(Into::into).collect()) }
                    } else if is_option_type(&field.ty) {
                        quote! { #field_name: #field_name.map(Into::into) }
                    } else if is_vec_type(&field.ty) {
                        quote! { #field_name: #field_name.into_iter().map(Into::into).collect() }
                    } else {
                        quote! { #field_name: #field_name.into() }
                    }
                });

                quote! {
                    #source_name::#variant_name { #(#field_names),* } => #target_type::#variant_name {
                        #(#field_conversions),*
                    }
                }
            }
            syn::Fields::Unnamed(fields) => {
                let field_count = fields.unnamed.len();
                let field_names: Vec<_> = (0..field_count)
                    .map(|i| syn::Ident::new(&format!("field{}", i), variant_name.span()))
                    .collect();

                let field_conversions = fields.unnamed.iter().enumerate().map(|(i, field)| {
                    let field_name = &field_names[i];

                    if is_option_type(&field.ty) {
                        quote! { #field_name.map(Into::into) }
                    } else if is_vec_type(&field.ty) {
                        quote! { #field_name.into_iter().map(Into::into).collect() }
                    } else {
                        quote! { #field_name.into() }
                    }
                });

                quote! {
                    #source_name::#variant_name(#(#field_names),*) => #target_type::#variant_name(
                        #(#field_conversions),*
                    )
                }
            }
            syn::Fields::Unit => {
                quote! {
                    #source_name::#variant_name => #target_type::#variant_name
                }
            }
        }
    });

    quote! {
        impl From<#source_name> for #target_type {
            fn from(source: #source_name) -> Self {
                match source {
                    #(#variant_conversions),*
                }
            }
        }
    }
}
