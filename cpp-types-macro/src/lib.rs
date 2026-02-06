use proc_macro::TokenStream;

#[proc_macro]
pub fn create_function_param_structs(input: TokenStream) -> TokenStream {
    cpp_types_macro_impl::param::create_function_param_structs(input.into()).into()
}