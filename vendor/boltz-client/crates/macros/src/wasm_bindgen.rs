use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type,
    TypePath, parse_macro_input,
};

pub fn extern_wasm_bindgen(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let (additional_definition, input, output) = extern_wasm_bindgen_from(attr, &derive_input);
    let output = quote! {
        #[derive(serde::Deserialize, serde::Serialize, tsify_next::Tsify)]
        #[tsify(from_wasm_abi, into_wasm_abi)]
        #additional_definition
        #input
        #output
    };
    output.into()
}

fn extern_wasm_bindgen_from(
    attr: proc_macro::TokenStream,
    derive_input: &DeriveInput,
) -> (
    /* additional_definition */ TokenStream,
    /* input */ DeriveInput,
    /* output */ TokenStream,
) {
    let mut input = derive_input.clone();
    let mut output = TokenStream::new();
    let internal_name = &derive_input.ident;
    let external_name = TokenStream::from(attr);
    let additional_definition = match &derive_input.data {
        Data::Enum(data) => {
            let (additional_definition, input_data_enum, output_impl) =
                extern_wasm_bindgen_from_enum(internal_name, &external_name, data);
            input.data = Data::Enum(input_data_enum);
            output.extend(output_impl);
            additional_definition
        }
        Data::Struct(data) => {
            let (additional_definition, input_data_struct, output_impl) =
                extern_wasm_bindgen_from_struct(internal_name, &external_name, data);
            input.data = Data::Struct(input_data_struct);
            output.extend(output_impl);
            additional_definition
        }
        _ => quote! {
            #[serde(rename_all = "camelCase")]
        },
    };
    output.extend(quote! {
        impl wasm_bindgen::__rt::VectorIntoJsValue for #internal_name {
            fn vector_into_jsvalue(vector: Box<[#internal_name]>) -> wasm_bindgen::JsValue {
                wasm_bindgen::__rt::js_value_vector_into_jsvalue(vector)
            }
        }
    });
    (additional_definition, input, output)
}

fn extern_wasm_bindgen_from_enum(
    internal_name: &Ident,
    external_name: &TokenStream,
    data_enum: &DataEnum,
) -> (
    /* additional_definition */ TokenStream,
    /* input_data_enum */ DataEnum,
    /* output_impl */ TokenStream,
) {
    let input_data_enum = update_data_enum(data_enum);
    let use_tag = data_enum
        .variants
        .iter()
        .any(|variant| !variant.fields.is_empty());
    let variants = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let field_names = get_enum_field_names(&variant.fields);
        let fields = get_enum_fields(&variant.fields);
        if fields.is_empty() {
            quote! { #variant_name => Self::#variant_name }
        } else {
            quote! { #variant_name{#(#field_names),*} => Self::#variant_name{#(#fields),*} }
        }
    });
    let _variants_clone = variants.clone();

    let from_external_variants = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let field_names = get_enum_field_names(&variant.fields);
        let fields = get_enum_fields(&variant.fields);
        if fields.is_empty() {
            quote! { #external_name::#variant_name => #internal_name::#variant_name }
        } else {
            match &variant.fields {
                Fields::Named(_) => {
                    quote! { #external_name::#variant_name{#(#field_names),*} => #internal_name::#variant_name{#(#fields),*} }
                }
                Fields::Unnamed(_) => {
                    quote! { #external_name::#variant_name(#(#field_names),*) => #internal_name::#variant_name(#(#fields),*) }
                }
                Fields::Unit => {
                    quote! { #external_name::#variant_name => #internal_name::#variant_name }
                }
            }
        }
    });

    let from_internal_variants = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let field_names = get_enum_field_names(&variant.fields);
        let fields = get_enum_fields(&variant.fields);
        if fields.is_empty() {
            quote! { #internal_name::#variant_name => #external_name::#variant_name }
        } else {
            match &variant.fields {
                Fields::Named(_) => {
                    quote! { #internal_name::#variant_name{#(#field_names),*} => #external_name::#variant_name{#(#fields),*} }
                }
                Fields::Unnamed(_) => {
                    quote! { #internal_name::#variant_name(#(#field_names),*) => #external_name::#variant_name(#(#fields),*) }
                }
                Fields::Unit => {
                    quote! { #internal_name::#variant_name => #external_name::#variant_name }
                }
            }
        }
    });

    let mut output_impl = TokenStream::new();
    let additional_definition = if use_tag {
        quote! {
            #[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
        }
    } else {
        quote! {
            #[serde(rename_all = "camelCase")]
        }
    };
    output_impl.extend(quote! {
        impl From<#external_name> for #internal_name {
            fn from(val: #external_name) -> Self {
                match val {
                    #(#from_external_variants),*
                }
            }
        }
        impl From<#internal_name> for #external_name {
            fn from(val: #internal_name) -> Self {
                match val {
                    #(#from_internal_variants),*
                }
            }
        }
    });
    (additional_definition, input_data_enum, output_impl)
}

fn extern_wasm_bindgen_from_struct(
    internal_name: &Ident,
    external_name: &TokenStream,
    data_struct: &DataStruct,
) -> (
    /* additional_definition */ TokenStream,
    /* input_data_struct */ DataStruct,
    /* output_impl */ TokenStream,
) {
    let input_data_struct = update_data_struct(data_struct);
    let fields = get_struct_fields(&data_struct.fields);
    let fields_clone = fields.clone();
    let additional_definition = quote! {
        #[serde(rename_all = "camelCase")]
    };
    let output_impl = quote! {
        impl From<#external_name> for #internal_name {
            fn from(val: #external_name) -> Self {
                #internal_name {
                    #(#fields),*
                }
            }
        }
        impl From<#internal_name> for #external_name {
            fn from(val: #internal_name) -> Self {
                #external_name {
                    #(#fields_clone),*
                }
            }
        }
    };
    (additional_definition, input_data_struct, output_impl)
}

