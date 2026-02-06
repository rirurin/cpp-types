use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser};
use syn::Token;

pub struct FunctionParam {
    min: usize,
    max: usize
}

impl Parse for FunctionParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let min = input.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
        input.parse::<Token![,]>()?;
        let max = input.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
        if min > max {
            return Err(syn::Error::new(Span::call_site(), "Min value must be smaller than max value"));
        }
        Ok(Self { min, max })
    }
}

fn make_struct_name(index: usize) -> String {
    let plural = if index == 1 { "" } else { "s" };
    format!("With{}Param{}", index, plural)
}

pub fn create_function_param_structs(input: TokenStream) -> TokenStream {
    let params = match FunctionParam::parse.parse2(input) {
        Ok(p) => p, Err(e) => return e.to_compile_error()
    };
    let mut structs = vec![];
    for i in params.min..params.max {
        let struct_name = syn::Ident::new(&make_struct_name(i), Span::call_site());
        let mut types = vec![];
        let mut def_field = vec![];
        for p in 0..i {
            let type_name = syn::Ident::new(&format!("P{}", p), Span::call_site());
            let param_name = syn::Ident::new(&format!("_param{}", p), Span::call_site());
            types.push(quote! { #type_name });
            def_field.push(quote! {
                #param_name: #type_name
            });
        }
        let generics = match types.len() {
            0 => quote! { R }, _ => quote! { #(#types),*, R }
        };
        let def_fields = match def_field.len() {
            0 => TokenStream::new(), _ => quote! { #(#def_field),*, }
        };
        let definition = quote! {
            pub struct #struct_name<#generics> where R: Sized {
                #def_fields _return_value: PhantomData<R>
            }
        };
        let function_params = match types.len() {
            0 => quote! { &F }, _ => quote! { &F, #(&#types),* }
        };
        let mut function_value = vec![];
        for p in 0..i {
            function_value.push(syn::Ident::new(&format!("_param{}", p), Span::call_site()));
        }
        let function_values = match types.len() {
            0 => quote! { function },
            _ => quote! { function, #(&self.#function_value),* }
        };
        let impl_params = quote! {
            impl<#generics> FunctionParams<R> for #struct_name<#generics> where R: Sized {
                fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
                    (unsafe { &*(ptr as *const fn(#function_params) -> R) })(#function_values)
                }
            }
        };
        let mut in_param = vec![];
        let mut in_value = vec![];
        for p in 0..i {
            let curr_type = types[p].clone();
            let param_name = syn::Ident::new(&format!("_param{}", p), Span::call_site());
            in_param.push(quote! { #param_name: #curr_type });
            in_value.push(quote! { #param_name });
        }
        let in_values = match in_value.len() {
            0 => TokenStream::new(), _ => quote! { #(#in_value),*, }
        };
        let impl_new = quote! {
            impl<#generics> #struct_name<#generics> where R: Sized {
                pub fn new(#(#in_param),*) -> Self {
                    Self {#in_values _return_value: PhantomData::<R>}
                }
            }
        };
        // println!("{}", impl_params.to_string());
        // println!("{}", impl_new.to_string());
        structs.push(quote! {
            #definition
            #impl_params
            #impl_new
        });
    }
    quote! { #(#structs)* }
}