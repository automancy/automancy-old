use proc_macro::TokenStream;

use proc_macro2::TokenTree;
use syn::{Attribute, Meta};

const NAMESPACE: &str = "namespace";
const NAME: &str = "name";

fn e(field: &str, namespace: &str, name: &str) -> String {
    format!("{field}: automancy_defs::id::id_static(\"{namespace}\", \"{name}\").to_id(interner),")
}

fn parse(attr: &Attribute) -> String {
    match &attr.meta {
        Meta::List(list) => list
            .tokens
            .clone()
            .into_iter()
            .flat_map(|v| match v {
                TokenTree::Literal(v) => Some(v.to_string().trim_matches('\"').to_string()),
                TokenTree::Ident(v) => Some(v.to_string()),
                _ => None,
            })
            .next(),
        _ => None,
    }
    .unwrap_or_else(|| panic!("should be either an identifier in parentheses"))
}

/// # Examples
///
/// ```
/// use automancy_macros::IdReg;
/// use automancy_defs::id::Id;
///
/// #[derive(IdReg)]
/// pub struct FooIds {
///     id_foo: Id,
///     #[namespace(core)]
///     id_bar: Id,
///     #[namespace("meowzer/")]
///     #[name(zoo)]
///     id_zoo: Id,
/// }
/// ```
///
/// # Invalid usages
/// ```compile_fail
/// #[derive(automancy_macros::IdReg)]
/// pub enum Foo {}
///
/// #[derive(automancy_macros::IdReg)]
/// pub struct Bar();
/// ```
#[proc_macro_derive(IdReg, attributes(name, namespace))]
pub fn derive_id_reg(item: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();

    let mut names = vec![];
    let mut namespaces = vec![];

    match ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields_named) => {
                for field in fields_named.named.iter() {
                    let mut iter = field.attrs.iter();
                    let attrs @ [a, b] = [iter.next(), iter.next()];

                    let [a_ident, b_ident] =
                        attrs.map(|v| v.and_then(|v| v.path().get_ident().map(|v| v.to_string())));

                    let name = field.ident.clone().unwrap().to_string();

                    namespaces.push(if Some(NAMESPACE) == a_ident.as_deref() {
                        (name.clone(), parse(a.unwrap()))
                    } else if Some(NAMESPACE) == b_ident.as_deref() {
                        (name.clone(), parse(b.unwrap()))
                    } else {
                        (name.clone(), "automancy".to_string())
                    });

                    names.push(if Some(NAME) == a_ident.as_deref() {
                        parse(a.unwrap())
                    } else if Some(NAME) == b_ident.as_deref() {
                        parse(b.unwrap())
                    } else {
                        name
                    });
                }
            }
            _ => panic!("must be a struct with named fields"),
        },
        _ => panic!("must be a struct"),
    }

    let name = ast.ident;

    let items = namespaces
        .into_iter()
        .zip(names.into_iter())
        .map(|((field, namespace), name)| e(&field, &namespace, &name))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "
        impl {name} {{
            pub fn new(interner: &mut automancy_defs::id::Interner) -> Self {{
                Self {{
                    {items}
                }}
            }}
        }}
        "
    )
    .parse()
    .unwrap()
}
