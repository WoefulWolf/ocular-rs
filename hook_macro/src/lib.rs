use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2};
use syn::{parse_macro_input, DeriveInput};
use syn::spanned::Spanned;
use quote::{quote, quote_spanned, format_ident};
use convert_case::{Case, Casing};

#[proc_macro_derive(Hookable)]
pub fn derive_hookable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let data = &input.data;

    let class = format_ident!("{}", name.to_string().as_str().to_case(Case::Snake));

    let mut variant_hook_functions = TokenStream2::new();

    match data {
        syn::Data::Enum(data_enum) => {
            for variant in &data_enum.variants {
                let variant_name = &variant.ident;
                let variant_name_str = format_ident!("{}", variant_name.to_string());
                let variant_name_fn = format_ident!("{}Fn", variant_name.to_string());

                let variant_name_msg = format!("Failed to hook {}", variant_name.to_string());
                
                let hook_variant_name = format_ident!("{}_HOOK", variant_name.to_string().to_case(Case::Snake).to_uppercase());
                
                let mut hook_variant_func_name = format_ident!("hook_{}", variant_name.to_string().to_case(Case::Snake));
                hook_variant_func_name.set_span(variant_name.span());

                let get_variant_func_name = format_ident!("get_{}", variant_name.to_string().to_case(Case::Snake));

                variant_hook_functions.extend( quote_spanned! { variant.span() =>
                    // Static to store detour
                    static mut #hook_variant_name: Option<GenericDetour<#variant_name_fn>> = None;

                    // Create hook
                    pub fn #hook_variant_func_name(detour: #variant_name_fn) {
                        let ocular = get_ocular();

                        unsafe {
                            let hook = GenericDetour::<#variant_name_fn>::new(std::mem::transmute(ocular.#class.vtable().#variant_name_str), detour).expect(#variant_name_msg);
                            let _ = hook.enable();
                            
                            #hook_variant_name = Some(hook);
                        }
                    }

                    // Get original func
                    pub fn #get_variant_func_name() -> Option<&'static GenericDetour<#variant_name_fn>> {
                        unsafe { #hook_variant_name.as_ref() }
                    }
                });
            }
        },
        _ => {}
    }

    let expanded = quote! {
        #variant_hook_functions
    };

    TokenStream::from(expanded)
}
