use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, parse_macro_input, parse_str};

pub fn optimizable_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Define trait path as a constant - easy to change in one place
    let trait_path = parse_str::<syn::Path>("::dsrs::core::module::Optimizable").unwrap();

    // Extract parameter field names
    let parameter_fields = extract_parameter_fields(&input);

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let parameter_names: Vec<_> = parameter_fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect();

    // Generate the Optimizable implementation (flatten nested parameters with compound names)
    let expanded = quote! {
        impl #impl_generics #trait_path for #name #type_generics #where_clause {
            fn parameters(
                &mut self,
            ) -> indexmap::IndexMap<::std::string::String, &mut dyn #trait_path> {
                let mut params: indexmap::IndexMap<::std::string::String, &mut dyn #trait_path> = indexmap::IndexMap::new();
                #(
                {
                    let __field_name = stringify!(#parameter_names).to_string();
                    // SAFETY: We only create disjoint mutable borrows to distinct struct fields
                    let __field_ptr: *mut dyn #trait_path = &mut self.#parameter_names as *mut dyn #trait_path;
                    let __child_params: indexmap::IndexMap<::std::string::String, &mut dyn #trait_path> = unsafe { (&mut *__field_ptr).parameters() };
                    if __child_params.is_empty() {
                        // Leaf: insert the field itself
                        unsafe {
                            params.insert(__field_name, &mut *__field_ptr);
                        }
                    } else {
                        // Composite: flatten children with compound names
                        for (grand_name, grand_param) in __child_params.into_iter() {
                            params.insert(format!("{}.{}", __field_name, grand_name), grand_param);
                        }
                    }
                }
                )*
                params
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_parameter_fields(input: &DeriveInput) -> Vec<&Field> {
    match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named
                .named
                .iter()
                .filter(|field| has_parameter_attribute(field))
                .collect(),
            _ => {
                panic!("Optimizable can only be derived for structs with named fields");
            }
        },
        _ => {
            panic!("Optimizable can only be derived for structs");
        }
    }
}

fn has_parameter_attribute(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("parameter"))
}

#[test]
fn trybuild() {
    let t = trybuild::TestCases::new();
    t.pass("tests/optim/*.rs");
}
