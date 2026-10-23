use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Path to the `teloxide-router` crate as named in the user's manifest
/// (usually aliased as `txr`), falling back to the canonical name.
pub(crate) fn crate_path() -> TokenStream {
    let name = match proc_macro_crate::crate_name("teloxide-router") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => name,
        // `Itself` and errors both cover the library's own tests and doctests,
        // which link it under the canonical name; expansion never runs inside
        // the library's sources.
        _ => "teloxide_router".to_owned(),
    };
    let ident = Ident::new(&name, Span::call_site());
    quote! { ::#ident }
}
