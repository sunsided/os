//! # Accessor Derive
//!
//! This crate provides a derive macro for generating setters and getters for
//! structs.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitBool, parse_macro_input, spanned::Spanned};

/// Derive to generate `.set_<field>(&mut self, value: Ty) -> &mut Self` and
/// `const .with_<field>(mut self, value: Ty) -> Self` for each **named** field.
///
/// - Skipping a field: `#[setters(skip)]`
///
/// # Example
///
/// ```
/// use utils_accessors_derive::Setters;
///
/// #[derive(Setters)]
/// struct Foo<T> where T: Default {
///     a: u32,
///     #[setters(skip)]
///     _phantom: T,
/// }
///
/// let mut f = Foo { a: 1, _phantom: u8::default() };
/// f.set_a(10).set_a(11);
/// let f2 = f.with_a(42);
/// assert_eq!(f2.a, 42);
/// ```
#[proc_macro_derive(Setters, attributes(setters))]
pub fn derive_generate_setters(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = parse_macro_input!(input as DeriveInput);

    let fields = match data {
        Data::Struct(s) => match s.fields {
            Fields::Named(n) => n.named,
            Fields::Unnamed(u) => {
                return syn::Error::new(u.span(), "Setters only supports named fields")
                    .to_compile_error()
                    .into();
            }
            Fields::Unit => {
                return syn::Error::new(
                    ident.span(),
                    "GenerateSetters does not apply to unit structs",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new(
                ident.span(),
                "GenerateSetters can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut methods = Vec::new();

    for field in fields {
        let Some(fname) = &field.ident else { continue };
        if should_skip(&field.attrs) {
            continue;
        }

        let ty = &field.ty;
        let set_name = format_ident!("set_{}", fname);
        let with_name = format_ident!("with_{}", fname);

        methods.push(quote! {
            #[inline]
            pub fn #set_name(&mut self, value: #ty) -> &mut Self {
                self.#fname = value;
                self
            }

            #[inline]
            pub const fn #with_name(mut self, value: #ty) -> Self {
                self.#fname = value;
                self
            }
        });
    }

    let expanded = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}

fn should_skip(attrs: &[syn::Attribute]) -> bool {
    let mut skip = false;
    for attr in attrs {
        if !attr.path().is_ident("setters") {
            continue;
        }

        // Accept #[setters(skip)] and #[setters(skip = true)]
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                if meta.input.is_empty() {
                    skip = true; // #[setters(skip)]
                } else if let Ok(v) = meta.value()?.parse::<LitBool>()
                    && v.value
                {
                    skip = true;
                } // #[setters(skip = true)]
            }
            Ok(())
        });
    }
    skip
}