fn get_path(path_args: &PathArguments) -> Option<&TypePath> {
    if let PathArguments::AngleBracketed(bracketed_args) = path_args {
        for arg in bracketed_args.args.iter() {
            if let GenericArgument::Type(ty) = arg {
                if let Type::Path(path) = ty {
                    return Some(path);
                } else if let Type::Reference(reference) = ty
                    && let Type::Path(ref path) = *reference.elem
                {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn update_data_enum(data_enum: &DataEnum) -> DataEnum {
    let mut data_enum = data_enum.clone();
    for variant in data_enum.variants.iter_mut() {
        variant.fields = update_data_fields(&variant.fields);
    }
    data_enum
}

fn update_data_struct(data_struct: &DataStruct) -> DataStruct {
    let mut data_struct = data_struct.clone();
    data_struct.fields = update_data_fields(&data_struct.fields);
    data_struct
}

fn update_data_fields(fields: &Fields) -> Fields {
    let mut fields = fields.clone();
    if let Fields::Named(ref mut fields) = fields {
        for field in fields.named.iter_mut() {
            if let Type::Path(ref type_path) = field.ty {
                let segment = &type_path.path.segments[0];
                if segment.ident == "Option" {
                    field.attrs.push(
                        syn::parse_quote! { #[serde(skip_serializing_if = "Option::is_none")] },
                    );
                }
            }
        }
    }
    fields
}

fn get_struct_fields(fields: &Fields) -> Vec<TokenStream> {
    fields
        .iter()
        .filter_map(|field| {
            let ty = &field.ty;
            field.ident.as_ref().map(|ident| match ty {
                Type::Path(type_path) => {
                    let segment = &type_path.path.segments[0];
                    if segment.ident == "Vec" {
                        quote! { #ident: val.#ident.into_iter().map(|i| i.into()).collect() }
                    } else if segment.ident == "HashMap" {
                        quote! { #ident: val.#ident.into_iter().map(|(k, v)| (k, v.into())).collect() }
                    } else if segment.ident == "Option" {
                        if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "Vec") {
                            quote! { #ident: val.#ident.map(|i| i.into_iter().map(|a| a.into()).collect()) }
                        } else if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "HashMap") {
                            quote! { #ident: val.#ident.map(|i| i.into_iter().map(|(k, v)| (k, v.into())).collect()) }
                        } else {
                            quote! { #ident: val.#ident.map(|i| i.into()) }
                        }
                    } else {
                        quote! { #ident: val.#ident.into() }
                    }
                }
                _ => quote! { #ident: val.#ident.into() },
            })
        })
        .collect()
}

fn get_enum_fields(fields: &Fields) -> Vec<TokenStream> {
    match fields {
        Fields::Named(fields) => {
            fields
                .named
                .iter()
                .filter_map(|field| {
                    let ty = &field.ty;
                    field.ident.as_ref().map(|ident| match ty {
                        Type::Path(type_path) => {
                            let segment = &type_path.path.segments[0];
                            if segment.ident == "Vec" {
                                quote! { #ident: #ident.into_iter().map(|i| i.into()).collect() }
                            } else if segment.ident == "HashMap" {
                                quote! { #ident: #ident.into_iter().map(|(k, v)| (k, v.into())).collect() }
                            } else if segment.ident == "Option" {
                                if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "Vec") {
                                    quote! { #ident: #ident.map(|i| i.into_iter().map(|a| a.into()).collect()) }
                                } else if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "HashMap") {
                                    quote! { #ident: #ident.map(|i| i.into_iter().map(|(k, v)| (k, v.into())).collect()) }
                                } else {
                                    quote! { #ident: #ident.map(|i| i.into()) }
                                }
                            } else {
                                quote! { #ident: #ident.into() }
                            }
                        }
                        _ => quote! { #ident: #ident.into() },
                    })
                })
                .collect()
        }
        Fields::Unnamed(fields) => {
            fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let ident = syn::Ident::new(&format!("_{i}"), proc_macro2::Span::call_site());
                    let ty = &field.ty;
                    match ty {
                        Type::Path(type_path) => {
                            let segment = &type_path.path.segments[0];
                            if segment.ident == "Vec" {
                                quote! { #ident.into_iter().map(|i| i.into()).collect() }
                            } else if segment.ident == "HashMap" {
                                quote! { #ident.into_iter().map(|(k, v)| (k, v.into())).collect() }
                            } else if segment.ident == "Option" {
                                if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "Vec") {
                                    quote! { #ident.map(|i| i.into_iter().map(|a| a.into()).collect()) }
                                } else if get_path(&segment.arguments).is_some_and(|tp| tp.path.segments[0].ident == "HashMap") {
                                    quote! { #ident.map(|i| i.into_iter().map(|(k, v)| (k, v.into())).collect()) }
                                } else {
                                    quote! { #ident.map(|i| i.into()) }
                                }
                            } else {
                                quote! { #ident.into() }
                            }
                        }
                        _ => quote! { #ident.into() },
                    }
                })
                .collect()
        }
        Fields::Unit => vec![],
    }
}

fn get_enum_field_names(fields: &Fields) -> Vec<TokenStream> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref().map(|ident| quote! { #ident }))
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let ident = syn::Ident::new(&format!("_{i}"), proc_macro2::Span::call_site());
                quote! { #ident }
            })
            .collect(),
        Fields::Unit => vec![],
    }
}
