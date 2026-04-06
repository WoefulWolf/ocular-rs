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

                let hook_variant_name = format_ident!("{}_HOOK", variant_name.to_string().to_case(Case::Snake).to_uppercase());
                
                let mut hook_variant_func_name = format_ident!("hook_{}", variant_name.to_string().to_case(Case::Snake));
                hook_variant_func_name.set_span(variant_name.span());

                let get_variant_func_name = format_ident!("{}", variant_name.to_string().to_case(Case::Snake));

                variant_hook_functions.extend( quote_spanned! { variant.span() =>
                    static #hook_variant_name: std::sync::OnceLock<retour::GenericDetour<#variant_name_fn>> = std::sync::OnceLock::new();

                    pub fn #hook_variant_func_name(detour: #variant_name_fn) -> Result<(), HookError> {
                        let ocular = get_ocular();

                        let hook = unsafe {
                            retour::GenericDetour::<#variant_name_fn>::new(
                                std::mem::transmute(ocular.#class.vtable().#variant_name_str),
                                detour
                            )
                        }.map_err(|e| HookError::CreateFailed(e.to_string()))?;

                        unsafe { hook.enable() }
                            .map_err(|e| HookError::EnableFailed(e.to_string()))?;

                        #hook_variant_name
                            .set(hook)
                            .map_err(|_| HookError::AlreadyHooked)
                    }

                    pub fn #get_variant_func_name() -> Option<&'static retour::GenericDetour<#variant_name_fn>> {
                        #hook_variant_name.get()
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
